use crate::bus::{CART_BASE, CART_SIZE};

pub const BUNDLE_MAGIC: [u8; 8] = *b"V68BNDL\0";
pub const HEADER_LEN: usize = 16;
pub const MAGIC: [u8; 4] = *b"V68\0";
pub const VERSION: u32 = 0;

#[derive(Debug, PartialEq, Eq)]
pub enum CartError {
    BadEntry(u32),
    BadFooter,
    BadMagic,
    BadVersion(u32),
    Empty,
    OddEntry(u32),
    TooLarge(usize),
    Truncated,
}

impl std::fmt::Display for CartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadEntry(entry) => write!(f, "entry {entry:#010x} is outside the cart image"),
            Self::BadFooter => f.write_str("bundled cart length exceeds the executable"),
            Self::BadMagic => f.write_str("not a .v68 cart (bad magic)"),
            Self::BadVersion(version) => write!(f, "cart format version {version} is unsupported"),
            Self::Empty => f.write_str("entry is 0"),
            Self::OddEntry(entry) => write!(f, "entry {entry:#010x} is odd"),
            Self::TooLarge(len) => write!(f, "{len} bytes, over the {CART_SIZE}-byte cart window"),
            Self::Truncated => f.write_str("truncated"),
        }
    }
}

pub fn bundled(exe: &[u8]) -> Result<Option<&[u8]>, CartError> {
    const FOOTER: usize = 8 + BUNDLE_MAGIC.len();

    let Some(foot) = exe.len().checked_sub(FOOTER) else {
        return Ok(None);
    };

    if exe[foot + 8..] != BUNDLE_MAGIC {
        return Ok(None);
    }

    let len = u64::from_be_bytes(exe[foot..foot + 8].try_into().unwrap());
    let start = usize::try_from(len)
        .ok()
        .and_then(|l| foot.checked_sub(l))
        .ok_or(CartError::BadFooter)?;

    Ok(Some(&exe[start..foot]))
}

pub fn parse(file: &[u8]) -> Result<u32, CartError> {
    if file.len() < HEADER_LEN {
        return Err(CartError::Truncated);
    }

    if file[0..4] != MAGIC {
        return Err(CartError::BadMagic);
    }

    if file.len() > CART_SIZE as usize {
        return Err(CartError::TooLarge(file.len()));
    }

    let be32 = |o: usize| u32::from_be_bytes(file[o..o + 4].try_into().unwrap());
    let version = be32(4);

    if version != VERSION {
        return Err(CartError::BadVersion(version));
    }

    let entry = be32(8);

    if entry == 0 {
        return Err(CartError::Empty);
    }

    let offset = entry
        .checked_sub(CART_BASE)
        .map_or(usize::MAX, |o| o as usize);

    if !(HEADER_LEN..file.len()).contains(&offset) {
        return Err(CartError::BadEntry(entry));
    }

    if entry % 2 != 0 {
        return Err(CartError::OddEntry(entry));
    }

    Ok(entry)
}

#[cfg(test)]
pub(crate) fn test_bios() -> Vec<u8> {
    use crate::bus::STACK_TOP;

    let mut b = Vec::new();

    b.extend_from_slice(&STACK_TOP.to_be_bytes()); // vector 0: initial SP
    b.extend_from_slice(&8u32.to_be_bytes()); // vector 1: reset PC
    b.extend_from_slice(&[0x23, 0xfc, 0x00, 0x00, 0x00, 0x7f, 0xff, 0x00, 0x00, 0x10]); // move.l #0x7F, BRIGHTNESS
    b.extend_from_slice(&[0x4e, 0xf9]); // jmp cart entry
    b.extend_from_slice(&(CART_BASE + HEADER_LEN as u32).to_be_bytes());

    b
}

#[cfg(test)]
pub(crate) fn test_cart(code: &[u8]) -> Vec<u8> {
    test_header(CART_BASE + HEADER_LEN as u32, code)
}

#[cfg(test)]
fn test_header(entry: u32, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();

    f.extend_from_slice(&MAGIC);
    f.extend_from_slice(&VERSION.to_be_bytes());

    for w in [entry, 0] {
        f.extend_from_slice(&w.to_be_bytes());
    }

    f.extend_from_slice(payload);

    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(exe: &[u8], cart: &[u8]) -> Vec<u8> {
        let mut f = exe.to_vec();

        f.extend_from_slice(cart);
        f.extend_from_slice(&(cart.len() as u64).to_be_bytes());
        f.extend_from_slice(&BUNDLE_MAGIC);

        f
    }

    fn native_with_entry(entry: u32) -> Vec<u8> {
        let mut f = test_cart(&[0x60, 0xfe]);

        f[8..12].copy_from_slice(&entry.to_be_bytes());

        f
    }

    #[test]
    fn accepts_entry_at_payload_start() {
        assert_eq!(
            parse(&native_with_entry(CART_BASE + HEADER_LEN as u32)).unwrap(),
            CART_BASE + HEADER_LEN as u32
        );
    }

    #[test]
    fn bundle_magic_matches_xtask() {
        assert_eq!(BUNDLE_MAGIC, xtask::BUNDLE_MAGIC);
    }

    #[test]
    fn bundled_ignores_a_plain_binary() {
        assert_eq!(
            bundled(b"no footer here, just plain trailing bytes"),
            Ok(None)
        );
        assert_eq!(bundled(b"short"), Ok(None));
        assert_eq!(bundled(b""), Ok(None));
    }

    #[test]
    fn bundled_recovers_the_appended_cart() {
        assert_eq!(
            bundled(&bundle(b"an executable image", b"cart bytes")),
            Ok(Some(&b"cart bytes"[..]))
        );
        assert_eq!(bundled(&bundle(b"exe", b"")), Ok(Some(&b""[..])));
    }

    #[test]
    fn bundled_rejects_a_length_past_the_file_start() {
        let mut f = b"tiny".to_vec();

        f.extend_from_slice(&100u64.to_be_bytes());
        f.extend_from_slice(&BUNDLE_MAGIC);

        assert_eq!(bundled(&f).err(), Some(CartError::BadFooter));
    }

    #[test]
    fn parses_a_cart_header() {
        assert_eq!(
            parse(&test_cart(&[0x60, 0xfe])).unwrap(),
            CART_BASE + HEADER_LEN as u32
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut f = test_cart(&[0x60, 0xfe]);

        f[0] = b'X';

        assert_eq!(parse(&f).err(), Some(CartError::BadMagic));
    }

    #[test]
    fn rejects_empty_cart() {
        assert_eq!(
            parse(&test_header(0, &[0x60, 0xfe])).err(),
            Some(CartError::Empty)
        );
    }

    #[test]
    fn rejects_entry_far_outside_the_payload() {
        for entry in [1u32, CART_BASE - 1, CART_BASE + CART_SIZE, u32::MAX] {
            assert_eq!(
                parse(&native_with_entry(entry)).err(),
                Some(CartError::BadEntry(entry))
            );
        }
    }

    #[test]
    fn rejects_entry_outside_payload() {
        for entry in [
            CART_BASE,
            CART_BASE + 4,
            CART_BASE + 8,
            CART_BASE + 12,
            CART_BASE + 64,
        ] {
            assert_eq!(
                parse(&native_with_entry(entry)).err(),
                Some(CartError::BadEntry(entry))
            );
        }
    }

    #[test]
    fn rejects_odd_entry() {
        let entry = CART_BASE + HEADER_LEN as u32 + 1;

        assert_eq!(
            parse(&native_with_entry(entry)).err(),
            Some(CartError::OddEntry(entry))
        );
    }

    #[test]
    fn rejects_truncated_payload() {
        assert_eq!(parse(b"V68\0").err(), Some(CartError::Truncated));
        assert_eq!(parse(b"").err(), Some(CartError::Truncated));
        assert_eq!(
            parse(&test_cart(&[0x60, 0xfe])[..HEADER_LEN - 1]).err(),
            Some(CartError::Truncated)
        );
    }

    #[test]
    fn rejects_wrong_version() {
        let mut f = test_cart(&[0x60, 0xfe]);

        f[7] = 1;

        assert_eq!(parse(&f).err(), Some(CartError::BadVersion(1)));
    }
}

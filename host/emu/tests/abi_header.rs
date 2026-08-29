use vega68::{bus, vdp};

use std::collections::BTreeMap;

fn header_const(header: &str, name: &str) -> u32 {
    let line = header
        .lines()
        .find(|l| l.starts_with("#define") && l.split_whitespace().nth(1) == Some(name))
        .unwrap_or_else(|| panic!("missing #define {name}"));

    line.split_once(name)
        .unwrap()
        .1
        .trim()
        .trim_matches(['(', ')'])
        .split('*')
        .map(|term| {
            let term = term.trim();

            match term.strip_prefix("0x") {
                Some(hex) => u32::from_str_radix(hex, 16).unwrap(),
                None => term.parse().unwrap_or_else(|_| header_const(header, term)),
            }
        })
        .product()
}

fn repo_file(name: &str) -> String {
    let path = xtask::repo_root().unwrap().join(name);

    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn devkit_headers() -> String {
    ["devkit/sys.h", "devkit/gfx.h", "devkit/afx.h"]
        .map(repo_file)
        .join("\n")
}

#[test]
fn header_matches_emulator_abi() {
    let header = devkit_headers();

    #[rustfmt::skip]
    let defines: Vec<(&str, String)> = vec![
        ("V68_VRAM", format!("{:#010X}", bus::VRAM_BASE)),
        ("V68_TILES", format!("{:#010X}", bus::VRAM_BASE)),
        ("V68_TILEMAPS", format!("{:#010X}", bus::VRAM_BASE + 0x4_0000)),
        ("V68_SPRITES", format!("{:#010X}", bus::VRAM_BASE + 0x6_0000)),
        ("V68_SCROLL", format!("{:#010X}", bus::VRAM_BASE + 0x6_1000)),
        ("V68_PALETTE", format!("{:#010X}", bus::PALETTE_BASE)),
    ];

    for (name, value) in defines {
        let line = header
            .lines()
            .find(|l| l.starts_with("#define") && l.split_whitespace().nth(1) == Some(name))
            .unwrap_or_else(|| panic!("devkit: missing #define {name}"));

        assert!(
            line.to_uppercase().contains(&value.to_uppercase()),
            "devkit: {name} disagrees with emulator ({line} vs {value})"
        );
    }
}

fn bus_mmio_consts(bus_src: &str) -> BTreeMap<String, u32> {
    bus_src
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("pub const ")?;
            let (name, rest) = rest.split_once(':')?;
            let rest = rest.trim().strip_prefix("u32 = ")?;
            let (value, _) = rest.split_once(';')?;
            let value = value.trim();

            if name == "MMIO_BASE" || name.starts_with("AUDIO_") {
                return None;
            }

            let Some(hex) = value.strip_prefix("0x") else {
                assert!(
                    !value.contains("MMIO_BASE"),
                    "bus.rs: {name}: MMIO const {value:?} is not a bare 0x literal; \
                     mmio_block_is_exhaustive can't check it"
                );
                return None;
            };
            let hex = hex.replace('_', "");

            if hex.len() != 8 || !hex.to_uppercase().starts_with("FF00") {
                return None;
            }

            Some((name.to_string(), u32::from_str_radix(&hex, 16).unwrap()))
        })
        .collect()
}

fn header_mmio_defines(header: &str) -> BTreeMap<String, u32> {
    header
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("#define V68_")?;
            let (name, rest) = rest.split_once(char::is_whitespace)?;

            if name.starts_with("AUDIO_") {
                return None;
            }

            let idx = rest.find("0x")?;
            let hex: String = rest[idx + 2..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();

            if hex.len() != 8 || !hex.to_uppercase().starts_with("FF00") {
                return None;
            }

            Some((name.to_string(), u32::from_str_radix(&hex, 16).unwrap()))
        })
        .collect()
}

#[test]
fn mmio_block_is_exhaustive() {
    let bus_src = repo_file("host/emu/src/bus.rs");
    let header = devkit_headers();

    let rust = bus_mmio_consts(&bus_src);
    let mut hdr = header_mmio_defines(&header);

    assert!(!rust.is_empty(), "no MMIO consts parsed from bus.rs");

    for (name, value) in &rust {
        let hval = hdr
            .remove(name)
            .unwrap_or_else(|| panic!("devkit: missing #define V68_{name}"));

        assert_eq!(
            hval, *value,
            "V68_{name} disagrees between bus.rs ({value:#010X}) and devkit ({hval:#010X})"
        );
    }

    assert!(
        hdr.is_empty(),
        "devkit defines MMIO registers with no Rust const: {:?}",
        hdr.keys().collect::<Vec<_>>()
    );
}

#[test]
fn vblank_and_line_mask_invariants() {
    let header = devkit_headers();

    let vblank = header_const(&header, "V68_VBLANK");
    let line_mask = header_const(&header, "V68_LINE_MASK");

    assert!(
        line_mask >= bus::LINES_PER_FRAME - 1,
        "V68_LINE_MASK ({line_mask:#X}) does not cover the largest line number"
    );
    assert_eq!(
        vblank & line_mask,
        0,
        "V68_VBLANK overlaps V68_LINE_MASK's line field"
    );
    assert_eq!(vblank.count_ones(), 1, "V68_VBLANK is not a single bit");
}

#[test]
fn header_sizes_match_emulator_abi() {
    let header = devkit_headers();

    let sizes: Vec<(&str, u32)> = vec![
        ("V68_VRAM_SIZE", bus::VRAM_SIZE),
        ("V68_PALETTE_SIZE", bus::PALETTE_SIZE / 4),
        ("V68_BRIGHTNESS_LEVELS", u8::MAX as u32 + 1),
        ("V68_TILEMAP_COLS", vdp::TILEMAP_COLS as u32),
        ("V68_TILEMAP_ROWS", vdp::TILEMAP_ROWS as u32),
        (
            "V68_TILEMAP_CELLS",
            (vdp::TILEMAP_COLS * vdp::TILEMAP_ROWS) as u32,
        ),
        ("V68_TILEMAP_PLANES", vdp::TILEMAP_PLANES as u32),
        ("V68_SPRITE_COUNT", vdp::SPRITE_COUNT as u32),
    ];

    assert_eq!(vdp::SPRITE_STRIDE, 8);

    for (name, value) in sizes {
        assert_eq!(
            header_const(&header, name),
            value,
            "devkit: {name} disagrees with emulator"
        );
    }
}

#[test]
fn gfx_flag_defines_match_the_renderer() {
    let header = devkit_headers();
    let on = 0x8000u16;
    let hi = header_const(&header, "V68_HI") as u16;
    let hflip = header_const(&header, "V68_HFLIP") as u16;
    let vflip = header_const(&header, "V68_VFLIP") as u16;

    let vram = bus::VRAM_BASE as usize;
    let mut mem = vec![0u8; bus::MEM_END as usize];

    mem[bus::PALETTE_BASE as usize + 4..][..4].copy_from_slice(&0x0011_1111u32.to_be_bytes());
    mem[bus::PALETTE_BASE as usize + 8..][..4].copy_from_slice(&0x0022_2222u32.to_be_bytes());

    mem[vram + 64] = 1;
    mem[vram + 128..vram + 192].fill(2);

    let mut cell = |col: usize, entry: u16| {
        mem[vram + 0x4_0000 + col * 2..][..2].copy_from_slice(&entry.to_be_bytes());
    };
    cell(0, 1 | hflip);
    cell(1, 1 | vflip);
    cell(2, 2 | hi);

    let mut sprite = |i: usize, x: i16, ctrl: u16| {
        let e = vram + 0x6_0000 + i * 8;
        mem[e..e + 2].copy_from_slice(&x.to_be_bytes());
        mem[e + 4..e + 6].copy_from_slice(&ctrl.to_be_bytes());
    };
    sprite(0, 16, on | 1);
    sprite(1, 30, 1);
    sprite(2, 40, on | 1);

    let mut out = vec![0u32; vdp::WIDTH * vdp::HEIGHT];
    vdp::render(&mem, 255, &mut out);

    assert_eq!(out[7], 0x0011_1111, "V68_HFLIP is not the cell h-flip bit");
    assert_eq!(
        out[7 * vdp::WIDTH + 8],
        0x0011_1111,
        "V68_VFLIP is not the cell v-flip bit"
    );
    assert_eq!(
        out[16], 0x0022_2222,
        "a V68_HI cell must cover a low-priority sprite"
    );
    assert_eq!(out[30], 0, "a sprite without ctrl bit 15 must not render");
    assert_eq!(
        out[40], 0x0011_1111,
        "ctrl bit 15 is not the sprite enable bit"
    );
}

#[test]
fn bios_ram_bounds_match_emulator_abi() {
    let header = repo_file("bios/bios.h");

    let bounds: Vec<(&str, u32)> = vec![
        ("V68_CART_RAM", bus::RAM_BASE + bus::BIOS_PARTITION),
        ("V68_RAM_END", bus::RAM_BASE + bus::RAM_SIZE),
    ];

    for (name, value) in bounds {
        assert_eq!(
            header_const(&header, name),
            value,
            "bios/bios.h: {name} disagrees with emulator"
        );
    }
}

#[test]
fn header_pad_bits_match_emulator_abi() {
    let header = devkit_headers();

    let bits: Vec<(&str, u16)> = vec![
        ("V68_PAD_UP", bus::PAD_UP),
        ("V68_PAD_DOWN", bus::PAD_DOWN),
        ("V68_PAD_LEFT", bus::PAD_LEFT),
        ("V68_PAD_RIGHT", bus::PAD_RIGHT),
        ("V68_PAD_A", bus::PAD_A),
        ("V68_PAD_B", bus::PAD_B),
        ("V68_PAD_X", bus::PAD_X),
        ("V68_PAD_Y", bus::PAD_Y),
        ("V68_PAD_START", bus::PAD_START),
        ("V68_PAD_SELECT", bus::PAD_SELECT),
        ("V68_PAD_L", bus::PAD_L),
        ("V68_PAD_R", bus::PAD_R),
    ];

    for (name, value) in &bits {
        assert_eq!(
            header_const(&header, name),
            *value as u32,
            "devkit: {name} disagrees with emulator"
        );
    }

    let mut seen = 0u16;

    for (name, value) in &bits {
        assert_eq!(value.count_ones(), 1, "{name} is not a single bit");
        assert_eq!(seen & value, 0, "{name} collides with an earlier pad bit");
        seen |= value;
    }
}

fn ld_number(text: &str) -> u32 {
    let text = text.trim();

    let (digits, scale) = match text.chars().last() {
        Some('K') => (&text[..text.len() - 1], 1024),
        Some('M') => (&text[..text.len() - 1], 1024 * 1024),
        _ => (text, 1),
    };

    let value = match digits.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => digits.parse(),
    };

    value.unwrap_or_else(|_| panic!("unparseable linker number {text:?}")) * scale
}

fn ram_region(name: &str) -> (u32, u32) {
    let script = repo_file(name);

    let line = script
        .lines()
        .find(|l| l.trim_start().starts_with("RAM"))
        .unwrap_or_else(|| panic!("{name}: no RAM region in MEMORY block"));

    let (origin, length) = line
        .split_once("ORIGIN =")
        .and_then(|(_, rest)| rest.split_once(", LENGTH ="))
        .unwrap_or_else(|| panic!("{name}: malformed RAM region ({line})"));

    (ld_number(origin), ld_number(length))
}

#[test]
fn linker_scripts_match_emulator_abi() {
    assert_eq!(
        ram_region("bios/bios.ld"),
        (bus::RAM_BASE, bus::BIOS_PARTITION),
        "bios.ld: RAM region must be exactly the BIOS partition"
    );

    assert_eq!(
        ram_region("carts/cart.ld"),
        (
            bus::RAM_BASE + bus::BIOS_PARTITION,
            bus::RAM_SIZE - bus::BIOS_PARTITION
        ),
        "cart.ld: RAM region must be main RAM above the BIOS partition"
    );
}

#[test]
fn xtask_matches_emulator_abi() {
    assert_eq!(xtask::CART_MAX as u32, bus::CART_SIZE);
    assert_eq!(xtask::HEADER_LEN, vega68::cart::HEADER_LEN);
}

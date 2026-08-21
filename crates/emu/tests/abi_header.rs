use vega68::tpu::{self, Tpu};
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

fn rust_const(src: &str, name: &str) -> u32 {
    let line = src
        .lines()
        .find(|l| l.starts_with(&format!("const {name}:")))
        .unwrap_or_else(|| panic!("missing const {name}"));

    let value = line
        .split_once('=')
        .unwrap()
        .1
        .trim()
        .trim_end_matches(';')
        .trim();

    match value.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(&hex.replace('_', ""), 16).unwrap(),
        None => value.parse().unwrap(),
    }
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
        ("V68_VDP_MODE", format!("{:#010X}", bus::VDP_MODE)),
        ("V68_FB_BASE", format!("{:#010X}", bus::FB_BASE)),
        ("V68_PALETTE", format!("{:#010X}", bus::PALETTE_BASE)),
        ("V68_TPU_RAM", format!("{:#010X}", bus::TPU_RAM_BASE)),
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
    let bus_src = repo_file("crates/emu/src/bus.rs");
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
        ("V68_TPU_RAM_SIZE", bus::TPU_RAM_SIZE),
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
fn tpu_command_encoding_matches_emulator_abi() {
    let header = devkit_headers();
    let tpu_src = repo_file("crates/emu/src/tpu.rs");

    let pins: Vec<(&str, &str)> = vec![
        ("V68_TPU_OP_TRI", "OP_TRI"),
        ("V68_TPU_OP_FILL", "OP_FILL"),
        ("V68_TPU_TRI_WORDS", "TRI_WORDS"),
        ("V68_TPU_FILL_WORDS", "FILL_WORDS"),
        ("V68_TRI_BLEND", "TRI_BLEND"),
        ("V68_TRI_ZGREATER", "TRI_ZGREATER"),
        ("V68_TRI_ZTEST_OFF", "TRI_ZTEST_OFF"),
        ("V68_TRI_ZWRITE_OFF", "TRI_ZWRITE_OFF"),
        ("V68_FILL_COLOR", "FILL_COLOR"),
        ("V68_FILL_Z", "FILL_Z"),
    ];

    for (define, name) in pins {
        assert_eq!(
            header_const(&header, define),
            rust_const(&tpu_src, name),
            "devkit: {define} disagrees with tpu.rs's {name}"
        );
    }

    assert_eq!(
        header_const(&header, "V68_TPU_BUSY"),
        bus::TPU_BUSY as u32,
        "devkit: V68_TPU_BUSY disagrees with bus::TPU_BUSY"
    );
}

#[test]
fn tpu_state_block_offsets_match_emulator_abi() {
    const COLOR: u32 = 0x1000;
    const RING: u32 = 0x100;
    const Z: u32 = 0x2000;
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 8;

    let base = bus::TPU_RAM_BASE as usize;
    let mut mem = vec![0u8; bus::MEM_END as usize];

    let mut put = |at: u32, bytes: &[u8]| {
        mem[base + at as usize..][..bytes.len()].copy_from_slice(bytes);
    };

    put(0, &RING.to_be_bytes());
    put(4, &8u32.to_be_bytes());
    put(8, &COLOR.to_be_bytes());
    put(12, &Z.to_be_bytes());
    put(16, &(WIDTH as u16).to_be_bytes());
    put(18, &(HEIGHT as u16).to_be_bytes());

    for (i, w) in [0x0200_0003u32, 0x0001_0001, 0x0003_0003, 7, 0xBEEF]
        .into_iter()
        .enumerate()
    {
        put(RING + i as u32 * 4, &w.to_be_bytes());
    }

    let mut t = Tpu::new();
    t.tail = 5;
    tpu::run(&mut t, &mut mem);

    let at = (WIDTH + 1) as usize;

    assert_eq!(
        mem[base + COLOR as usize + at],
        7,
        "the FILL never reached the colour target the header's offsets describe"
    );
    assert_eq!(
        mem[base + Z as usize + at * 2..][..2],
        [0xBE, 0xEF],
        "the FILL never reached the z target the header's offsets describe"
    );
}

#[test]
fn vdp_mode_bits_match_emulator_abi() {
    let header = devkit_headers();
    let hires = header_const(&header, "V68_MODE_HIRES") as u16;
    let fb = header_const(&header, "V68_MODE_FB") as u16;
    let mut mem = vec![0u8; bus::MEM_END as usize];

    mem[bus::PALETTE_BASE as usize + 4..][..4].copy_from_slice(&0x00AB_CDEFu32.to_be_bytes());
    mem[bus::TPU_RAM_BASE as usize] = 1;

    mem[bus::VDP_MODE as usize..][..2].copy_from_slice(&hires.to_be_bytes());
    assert_eq!(
        vdp::mode(&mem),
        (vdp::WIDTH * 2, vdp::HEIGHT * 2),
        "V68_MODE_HIRES is not the bit that doubles the output resolution"
    );

    mem[bus::VDP_MODE as usize..][..2].copy_from_slice(&fb.to_be_bytes());
    let mut out = vec![0u32; vdp::WIDTH * vdp::HEIGHT];
    vdp::render(&mem, 255, &mut out);

    assert_eq!(
        vdp::mode(&mem),
        (vdp::WIDTH, vdp::HEIGHT),
        "V68_MODE_FB must not change the output resolution"
    );
    assert_eq!(
        out[0], 0x00AB_CDEF,
        "V68_MODE_FB is not the bit that paints the framebuffer plane"
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

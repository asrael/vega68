mod common;

use common::{bios_rom, build_dir};
use vega68::System;
use vega68::vdp::{HEIGHT, WIDTH};

const BG: u32 = 0x0010_2040;
const BOOT_FRAMES: usize = 1;
const FG: u32 = 0x00FF_FFFF;

fn assert_displays(bios: &[u8], file: &[u8]) {
    let mut sys = System::new(bios, file).unwrap();
    let mut frame = vec![0u32; WIDTH * HEIGHT];

    for _ in 0..BOOT_FRAMES {
        sys.run_frame();
    }

    sys.render(&mut frame);

    assert_eq!(frame[0], BG, "backdrop");
    assert_eq!(frame[88 * WIDTH + 113], FG, "glyph stroke");
    assert_eq!(frame[(11 * 8 + 7) * WIDTH + 113], BG, "blank row under it");
}

#[test]
fn hello_cart_displays() {
    let Some((bios, file)) = build_dir("carts/hello") else {
        return;
    };

    assert_displays(&bios, &file);
}

#[test]
fn hello_cart_displays_on_the_bios_rom() {
    let Some((_, file)) = build_dir("carts/hello") else {
        return;
    };

    assert_displays(&bios_rom(), &file);
}

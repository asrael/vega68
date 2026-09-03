mod common;

use std::path::Path;

use vega68::System;
use vega68::bus::LINES_PER_FRAME;
use vega68::system::LINE_CYCLES;

#[test]
#[ignore = "measurement, not a check: cargo test -p vega68 --test calibrate -- --ignored --nocapture"]
fn instructions_per_line_on_the_demo_and_sound_carts() {
    let Some(bios) = common::build_bios() else {
        return;
    };
    let root = xtask::repo_root().unwrap();
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let demo = xtask::build_cart(&root.join("carts/demo"), tmp).unwrap();
    let sound = xtask::build_cart(&root.join("host/emu/tests/fixtures/sound"), tmp).unwrap();

    for (name, path, frames) in [("demo", demo, 600), ("sound", sound, 120)] {
        let mut sys = System::new(&bios, &std::fs::read(path).unwrap()).unwrap();

        for _ in 0..frames {
            sys.run_frame();
        }

        let lines = (frames * LINES_PER_FRAME) as f64;
        let per_line = sys.cpu.retired as f64 / lines;
        let cpi = LINE_CYCLES as f64 / per_line;

        println!("{name}: {per_line:.0} instructions/line, {cpi:.2} cycles/instruction");
    }
}

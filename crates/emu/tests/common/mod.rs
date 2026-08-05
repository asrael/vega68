#![allow(dead_code)] // each test binary pulls in only the part of the harness it needs

use vega68::System;

use std::path::Path;
use std::process::Command;

pub const DONE: u8 = 0x04;
pub const MAX_FRAMES: usize = 1000;

const TOOLS: [&str; 1] = ["m68k-elf-gcc"];

pub fn assert_cart(fixture: &str, expected: &str) -> Option<System> {
    let (bios, file) = build(fixture)?;
    let mut sys = System::new(&bios, &file).unwrap();

    for frame in 0..MAX_FRAMES {
        sys.run_frame();

        if sys.bus.debug_out.contains(&DONE) {
            break;
        }

        assert!(
            frame + 1 < MAX_FRAMES,
            "{fixture} did not finish in {MAX_FRAMES} frames; output so far:\n{}",
            String::from_utf8_lossy(&sys.bus.debug_out)
        );
    }

    let mut want = expected.as_bytes().to_vec();
    want.push(DONE);

    let cut = sys.bus.debug_out.iter().position(|&b| b == DONE).unwrap() + 1;

    assert_eq!(
        String::from_utf8_lossy(&sys.bus.debug_out[..cut]),
        String::from_utf8_lossy(&want)
    );

    Some(sys)
}

pub fn bios_section(name: &str) -> (u32, u32, u32) {
    xtask::bios_section(name).unwrap()
}

pub fn bios_symbol(name: &str) -> u32 {
    xtask::bios_symbol(name).unwrap()
}

pub fn build(fixture: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    build_dir(&format!("crates/emu/tests/fixtures/{fixture}"))
}

pub fn build_dir(rel: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let bios = build_bios()?;
    let dir = xtask::repo_root().unwrap().join(rel);
    let v68 = xtask::build_cart(&dir, Path::new(env!("CARGO_TARGET_TMPDIR"))).unwrap();

    Some((bios, std::fs::read(&v68).unwrap()))
}

pub fn bios_rom() -> Vec<u8> {
    let rom = xtask::repo_root().unwrap().join("bios/vega68.rom");

    std::fs::read(&rom).unwrap_or_else(|e| panic!("{}: {e}", rom.display()))
}

pub fn build_bios() -> Option<Vec<u8>> {
    for tool in TOOLS {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("skipping: {tool} not on PATH");
            return None;
        }
    }

    Some(std::fs::read(xtask::build_bios().unwrap()).unwrap())
}

pub fn run_until(sys: &mut System, from: usize, needle: &str) {
    let want = needle.as_bytes();

    for frame in 0..MAX_FRAMES {
        sys.run_frame();

        let seen = &sys.bus.debug_out[from..];

        if seen.windows(want.len()).any(|w| w == want) {
            return;
        }

        assert!(
            frame + 1 < MAX_FRAMES,
            "{needle:?} never appeared in {MAX_FRAMES} frames; output so far:\n{}",
            String::from_utf8_lossy(&sys.bus.debug_out)
        );
    }
}

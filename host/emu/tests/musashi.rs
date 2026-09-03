use std::path::Path;

use vega68::bus::Bus;
use vega68::cpu::Cpu;

const ENTRY: usize = 0x1_0000;
const FAIL_REG: usize = 0x10_0000;
const PASS_REG: usize = 0x10_0004;
const SLICE: u32 = 1_000_000;
const SLICES: u32 = 200;
const SKIP: [&str; 1] = ["interrupt"]; // wants the dropped C driver's interrupt register

fn run(bin: &Path) -> Result<(), &'static str> {
    let image = std::fs::read(bin).unwrap();
    let mut bios = vec![0u8; ENTRY + image.len()];

    for slot in bios[..0x100].chunks_mut(4) {
        slot.copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
    }

    bios[0..4].copy_from_slice(&0x3F0u32.to_be_bytes());
    bios[4..8].copy_from_slice(&(ENTRY as u32).to_be_bytes());
    bios[ENTRY..].copy_from_slice(&image);

    let mut bus = Bus::new(bios);
    let mut cpu = Cpu::new();

    bus.mem[FAIL_REG..FAIL_REG + 4].fill(0xFF);
    cpu.reset(&mut bus);

    for _ in 0..SLICES {
        cpu.run(&mut bus, SLICE);

        if bus.mem[FAIL_REG..FAIL_REG + 4] == [0; 4] {
            return Err("TEST_FAIL_REG written");
        }

        if bus.mem[PASS_REG..PASS_REG + 4] == [0, 0, 0, 1] {
            return Ok(());
        }
    }

    Err("no verdict")
}

#[test]
fn every_upstream_test_image_passes_on_the_lc040() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../musashi/test");
    let mut ran = 0;

    for dir in ["mc68000", "mc68040"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_stem().unwrap().to_str().unwrap();

            if path.extension().and_then(|e| e.to_str()) != Some("bin") || SKIP.contains(&name) {
                continue;
            }

            run(&path).unwrap_or_else(|e| panic!("{dir}/{name}: {e}"));
            ran += 1;
        }
    }

    assert_eq!(
        ran, 77,
        "an image went missing or was added without updating this count"
    );
}

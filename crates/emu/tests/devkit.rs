mod common;

use common::assert_cart;
use vega68::bus::VRAM_BASE;

#[test]
fn audio_devkit_keys_a_patch_and_the_mix_matches_the_golden() {
    let Some(mut sys) = assert_cart("audio", "ok\n") else {
        return;
    };

    sys.run_frame();

    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for s in &sys.bus.apu.frame {
        for b in s.to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    assert!(
        sys.bus.apu.frame.iter().any(|&s| s != 0),
        "fixture produced silence"
    );
    assert_eq!(
        h, 0xfbec_9276_3545_378d,
        "audio frame drifted from the golden"
    );
}

#[test]
fn irq_devkit_installs_both_sources_and_reports_the_measured_cadence() {
    let Some(sys) = assert_cart("irq", "ok\n") else {
        return;
    };

    assert_eq!(sys.bus.mem[VRAM_BASE as usize + 4], 70, "fire count");
    assert_eq!(sys.bus.mem[VRAM_BASE as usize + 5], 40, "first line seen");
    assert_eq!(sys.bus.mem[VRAM_BASE as usize + 6], 178, "last line seen");
}

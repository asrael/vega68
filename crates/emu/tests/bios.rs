mod common;

use common::{assert_cart, bios_symbol, build, build_bios, run_until};
use vega68::System;
use vega68::bus::{PAD_DOWN, PAD_L, PAD_START};

#[test]
fn reset_restarts_the_cart_window_three_times() {
    let Some(sys) = assert_cart("reset", "boot\nboot\nboot\nok\n") else {
        return;
    };

    assert_eq!(
        sys.bus.mem[bios_symbol("v68_reset_reason") as usize],
        1,
        "the warm-boot flag did not survive the crt0 re-entry"
    );
}

#[test]
fn reset_restores_the_vectors_and_disables_interrupts() {
    let Some(sys) = assert_cart("reset", "boot\nboot\nboot\nok\n") else {
        return;
    };

    let stub = bios_symbol("v68_rte_stub");
    let vector = |at: usize| u32::from_be_bytes(sys.bus.mem[at..at + 4].try_into().unwrap());

    assert_eq!(
        vector(0x78),
        stub,
        "reset left the cart's vblank handler installed"
    );
    assert_eq!(
        vector(0x70),
        stub,
        "reset left the cart's line handler installed"
    );
    assert_eq!(
        sys.bus.irq_enable, 0,
        "reset left an interrupt source enabled"
    );
}

#[test]
fn the_reset_reason_survives_the_crt0_clear_range() {
    if build_bios().is_none() {
        return;
    }

    let bss_end = bios_symbol("__bss_end");
    let flag = bios_symbol("v68_reset_reason");

    assert!(
        flag >= bss_end,
        "v68_reset_reason is at {flag:#010x}, inside the crt0 clear range \
         (ends {bss_end:#010x}) -- .noinit must be placed below .bss in bios.ld"
    );
}

#[test]
fn every_baked_vector_slot_is_filled() {
    let Some(bios) = build_bios() else {
        return;
    };

    let filled = |range: std::ops::Range<usize>, sym: &str| {
        let want = bios_symbol(sym).to_be_bytes();

        assert!(
            bios[range.clone()].chunks(4).all(|slot| slot == want),
            "vectors {:#x}..{:#x} are not all {sym}: {:02x?}",
            range.start / 4,
            range.end / 4,
            &bios[range]
        );
    };

    filled(8..0x64, "v68_fault");
    filled(0x64..0x80, "v68_rte_stub");
}

#[test]
fn an_illegal_instruction_reaches_the_baked_fault_vector() {
    let Some((bios, image)) = build("fault") else {
        return;
    };

    let mut sys = System::new(&bios, &image).unwrap();

    run_until(&mut sys, 0, "fault");

    let out = String::from_utf8_lossy(&sys.bus.debug_out).into_owned();

    assert!(out.starts_with("pre\n"), "output:\n{out}");
    assert!(out.contains("pc="), "no frame dump:\n{out}");
    assert!(
        !out.contains("post"),
        "execution resumed past the fault:\n{out}"
    );

    let mark = sys.bus.debug_out.len();

    for _ in 0..4 {
        sys.run_frame();
    }

    assert!(
        !sys.bus.debug_out[mark..].contains(&b'!'),
        "the cart's vblank handler kept running through the post-mortem"
    );
}

#[test]
fn the_monitor_freezes_a_cart_that_returns() {
    let Some((bios, image)) = build("terminal") else {
        return;
    };

    let mut sys = System::new(&bios, &image).unwrap();

    run_until(&mut sys, 0, "cart returned");

    for _ in 0..4 {
        sys.run_frame();
    }

    assert_eq!(
        String::from_utf8_lossy(&sys.bus.debug_out),
        "\u{4}vega68: cart returned\n"
    );
}

#[test]
fn the_monitor_answers_the_pad_and_resets_on_start() {
    let Some((bios, image)) = build("fault") else {
        return;
    };

    let mut sys = System::new(&bios, &image).unwrap();

    run_until(&mut sys, 0, "fault");

    let tail = |sys: &mut System, pad: u16| {
        let mark = sys.bus.debug_out.len();

        sys.bus.pads[0] = pad;
        sys.run_frame();
        sys.bus.pads[0] = 0;
        sys.run_frame();

        String::from_utf8_lossy(&sys.bus.debug_out[mark..]).into_owned()
    };

    assert!(
        tail(&mut sys, PAD_DOWN).contains("\n02000010:"),
        "down did not scroll the window"
    );
    assert!(
        tail(&mut sys, PAD_L).contains("\n02020000:"),
        "L did not jump to the next preset"
    );

    let mark = sys.bus.debug_out.len();

    sys.bus.pads[0] = PAD_START;
    sys.run_frame();
    sys.bus.pads[0] = 0;
    run_until(&mut sys, mark, "pre\n");

    let resets = String::from_utf8_lossy(&sys.bus.debug_out[mark..])
        .matches("pre\n")
        .count();

    assert_eq!(resets, 1, "holding start reset the machine repeatedly");
}

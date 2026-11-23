mod common;

use common::{assert_cart, build, fnv1a64, run_until};
use vega68::bus::{PAD_A, VRAM_BASE};
use vega68::{System, vdp};

#[test]
fn audio_devkit_keys_a_patch_and_the_mix_matches_the_golden() {
    let Some(mut sys) = assert_cart("audio", "ok\n") else {
        return;
    };

    sys.run_frame();

    assert!(
        sys.bus.apu.frame.iter().any(|&s| s != 0),
        "fixture produced silence"
    );
    assert_eq!(
        fnv1a64(&sys.bus.apu.frame),
        0x0bee_778e_e388_3045,
        "audio frame drifted from the golden"
    );
}

#[test]
fn sound_devkit_plays_a_two_bar_song_and_the_mix_matches_the_golden() {
    let Some(mut sys) = assert_cart("sound", "ok\n") else {
        return;
    };

    sys.run_frame();

    let status = ((sys.bus.apu.read(0x402) as u16) << 8) | sys.bus.apu.read(0x403) as u16;
    assert!(
        status & 0x0003 != 0,
        "neither lead nor bass channel is audible after the loop"
    );

    let peak = sys.bus.apu.frame.iter().map(|&s| s.abs()).max().unwrap();
    assert!(
        peak > 300,
        "playback is technically nonzero but inaudible (peak {peak})"
    );
    assert_eq!(
        fnv1a64(&sys.bus.apu.frame),
        0xfc7c_9e33_2281_86c5,
        "sound frame drifted from the golden"
    );
}

#[test]
fn sound_parser_accepts_the_grammar_and_rejects_bad_tokens_with_diagnostics() {
    let Some(_) = assert_cart(
        "sound_parse",
        "parse ok\nsound: s0 t0 bar0: bad token 'x9'\nsound: s0 t0 bar0: bad token '-'\nsound: s0 t0 bar0: bad token 'c1'\nsound: s0 t0 bar0: bad token '~'\nsound: s0 t0 bar0: empty group\nsound: s0 t0 bar0: empty group\nsound: bad channel\nsound: bad patch\nsound: s0 t0 bar0: bad token 'a2@0'\nsound: s0 t0 bar0: bad token '[c3 d3'\nsound: s0: uneven tracks\nsound: bad loop section\nrejects ok\n",
    ) else {
        return;
    };
}

#[test]
fn sound_patches_devkit_keys_all_twelve_presets_audible_and_distinct() {
    let Some((bios, file)) = build("sound_patches") else {
        return;
    };
    let mut sys = System::new(&bios, &file).unwrap();

    run_until(&mut sys, 0, "ok\n");
    let out = String::from_utf8_lossy(&sys.bus.debug_out).into_owned();
    let cut = out.find("ok\n").expect("run_until guarantees this") + "ok\n".len();

    let lines: Vec<&str> = out[..cut].lines().collect();
    assert_eq!(
        lines.len(),
        13,
        "expected 12 preset lines + ok, got:\n{out}"
    );
    assert_eq!(lines[12], "ok", "transcript did not end in ok:\n{out}");

    let mut fingerprints = Vec::new();
    for (i, line) in lines[..12].iter().enumerate() {
        assert!(line.ends_with(" on"), "preset {i} never keyed on: {line:?}");

        let want_prefix = format!("p{i} ");
        assert!(
            line.starts_with(&want_prefix),
            "preset {i} line out of order: {line:?}"
        );

        let bytes: Vec<u8> = line[want_prefix.len()..]
            .trim_end_matches(" on")
            .split(' ')
            .map(|tok| u8::from_str_radix(tok, 16).unwrap_or_else(|e| panic!("{tok:?}: {e}")))
            .collect();
        assert_eq!(bytes.len(), 5, "preset {i} tuple malformed: {line:?}");

        fingerprints.push(bytes);
    }

    for i in 0..fingerprints.len() {
        for j in (i + 1)..fingerprints.len() {
            assert_ne!(
                fingerprints[i], fingerprints[j],
                "presets {i} and {j} share a DT/MUL/algorithm fingerprint: {:?}",
                fingerprints[i]
            );
        }
    }
}

#[test]
fn canvas_devkit_plots_pixels_through_the_tile_swizzle() {
    let Some(mut sys) = assert_cart("canvas", "ok\n") else {
        return;
    };

    sys.run_frame(); // the beam pass over the finished drawing

    let mut out = vec![0u32; vdp::LO.w * vdp::LO.h];
    sys.render(&mut out);

    assert_eq!(out[0], 0x00FF_0000, "plot at (0,0) never reached the frame");
    assert_eq!(
        out[179 * vdp::LO.w + 319],
        0x0000_FF00,
        "plot at (319,179) never reached the frame"
    );

    for i in 0..32 {
        assert_eq!(
            out[(4 + i) * vdp::LO.w + 4 + i],
            0x0000_00FF,
            "diagonal broke crossing a cell boundary at step {i}"
        );
    }

    for (x, y) in [(100, 50), (129, 50), (100, 69), (129, 69), (115, 60)] {
        assert_eq!(
            out[y * vdp::LO.w + x],
            0x00FF_FF00,
            "fill missed ({x},{y}) inside its rect"
        );
    }

    for (x, y) in [(99, 50), (130, 50), (100, 49), (100, 70)] {
        assert_eq!(
            out[y * vdp::LO.w + x],
            0,
            "fill bled to ({x},{y}) outside its rect"
        );
    }

    assert_eq!(out[1], 0, "an unplotted pixel lost the backdrop");
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

#[test]
fn hires_devkit_draws_at_640x360_with_16_px_tiles() {
    let Some(mut sys) = assert_cart("hires", "ok\n") else {
        return;
    };

    sys.run_frame();
    assert_eq!(sys.mode().w, 640, "v68_mode did not reach the next frame");

    let mut out = vec![0u32; vdp::HI.w * vdp::HI.h];
    sys.render(&mut out);
    let at = |x: usize, y: usize| out[y * vdp::HI.w + x];

    assert_eq!(at(0, 0), 0x00FF_0000);
    assert_eq!(
        at(639, 359),
        0x0000_FF00,
        "canvas does not reach the hi-res corner"
    );

    for i in 0..32 {
        assert_eq!(at(4 + i, 4 + i), 0x0000_00FF, "diagonal broke at step {i}");
    }

    for (x, y) in [(200, 100), (229, 119), (215, 110)] {
        assert_eq!(at(x, y), 0x00FF_FF00);
    }

    assert_ne!(at(230, 100), 0x00FF_FF00, "fill bled right");
    assert_ne!(at(200, 120), 0x00FF_FF00, "fill bled down");

    assert_eq!(
        at(347, 247),
        0x00FF_FFFF,
        "a 40 px sprite did not round up to 48"
    );
    assert_ne!(at(348, 247), 0x00FF_FFFF);
    assert_ne!(
        at(347, 248),
        0x00FF_FFFF,
        "a 40 px sprite grew past 48 rows"
    );

    sys.bus.pads[0] = PAD_A;

    for _ in 0..4 {
        sys.run_frame();
    }

    assert_eq!(
        sys.mode().w,
        320,
        "holding A did not switch to lo-res exactly once"
    );
}

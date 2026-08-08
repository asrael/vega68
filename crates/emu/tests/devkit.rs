mod common;

use common::{assert_cart, build, fnv1a64, fnv1a64_pixels, run_until};
use vega68::bus::VRAM_BASE;
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
    // devkit/vega68_sfx.c links into every cart (including this one, which never
    // calls it) — its static pool is part of this cart's BSS. If this golden moves
    // after resizing that pool, that's the layout shift to check first, not a mix bug.
    assert_eq!(
        fnv1a64(&sys.bus.apu.frame),
        0xb87e_5b5a_725c_2fe5,
        "audio frame drifted from the golden"
    );
}

#[test]
fn sound_devkit_plays_a_two_bar_song_and_the_mix_matches_the_golden() {
    let Some(mut sys) = assert_cart("sound", "ok\n") else {
        return;
    };

    sys.run_frame(); // fixture halted in while(true) after "ok": one more frame of decay past the last dispatched event, not a further song tick

    let status = ((sys.bus.apu.read(0x402) as u16) << 8) | sys.bus.apu.read(0x403) as u16;
    assert!(
        status & 0x0003 != 0,
        "neither lead nor bass channel is audible after the loop"
    );

    // Level floor, not a golden: nonzero-but-inaudible output has shipped
    // once already (gcc's aliased post-increment miscompile shifted every
    // patch byte one register late, leaving peaks near -68 dBFS).
    let peak = sys.bus.apu.frame.iter().map(|&s| s.abs()).max().unwrap();
    assert!(
        peak > 300,
        "playback is technically nonzero but inaudible (peak {peak})"
    );
    assert_eq!(
        fnv1a64(&sys.bus.apu.frame),
        0x172a_84ff_bae3_227d,
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
        assert!(
            line.ends_with(" on"),
            "preset {i} never keyed on: {line:?}"
        );

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
fn tpu_devkit_rasterises_two_triangles_and_the_frame_matches_the_golden() {
    // The hex word is the fragment counter read after the tail register drained,
    // before the next frame start zeroes it: 14,400 for the quarter-cost
    // full-target FILL, 9,500 covered fragments for the textured triangle
    // (exactly its area) and 7,810 for the blended one (area 7,800).
    let Some(sys) = assert_cart("tpu", "00007bde\nok\n") else {
        return;
    };

    assert_eq!(
        vdp::mode(&sys.bus.mem),
        (vdp::WIDTH, vdp::HEIGHT),
        "fixture must display through lo-res + TPU_PLANE"
    );

    let mut out = vec![0u32; vdp::WIDTH * vdp::HEIGHT];
    sys.render(&mut out);

    // Covers the composited 320x180 output, which under TPU_PLANE is the
    // colour target mapped through the cart's palette and nothing else. The
    // fixture's tables put every feature in its own index band, so a drift
    // localises: 1 is the FILL background (41,386 px), 16..=26 the textured
    // triangle's level-0 texels shifted by its dithered colormap shades
    // (8,404 px visible), 138..=145 the blended triangle's level-1 texels
    // through the blend table (7,810 px, of which 1,096 land over the
    // textured one, which is also the z-test's only observable seam).
    assert_eq!(
        fnv1a64_pixels(&out),
        0x8aaa_8af3_619e_c60a,
        "rendered frame drifted from the golden"
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

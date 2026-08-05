mod common;

use common::build_dir;
use vega68::System;
use vega68::bus::{PAD_A, PAD_RIGHT};
use vega68::vdp::{HEIGHT, WIDTH};

const BACKDROP: u32 = 0x0010_2040; // palette 0
const BACKDROP_PX: usize = 10 * WIDTH + 10;
const CHECKER: u32 = 0x001A_3050; // palette 1
const CHECKER_PX: usize = 10 * WIDTH + 14;
const DIM_FRAMES: u8 = 4;
const FADE_CAP: usize = 34;
const HANDOFF_CAP: usize = 5;
const HOME_X: u16 = 152;
const HOME_Y: u16 = 82;
const MOVE_FRAMES: u16 = 5;
const SETUP_CAP: usize = 4;
const SPRITE_BODY: u32 = 0x00FF_8000; // palette 2
const SPRITE_EDGE: u32 = 0x00FF_FFFF; // palette 3

fn body_px(frame: &[u32], x: u16, y: u16) -> u32 {
    frame[(y as usize + 8) * WIDTH + x as usize + 8]
}

fn edge_px(frame: &[u32], x: u16, y: u16) -> u32 {
    frame[y as usize * WIDTH + x as usize]
}

fn reg(sys: &System, addr: usize) -> u16 {
    u16::from_be_bytes([sys.bus.mem[addr], sys.bus.mem[addr + 1]])
}

fn scroll_h(sys: &System) -> u16 {
    reg(sys, 0x0306_1000)
}

fn sprite_x(sys: &System) -> u16 {
    reg(sys, 0x0306_0000)
}

fn sprite_y(sys: &System) -> u16 {
    reg(sys, 0x0306_0002)
}

#[test]
fn demo_cart_fades_in_then_reacts_to_input() {
    let Some((bios, file)) = build_dir("carts/demo") else {
        return;
    };

    let mut sys = System::new(&bios, &file).unwrap();
    let mut frame = vec![0u32; WIDTH * HEIGHT];

    let mut booted = false;

    for _ in 0..SETUP_CAP {
        sys.run_frame();

        if sprite_x(&sys) == HOME_X {
            booted = true;
            break;
        }
    }

    assert!(
        booted,
        "setup never parked the sprite in {SETUP_CAP} frames"
    );
    sys.render(&mut frame);

    assert_eq!(sprite_y(&sys), HOME_Y, "sprite y after setup");
    assert_eq!(frame[BACKDROP_PX], BACKDROP, "backdrop already full");
    assert_eq!(frame[CHECKER_PX], CHECKER, "checkerboard already full");

    let mut last = body_px(&frame, HOME_X, HOME_Y);

    assert!(last < SPRITE_BODY, "sprite still fading: {last:08X}");

    let mut faded = false;

    for f in 1..=FADE_CAP {
        sys.run_frame();
        sys.render(&mut frame);

        let px = body_px(&frame, HOME_X, HOME_Y);

        assert_eq!(sys.bus.brightness, 255, "brightness at fade frame {f}");
        assert_eq!(frame[BACKDROP_PX], BACKDROP, "backdrop at fade frame {f}");
        assert_eq!(frame[CHECKER_PX], CHECKER, "checkerboard at fade frame {f}");
        assert!(px > last, "fade stalled at {f}: {last:08X} -> {px:08X}");
        last = px;

        if px == SPRITE_BODY {
            faded = true;
            break;
        }
    }

    assert!(faded, "fade never completed in {FADE_CAP} frames");

    assert_eq!(
        edge_px(&frame, HOME_X, HOME_Y),
        SPRITE_EDGE,
        "sprite outline at full colour"
    );

    let mut alive = false;

    for _ in 0..HANDOFF_CAP {
        sys.run_frame();

        if scroll_h(&sys) != 0 {
            alive = true;
            break;
        }
    }

    assert!(alive, "game loop never scrolled in {HANDOFF_CAP} frames");
    assert_eq!(sprite_x(&sys), HOME_X, "sprite has not moved yet");
    assert_eq!(sprite_y(&sys), HOME_Y, "sprite has not moved yet");

    sys.bus.pads[0] = PAD_RIGHT;

    for _ in 0..MOVE_FRAMES {
        sys.run_frame();
    }

    let moved = HOME_X + 2 * MOVE_FRAMES;

    assert_eq!(sprite_x(&sys), moved, "sprite x after {MOVE_FRAMES} frames");
    assert_eq!(sprite_y(&sys), HOME_Y, "sprite y must not drift");

    sys.render(&mut frame);

    assert_eq!(
        body_px(&frame, moved, HOME_Y),
        SPRITE_BODY,
        "sprite body renders at its new position"
    );

    sys.bus.pads[0] = PAD_A;

    for _ in 0..DIM_FRAMES {
        sys.run_frame();
    }

    assert_eq!(
        sys.bus.brightness,
        255 - 4 * DIM_FRAMES,
        "PAD_A dims by 4/frame"
    );
}

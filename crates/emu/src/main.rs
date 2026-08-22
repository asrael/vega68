use vega68::System;
use vega68::apu::out::AudioOut;
use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gilrs::{Axis, Button, EventType, Gilrs};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const BIOS: &[u8] = include_bytes!("../../../bios/vega68.rom");
const FRAME: Duration = Duration::from_nanos(16_666_667);
const STICK_LOOK: f64 = 24.0;
const LOOK_SMOOTH: f64 = 0.5;

const MAX_SCALE: usize = 6;
const FALLBACK_SCALE: usize = 4;

fn centre(origin: (i32, i32), monitor: (u32, u32), window: (u32, u32)) -> (i32, i32) {
    (
        origin.0 + (monitor.0 as i32 - window.0 as i32) / 2,
        origin.1 + (monitor.1 as i32 - window.1 as i32) / 2,
    )
}

fn auto_scale(monitor: Option<(u32, u32)>) -> usize {
    let Some((w, h)) = monitor else {
        return FALLBACK_SCALE;
    };

    (w as usize / WIDTH)
        .min(h as usize / HEIGHT)
        .clamp(1, MAX_SCALE)
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

struct Watch {
    bytes: Vec<u8>,
    mtime: Option<SystemTime>,
    path: String,
}

impl Watch {
    fn new(path: String, bytes: Vec<u8>) -> Self {
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        Watch { bytes, mtime, path }
    }

    fn poll(&mut self, sys: &mut System) {
        let Some(mtime) = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok()
        else {
            return;
        };

        if Some(mtime) == self.mtime {
            return;
        }
        self.mtime = Some(mtime);

        let bytes = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("vega68: watch: failed to read {}: {e}", self.path);
                return;
            }
        };

        if bytes == self.bytes {
            return;
        }

        match sys.reload(&bytes) {
            Ok(()) => {
                eprintln!("vega68: reloaded {}", self.path);
                self.bytes = bytes;
            }
            Err(e) => eprintln!("vega68: watch: {} is not a valid cart: {e}", self.path),
        }
    }
}

fn run_headless(mut sys: System, frames: u64, mut watch: Option<Watch>) {
    let mut frame = vec![0u32; WIDTH * HEIGHT];

    for i in 0..frames {
        if let Some(w) = &mut watch {
            w.poll(&mut sys);
        }

        sys.run_frame();
        sys.render(&mut frame);
        println!(
            "frame {i} {:016x}",
            vega68::fnv1a64(frame.iter().flat_map(|p| p.to_le_bytes()))
        );
    }
}

fn usage() -> ! {
    eprintln!("usage: vega68 <cart.v68> [--headless N] [--scale K] [--watch]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cart_path = None;
    let mut headless = None;
    let mut scale = None;
    let mut watch = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                headless = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--scale" => {
                scale = Some(
                    args.next()
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or_else(|| usage())
                        .max(1),
                )
            }
            "--watch" => watch = true,
            _ if cart_path.is_none() => cart_path = Some(arg),
            _ => usage(),
        }
    }

    let cart_path = cart_path.unwrap_or_else(|| usage());
    let file = std::fs::read(&cart_path)
        .unwrap_or_else(|e| die(&format!("failed to read {cart_path}: {e}")));
    let sys = System::new(BIOS, &file)
        .unwrap_or_else(|e| die(&format!("{cart_path} is not a valid cart: {e}")));
    let watch = watch.then(|| Watch::new(cart_path, file));

    match headless {
        Some(frames) => run_headless(sys, frames, watch),
        None => run_windowed(sys, scale, watch),
    }
}

struct Vega68 {
    audio: Option<AudioOut>,
    captured: bool,
    frame: Vec<u32>,
    gamepad: u16,
    gilrs: Option<Gilrs>,
    initial_scale: Option<usize>,
    look: (f64, f64),
    next_frame: Instant,
    pad: u16,
    rstick: (f64, f64),
    stick: u16,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    sys: System,
    watch: Option<Watch>,
    window: Option<Rc<Window>>,
}

fn blit(frame: &[u32], out: &mut [u32], surface_w: usize, surface_h: usize) {
    let (scale, ox, oy) = fit(surface_w, surface_h);
    let img_w = (WIDTH * scale).min(surface_w - ox);
    let img_h = (HEIGHT * scale).min(surface_h - oy);

    out[..oy * surface_w].fill(0);
    out[(oy + img_h) * surface_w..].fill(0);

    for y in 0..img_h {
        let row = &mut out[(oy + y) * surface_w..][..surface_w];

        row[..ox].fill(0);
        row[ox + img_w..].fill(0);

        let src = &frame[(y / scale) * WIDTH..][..WIDTH];
        let dst = &mut row[ox..][..img_w];
        let n_full = img_w / scale;
        let (chunked, tail) = dst.split_at_mut(n_full * scale);

        for (v, chunk) in src[..n_full].iter().zip(chunked.chunks_exact_mut(scale)) {
            chunk.fill(*v);
        }

        if !tail.is_empty() {
            tail.fill(src[n_full]);
        }
    }
}

fn fit(surface_w: usize, surface_h: usize) -> (usize, usize, usize) {
    let scale = (surface_w / WIDTH).min(surface_h / HEIGHT).max(1);
    let ox = surface_w.saturating_sub(WIDTH * scale) / 2;
    let oy = surface_h.saturating_sub(HEIGHT * scale) / 2;

    (scale, ox, oy)
}

fn gamepad_bit(button: Button) -> Option<u16> {
    Some(match button {
        Button::DPadUp => bus::PAD_UP,
        Button::DPadDown => bus::PAD_DOWN,
        Button::DPadLeft => bus::PAD_LEFT,
        Button::DPadRight => bus::PAD_RIGHT,
        Button::East => bus::PAD_A,
        Button::South => bus::PAD_B,
        Button::North => bus::PAD_X,
        Button::West => bus::PAD_Y,
        Button::Start => bus::PAD_START,
        Button::Select => bus::PAD_SELECT,
        Button::LeftTrigger => bus::PAD_L,
        Button::RightTrigger => bus::PAD_R,
        _ => return None,
    })
}

fn pad_bit(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::ArrowUp | KeyCode::KeyW => bus::PAD_UP,
        KeyCode::ArrowDown | KeyCode::KeyS => bus::PAD_DOWN,
        KeyCode::ArrowLeft | KeyCode::KeyA => bus::PAD_LEFT,
        KeyCode::ArrowRight | KeyCode::KeyD => bus::PAD_RIGHT,
        KeyCode::KeyX => bus::PAD_A,
        KeyCode::KeyZ => bus::PAD_B,
        KeyCode::KeyC => bus::PAD_X,
        KeyCode::KeyV => bus::PAD_Y,
        KeyCode::Enter => bus::PAD_START,
        KeyCode::ShiftRight => bus::PAD_SELECT,
        KeyCode::KeyQ => bus::PAD_L,
        KeyCode::KeyE => bus::PAD_R,
        _ => return None,
    })
}

fn run_windowed(sys: System, scale: Option<usize>, watch: Option<Watch>) {
    let event_loop = EventLoop::new().expect("failed to create event loop");

    let mut vega68 = Vega68 {
        audio: AudioOut::new(),
        captured: false,
        frame: vec![0u32; WIDTH * HEIGHT],
        gamepad: 0,
        gilrs: Gilrs::new()
            .inspect_err(|e| eprintln!("gamepads unavailable: {e}"))
            .ok(),
        initial_scale: scale,
        look: (0.0, 0.0),
        next_frame: Instant::now(),
        pad: 0,
        rstick: (0.0, 0.0),
        stick: 0,
        surface: None,
        sys,
        watch,
        window: None,
    };

    event_loop.run_app(&mut vega68).expect("event loop failed");
}

impl Vega68 {
    fn draw(&mut self) {
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let mut buffer = surface
            .buffer_mut()
            .expect("failed to acquire surface buffer");
        let w = buffer.width().get() as usize;
        let h = buffer.height().get() as usize;

        self.sys.render(&mut self.frame);
        blit(&self.frame, &mut buffer, w, h);

        buffer.present().expect("failed to present frame");
    }

    fn capture_cursor(&mut self) {
        let Some(w) = &self.window else { return };

        if self.captured {
            return;
        }

        let grabbed = w
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined))
            .is_ok();

        if grabbed {
            w.set_cursor_visible(false);
            self.captured = true;
        }
    }

    fn release_cursor(&mut self) {
        let Some(w) = &self.window else { return };

        let _ = w.set_cursor_grab(CursorGrabMode::None);
        w.set_cursor_visible(true);
        self.captured = false;
    }

    fn poll_gamepad(&mut self) {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return;
        };

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::ButtonPressed(button, _) => {
                    if let Some(bit) = gamepad_bit(button) {
                        self.gamepad |= bit;
                    }
                }

                EventType::ButtonReleased(button, _) => {
                    if let Some(bit) = gamepad_bit(button) {
                        self.gamepad &= !bit;
                    }
                }

                EventType::AxisChanged(Axis::LeftStickX, v, _) => {
                    self.stick &= !(1 << 2 | 1 << 3);

                    if v < -0.5 {
                        self.stick |= 1 << 2;
                    } else if v > 0.5 {
                        self.stick |= 1 << 3;
                    }
                }

                EventType::AxisChanged(Axis::LeftStickY, v, _) => {
                    self.stick &= !(1 << 0 | 1 << 1);

                    if v > 0.5 {
                        self.stick |= 1 << 0;
                    } else if v < -0.5 {
                        self.stick |= 1 << 1;
                    }
                }

                EventType::AxisChanged(Axis::RightStickX, v, _) => {
                    self.rstick.0 = if v.abs() > 0.15 { v as f64 } else { 0.0 };
                }

                EventType::AxisChanged(Axis::RightStickY, v, _) => {
                    self.rstick.1 = if v.abs() > 0.15 { -v as f64 } else { 0.0 };
                }

                _ => {}
            }
        }
    }
}

impl ApplicationHandler for Vega68 {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();

        if now >= self.next_frame {
            if let Some(w) = &mut self.watch {
                w.poll(&mut self.sys);
            }
            self.poll_gamepad();
            self.sys.bus.pads[0] = self.pad | self.gamepad | self.stick;
            self.look.0 += self.rstick.0 * STICK_LOOK;
            self.look.1 += self.rstick.1 * STICK_LOOK;
            let (dx, dy) = (
                (self.look.0 * LOOK_SMOOTH) as i16,
                (self.look.1 * LOOK_SMOOTH) as i16,
            );
            self.look = (self.look.0 - dx as f64, self.look.1 - dy as f64);
            self.sys.bus.mouse = [dx, dy];
            self.sys.run_frame();
            if let Some(a) = &self.audio {
                a.push(&self.sys.bus.apu.frame);
            }
            self.next_frame = (self.next_frame + FRAME).max(now - FRAME);

            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next());
        let geometry = monitor.as_ref().map(|m| (m.size().width, m.size().height));
        let scale = self.initial_scale.unwrap_or_else(|| auto_scale(geometry));
        let size = PhysicalSize::new((WIDTH * scale) as u32, (HEIGHT * scale) as u32);

        let mut attributes = Window::default_attributes()
            .with_title("vega68")
            .with_inner_size(size);

        if let (Some(m), Some(g)) = (&monitor, geometry) {
            let p = m.position();
            let (x, y) = centre((p.x, p.y), g, (size.width, size.height));

            attributes = attributes.with_position(PhysicalPosition::new(x, y));
        }

        let window = Rc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create window"),
        );
        let context = Context::new(window.clone()).expect("failed to create softbuffer context");
        let mut surface =
            Surface::new(&context, window.clone()).expect("failed to create softbuffer surface");

        surface
            .resize(
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            )
            .expect("surface resize");

        self.surface = Some(surface);
        self.window = Some(window);
        self.next_frame = Instant::now();
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.captured {
                self.look.0 += delta.0;
                self.look.1 += delta.1;
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    if self.captured {
                        self.release_cursor();
                    } else {
                        event_loop.exit();
                    }
                } else if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(bit) = pad_bit(code) {
                        match event.state {
                            ElementState::Pressed => self.pad |= bit,
                            ElementState::Released => self.pad &= !bit,
                        }
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.capture_cursor(),

            WindowEvent::RedrawRequested => self.draw(),

            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(w), Some(h)) = (
                    self.surface.as_mut(),
                    NonZeroU32::new(size.width),
                    NonZeroU32::new(size.height),
                ) {
                    surface.resize(w, h).expect("surface resize");
                }
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_frame() -> Vec<u32> {
        (0..(WIDTH * HEIGHT) as u32).map(|i| i + 1).collect()
    }

    #[test]
    fn fit_matches_the_worked_table() {
        let cases: [(usize, usize, usize, usize, usize); 7] = [
            (1280, 720, 4, 0, 0),
            (1600, 900, 5, 0, 0),
            (1700, 950, 5, 50, 25),
            (3440, 1440, 8, 440, 0),
            (900, 500, 2, 130, 70),
            (320, 180, 1, 0, 0),
            (200, 100, 1, 0, 0),
        ];

        for (sw, sh, scale, ox, oy) in cases {
            assert_eq!(fit(sw, sh), (scale, ox, oy), "surface {sw}x{sh}");
        }
    }

    #[test]
    fn auto_scale_matches_the_worked_table() {
        let cases: [(u32, u32, usize); 9] = [
            (1366, 768, 4),
            (1920, 1080, 6),
            (2560, 1440, 6),
            (3440, 1440, 6),
            (3840, 2160, 6),
            (1280, 800, 4),
            (1024, 768, 3),
            (640, 480, 2),
            (320, 180, 1),
        ];

        for (w, h, scale) in cases {
            assert_eq!(auto_scale(Some((w, h))), scale, "monitor {w}x{h}");
        }

        assert_eq!(auto_scale(None), FALLBACK_SCALE, "no monitor");
    }

    #[test]
    fn auto_scale_never_exceeds_1920x1080() {
        for w in (320..=3840).step_by(16) {
            for h in (180..=2160).step_by(18) {
                let s = auto_scale(Some((w, h)));

                assert!(WIDTH * s <= 1920, "{w}x{h} chose {s}x, too wide");
                assert!(HEIGHT * s <= 1080, "{w}x{h} chose {s}x, too tall");
            }
        }
    }

    #[test]
    fn blit_exact_fit_has_no_border() {
        let frame = test_frame();
        let (sw, sh) = (1280, 720);
        let mut out = vec![0u32; sw * sh];

        blit(&frame, &mut out, sw, sh);

        assert!(out.iter().all(|&px| px != 0));
        assert_eq!(out[0], frame[0]);
        assert_eq!(out[sw * sh - 1], frame[WIDTH * HEIGHT - 1]);
    }

    #[test]
    fn blit_letterboxed_is_black_outside_the_rect() {
        let frame = test_frame();
        let (sw, sh) = (1700, 950);
        let sentinel = 0xdead_beefu32;
        let mut out = vec![sentinel; sw * sh];

        blit(&frame, &mut out, sw, sh);

        let (scale, ox, oy) = fit(sw, sh);
        let mid_y = sh / 2;
        let mid_x = sw / 2;
        let right = ox + WIDTH * scale;
        let bottom = oy + HEIGHT * scale;

        assert_eq!(out[mid_y * sw + (ox - 1)], 0, "just outside left edge");
        assert_eq!(out[mid_y * sw + right], 0, "just outside right edge");
        assert_eq!(out[(oy - 1) * sw + mid_x], 0, "just outside top edge");
        assert_eq!(out[bottom * sw + mid_x], 0, "just outside bottom edge");

        for y in 0..HEIGHT * scale {
            for x in 0..WIDTH * scale {
                let expected = frame[(y / scale) * WIDTH + x / scale];
                assert_eq!(
                    out[(oy + y) * sw + (ox + x)],
                    expected,
                    "interior pixel ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn blit_clips_an_undersized_surface() {
        let frame = test_frame();
        let (sw, sh) = (200, 100);
        let mut out = vec![0u32; sw * sh];

        blit(&frame, &mut out, sw, sh);

        assert_eq!(out[0], frame[0]);
        assert_eq!(out[sh * sw - 1], frame[(sh - 1) * WIDTH + (sw - 1)]);
    }

    #[test]
    fn blit_zero_sized_surface_does_not_panic() {
        let frame = test_frame();
        let mut out: Vec<u32> = vec![];

        blit(&frame, &mut out, 0, 0);
        blit(&frame, &mut out, 0, 5);
        blit(&frame, &mut out, 5, 0);
    }
}

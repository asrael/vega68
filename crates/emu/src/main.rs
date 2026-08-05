//! vega68 <cart.v68> [--headless N] [--scale K]
//!
//! the bios image is burned into the binary (`bios/vega68.rom`, written by
//! `cargo xtask bios`), not built on demand.
//!
//! windowed: runs the cart at 60 Hz in a winit window, nearest-neighbor
//! scaled (default 4x = 1280x720).
//!
//! headless (CI): runs N frames, printing an fnv1a64 hash of each frame.

use vega68::System;
use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType, Gilrs};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

const BIOS: &[u8] = include_bytes!("../../../bios/vega68.rom");
const FRAME: Duration = Duration::from_nanos(16_666_667);

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

fn fnv1a64(data: &[u32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;

    for px in data {
        for b in px.to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    h
}

fn run_headless(mut sys: System, frames: u64) {
    let mut frame = vec![0u32; WIDTH * HEIGHT];

    for i in 0..frames {
        sys.run_frame();
        sys.render(&mut frame);
        println!("frame {i} {:016x}", fnv1a64(&frame));
    }
}

fn usage() -> ! {
    eprintln!("usage: vega68 <cart.v68> [--headless N] [--scale K]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cart_path = None;
    let mut headless = None;
    let mut scale = 4usize;

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
                scale = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            _ if cart_path.is_none() => cart_path = Some(arg),
            _ => usage(),
        }
    }

    let cart_path = cart_path.unwrap_or_else(|| usage());
    let file = std::fs::read(&cart_path)
        .unwrap_or_else(|e| die(&format!("failed to read {cart_path}: {e}")));
    let sys = System::new(BIOS, &file)
        .unwrap_or_else(|e| die(&format!("{cart_path} is not a valid cart: {e}")));

    match headless {
        Some(frames) => run_headless(sys, frames),
        None => run_windowed(sys, scale.max(1)),
    }
}

struct Vega68 {
    frame: Vec<u32>,
    gamepad: u16,
    gilrs: Option<Gilrs>,
    next_frame: Instant,
    pad: u16,
    scale: usize,
    stick: u16,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    sys: System,
    window: Option<Rc<Window>>,
}

fn gamepad_bit(button: Button) -> Option<u16> {
    Some(match button {
        Button::DPadUp => bus::PAD_UP,
        Button::DPadDown => bus::PAD_DOWN,
        Button::DPadLeft => bus::PAD_LEFT,
        Button::DPadRight => bus::PAD_RIGHT,
        Button::East => bus::PAD_A,  // right face
        Button::South => bus::PAD_B, // bottom face
        Button::North => bus::PAD_X, // top face
        Button::West => bus::PAD_Y,  // left face
        Button::Start => bus::PAD_START,
        Button::Select => bus::PAD_SELECT,
        Button::LeftTrigger => bus::PAD_L,
        Button::RightTrigger => bus::PAD_R,
        _ => return None,
    })
}

fn pad_bit(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::ArrowUp => bus::PAD_UP,
        KeyCode::ArrowDown => bus::PAD_DOWN,
        KeyCode::ArrowLeft => bus::PAD_LEFT,
        KeyCode::ArrowRight => bus::PAD_RIGHT,
        KeyCode::KeyX => bus::PAD_A,
        KeyCode::KeyZ => bus::PAD_B,
        KeyCode::KeyS => bus::PAD_X,
        KeyCode::KeyA => bus::PAD_Y,
        KeyCode::Enter => bus::PAD_START,
        KeyCode::ShiftRight => bus::PAD_SELECT,
        KeyCode::KeyQ => bus::PAD_L,
        KeyCode::KeyW => bus::PAD_R,
        _ => return None,
    })
}

fn run_windowed(sys: System, scale: usize) {
    let event_loop = EventLoop::new().expect("failed to create event loop");

    let mut vega68 = Vega68 {
        frame: vec![0u32; WIDTH * HEIGHT],
        gamepad: 0,
        gilrs: Gilrs::new()
            .inspect_err(|e| eprintln!("gamepads unavailable: {e}"))
            .ok(),
        next_frame: Instant::now(),
        pad: 0,
        scale,
        stick: 0,
        surface: None,
        sys,
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
        let w = WIDTH * self.scale;

        self.sys.render(&mut self.frame);

        for y in 0..HEIGHT * self.scale {
            let src = &self.frame[(y / self.scale) * WIDTH..][..WIDTH];
            let dst = &mut buffer[y * w..][..w];
            for (x, px) in dst.iter_mut().enumerate() {
                *px = src[x / self.scale];
            }
        }

        buffer.present().expect("failed to present frame");
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

                // left stick aliases the D-pad
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

                _ => {}
            }
        }
    }
}

impl ApplicationHandler for Vega68 {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();

        if now >= self.next_frame {
            self.poll_gamepad();
            self.sys.bus.pads[0] = self.pad | self.gamepad | self.stick;
            self.sys.run_frame();
            self.next_frame = (self.next_frame + FRAME).max(now - FRAME);

            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let size = PhysicalSize::new((WIDTH * self.scale) as u32, (HEIGHT * self.scale) as u32);
        let window = Rc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("vega68")
                        .with_inner_size(size)
                        .with_resizable(false),
                )
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                } else if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(bit) = pad_bit(code) {
                        match event.state {
                            ElementState::Pressed => self.pad |= bit,
                            ElementState::Released => self.pad &= !bit,
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => self.draw(),

            _ => {}
        }
    }
}

use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType, Gilrs};

use vega68::System;
use vega68::apu::out::AudioOut;
use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

use crate::sdl::{Input, Platform};
use crate::watch::Watch;

const CURSOR_SPEED: f64 = 4.0;
const FRAME: Duration = Duration::from_nanos(16_666_667);

pub struct App {
    audio: Option<AudioOut>,
    frame: Vec<u32>,
    input: Input,
    next_frame: Instant,
    pads: Pads,
    platform: Platform,
    system: System,
    watch: Option<Watch>,
}

struct Pads {
    gamepad: u16,
    gilrs: Option<Gilrs>,
    lstick: u16,
    rstick: (f64, f64),
}

impl App {
    pub fn new(system: System, scale: Option<usize>, watch: Option<Watch>) -> App {
        App {
            audio: AudioOut::new(),
            frame: vec![0u32; WIDTH * HEIGHT],
            input: Input::default(),
            next_frame: Instant::now(),
            pads: Pads {
                gamepad: 0,
                gilrs: Gilrs::new()
                    .inspect_err(|e| eprintln!("gamepads unavailable: {e}"))
                    .ok(),
                lstick: 0,
                rstick: (0.0, 0.0),
            },
            platform: Platform::new(scale),
            system,
            watch,
        }
    }

    pub fn run(mut self) {
        loop {
            if !self.platform.poll(&mut self.input) {
                return;
            }

            let now = Instant::now();
            let due = match &self.audio {
                Some(a) => a.queued() <= a.target() || now >= self.next_frame + 4 * FRAME,
                None => now >= self.next_frame,
            };

            if due {
                if let Some(w) = &mut self.watch {
                    w.poll(&mut self.system);
                }

                poll_gamepad(&mut self.pads);

                let pads = self.input.keys | self.pads.gamepad | self.pads.lstick;
                let mut buttons = self.input.buttons;

                if pads & bus::PAD_A != 0 {
                    buttons |= bus::MOUSE_L;
                }
                if pads & bus::PAD_B != 0 {
                    buttons |= bus::MOUSE_R;
                }

                self.input.set_cursor(
                    self.input.cursor.0 + self.pads.rstick.0 * CURSOR_SPEED,
                    self.input.cursor.1 + self.pads.rstick.1 * CURSOR_SPEED,
                );

                self.system.bus.pads[0] = pads;
                self.system.bus.mouse = [self.input.cursor.0 as u16, self.input.cursor.1 as u16];
                self.system.bus.mouse_btn = buttons;
                self.system.run_frame();

                if let Some(a) = &self.audio {
                    a.push(&self.system.bus.apu.frame);
                }

                self.system.render(&mut self.frame);
                self.platform.present(&self.frame);
                self.next_frame = (self.next_frame + FRAME).max(now - FRAME);
            }

            let wake = match &self.audio {
                Some(_) => now + FRAME / 8,
                None => self.next_frame,
            };

            self.platform
                .wait(wake.saturating_duration_since(Instant::now()).as_millis() as i32);
        }
    }

    pub fn run_headless(mut system: System, frames: u64, mut watch: Option<Watch>) {
        let mut frame = vec![0u32; WIDTH * HEIGHT];

        for i in 0..frames {
            if let Some(w) = &mut watch {
                w.poll(&mut system);
            }

            system.run_frame();
            system.render(&mut frame);
            println!(
                "frame {i} {:016x}",
                vega68::fnv1a64(frame.iter().flat_map(|p| p.to_le_bytes()))
            );
        }
    }
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

fn poll_gamepad(pads: &mut Pads) {
    let Some(gilrs) = pads.gilrs.as_mut() else {
        return;
    };

    while let Some(event) = gilrs.next_event() {
        match event.event {
            EventType::ButtonPressed(button, _) => {
                if let Some(bit) = gamepad_bit(button) {
                    pads.gamepad |= bit;
                }
            }

            EventType::ButtonReleased(button, _) => {
                if let Some(bit) = gamepad_bit(button) {
                    pads.gamepad &= !bit;
                }
            }

            EventType::AxisChanged(Axis::LeftStickX, v, _) => {
                pads.lstick &= !(1 << 2 | 1 << 3);

                if v < -0.5 {
                    pads.lstick |= 1 << 2;
                } else if v > 0.5 {
                    pads.lstick |= 1 << 3;
                }
            }

            EventType::AxisChanged(Axis::LeftStickY, v, _) => {
                pads.lstick &= !(1 << 0 | 1 << 1);

                if v > 0.5 {
                    pads.lstick |= 1 << 0;
                } else if v < -0.5 {
                    pads.lstick |= 1 << 1;
                }
            }

            EventType::AxisChanged(Axis::RightStickX, v, _) => {
                pads.rstick.0 = if v.abs() > 0.15 { v as f64 } else { 0.0 };
            }

            EventType::AxisChanged(Axis::RightStickY, v, _) => {
                pads.rstick.1 = if v.abs() > 0.15 { -v as f64 } else { 0.0 };
            }

            _ => {}
        }
    }
}

use std::time::{Duration, Instant};

use vega68::System;
use vega68::apu::out::AudioOut;
use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

use crate::sdl::{Input, Platform};
use crate::watch::Watch;

const FRAME: Duration = Duration::from_nanos(16_666_667);

pub struct App {
    audio: Option<AudioOut>,
    frame: Vec<u32>,
    input: Input,
    next_frame: Instant,
    platform: Platform,
    system: System,
    watch: Option<Watch>,
}

impl App {
    pub fn new(system: System, scale: Option<usize>, watch: Option<Watch>) -> App {
        App {
            audio: AudioOut::new(),
            frame: vec![0u32; WIDTH * HEIGHT],
            input: Input::default(),
            next_frame: Instant::now(),
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

                let pads = self.input.keys | self.input.gamepad | self.input.lstick;
                let mut buttons = self.input.buttons;

                if pads & bus::PAD_A != 0 {
                    buttons |= bus::MOUSE_L;
                }
                if pads & bus::PAD_B != 0 {
                    buttons |= bus::MOUSE_R;
                }

                self.input.drive_cursor();

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

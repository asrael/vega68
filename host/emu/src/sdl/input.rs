use sdl3_sys::everything::*;

use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

#[derive(Default)]
pub struct Input {
    pub buttons: u16,
    pub cursor: (f64, f64),
    pub keys: u16,
}

impl Input {
    pub fn key_bit(code: SDL_Scancode) -> Option<u16> {
        Some(match code {
            SDL_SCANCODE_UP | SDL_SCANCODE_W => bus::PAD_UP,
            SDL_SCANCODE_DOWN | SDL_SCANCODE_S => bus::PAD_DOWN,
            SDL_SCANCODE_LEFT | SDL_SCANCODE_A => bus::PAD_LEFT,
            SDL_SCANCODE_RIGHT | SDL_SCANCODE_D => bus::PAD_RIGHT,
            SDL_SCANCODE_X => bus::PAD_A,
            SDL_SCANCODE_Z => bus::PAD_B,
            SDL_SCANCODE_C => bus::PAD_X,
            SDL_SCANCODE_V => bus::PAD_Y,
            SDL_SCANCODE_RETURN => bus::PAD_START,
            SDL_SCANCODE_RSHIFT => bus::PAD_SELECT,
            SDL_SCANCODE_Q => bus::PAD_L,
            SDL_SCANCODE_E => bus::PAD_R,
            _ => return None,
        })
    }

    pub fn set_cursor(&mut self, x: f64, y: f64) {
        self.cursor = (
            x.clamp(0.0, (WIDTH - 1) as f64),
            y.clamp(0.0, (HEIGHT - 1) as f64),
        );
    }
}

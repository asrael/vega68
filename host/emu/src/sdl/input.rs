use sdl3_sys::everything::*;

use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

#[derive(Default)]
pub struct Input {
    pub buttons: u16,
    pub cursor: (f64, f64),
    pub gamepad: u16,
    pub keys: u16,
    pub lstick: u16,
    pub rstick: (f64, f64),
}

impl Input {
    const CURSOR_SPEED: f64 = 4.0;
    const DEADZONE: i16 = 4_915;
    const STICK_ON: i16 = 16_384;

    pub fn apply_axis(&mut self, axis: SDL_GamepadAxis, raw: i16) {
        let analog = |v: i16| {
            if v.abs() > Self::DEADZONE {
                v as f64 / 32_767.0
            } else {
                0.0
            }
        };

        match axis {
            SDL_GAMEPAD_AXIS_LEFTY => {
                self.lstick &= !(bus::PAD_UP | bus::PAD_DOWN);

                if raw < -Self::STICK_ON {
                    self.lstick |= bus::PAD_UP;
                } else if raw > Self::STICK_ON {
                    self.lstick |= bus::PAD_DOWN;
                }
            }

            SDL_GAMEPAD_AXIS_LEFTX => {
                self.lstick &= !(bus::PAD_LEFT | bus::PAD_RIGHT);

                if raw < -Self::STICK_ON {
                    self.lstick |= bus::PAD_LEFT;
                } else if raw > Self::STICK_ON {
                    self.lstick |= bus::PAD_RIGHT;
                }
            }

            SDL_GAMEPAD_AXIS_RIGHTX => self.rstick.0 = analog(raw),
            SDL_GAMEPAD_AXIS_RIGHTY => self.rstick.1 = analog(raw),
            _ => {}
        }
    }

    pub fn drive_cursor(&mut self) {
        self.set_cursor(
            self.cursor.0 + self.rstick.0 * Self::CURSOR_SPEED,
            self.cursor.1 + self.rstick.1 * Self::CURSOR_SPEED,
        );
    }

    pub fn gamepad_bit(button: SDL_GamepadButton) -> Option<u16> {
        Some(match button {
            SDL_GAMEPAD_BUTTON_DPAD_UP => bus::PAD_UP,
            SDL_GAMEPAD_BUTTON_DPAD_DOWN => bus::PAD_DOWN,
            SDL_GAMEPAD_BUTTON_DPAD_LEFT => bus::PAD_LEFT,
            SDL_GAMEPAD_BUTTON_DPAD_RIGHT => bus::PAD_RIGHT,
            SDL_GAMEPAD_BUTTON_EAST => bus::PAD_A,
            SDL_GAMEPAD_BUTTON_SOUTH => bus::PAD_B,
            SDL_GAMEPAD_BUTTON_NORTH => bus::PAD_X,
            SDL_GAMEPAD_BUTTON_WEST => bus::PAD_Y,
            SDL_GAMEPAD_BUTTON_START => bus::PAD_START,
            SDL_GAMEPAD_BUTTON_BACK => bus::PAD_SELECT,
            SDL_GAMEPAD_BUTTON_LEFT_SHOULDER => bus::PAD_L,
            SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER => bus::PAD_R,
            _ => return None,
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_digitize_left_and_deadzone_right() {
        // (axis, raw, lstick_before) -> (lstick_after, rstick_x, rstick_y)
        let cases: [(SDL_GamepadAxis, i16, u16, u16, f64, f64); 8] = [
            (SDL_GAMEPAD_AXIS_LEFTY, -20_000, 0, 0b0001, 0.0, 0.0), // sdl y is down-positive: negative = up
            (SDL_GAMEPAD_AXIS_LEFTY, 20_000, 0b0001, 0b0010, 0.0, 0.0), // down replaces up
            (SDL_GAMEPAD_AXIS_LEFTY, 1_000, 0b1111, 0b1100, 0.0, 0.0), // centre clears only the y bits
            (SDL_GAMEPAD_AXIS_LEFTX, -20_000, 0, 0b0100, 0.0, 0.0),
            (SDL_GAMEPAD_AXIS_LEFTX, 20_000, 0, 0b1000, 0.0, 0.0),
            (SDL_GAMEPAD_AXIS_RIGHTX, 16_384, 0, 0, 0.5000, 0.0),
            (SDL_GAMEPAD_AXIS_RIGHTY, 16_384, 0, 0, 0.0, 0.5000), // down-positive: no negation
            (SDL_GAMEPAD_AXIS_RIGHTX, 4_000, 0, 0, 0.0, 0.0),     // inside the deadzone
        ];

        for (axis, raw, before, lstick, rx, ry) in cases {
            let mut input = Input {
                lstick: before,
                ..Input::default()
            };

            input.apply_axis(axis, raw);

            assert_eq!(input.lstick, lstick, "axis {} raw {raw}", axis.0);
            assert!((input.rstick.0 - rx).abs() < 1e-3, "axis {} raw {raw} x", axis.0);
            assert!((input.rstick.1 - ry).abs() < 1e-3, "axis {} raw {raw} y", axis.0);
        }
    }

    #[test]
    fn drive_cursor_integrates_rstick_and_clamps() {
        let mut input = Input {
            cursor: (317.0, 2.0),
            rstick: (1.0, -1.0),
            ..Input::default()
        };

        input.drive_cursor();
        assert_eq!(input.cursor, (319.0, 0.0), "clamped at the corner");

        input.rstick = (-0.5, 0.0);
        input.drive_cursor();
        assert_eq!(input.cursor, (317.0, 0.0), "half tilt moves half speed");
    }
}

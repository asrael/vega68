mod input;

use std::ffi::CStr;
use std::mem;
use std::ptr;

use sdl3_sys::everything::*;

use vega68::bus;
use vega68::vdp::{HEIGHT, WIDTH};

pub use input::Input;

pub struct Platform {
    renderer: *mut SDL_Renderer,
    texture: *mut SDL_Texture,
}

impl Platform {
    const FALLBACK_SCALE: usize = 4;

    pub fn new(scale: Option<usize>) -> Platform {
        unsafe {
            if !SDL_Init(SDL_INIT_VIDEO) {
                Self::fatal("SDL_Init");
            }

            let mode = SDL_GetDesktopDisplayMode(SDL_GetPrimaryDisplay());
            let monitor = (!mode.is_null()).then(|| ((*mode).w as u32, (*mode).h as u32));
            let scale = scale.unwrap_or_else(|| Self::auto_scale(monitor));

            let window = SDL_CreateWindow(
                c"vega68".as_ptr(),
                (WIDTH * scale) as i32,
                (HEIGHT * scale) as i32,
                SDL_WINDOW_RESIZABLE,
            );

            if window.is_null() {
                Self::fatal("SDL_CreateWindow");
            }

            SDL_SetWindowPosition(window, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);

            let renderer = SDL_CreateRenderer(window, ptr::null());

            if renderer.is_null() {
                Self::fatal("SDL_CreateRenderer");
            }

            if !SDL_SetRenderLogicalPresentation(
                renderer,
                WIDTH as i32,
                HEIGHT as i32,
                SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
            ) {
                Self::fatal("SDL_SetRenderLogicalPresentation");
            }

            let texture = SDL_CreateTexture(
                renderer,
                SDL_PIXELFORMAT_XRGB8888,
                SDL_TEXTUREACCESS_STREAMING,
                WIDTH as i32,
                HEIGHT as i32,
            );

            if texture.is_null() {
                Self::fatal("SDL_CreateTexture");
            }

            SDL_SetTextureScaleMode(texture, SDL_SCALEMODE_NEAREST);
            SDL_HideCursor();

            Platform { renderer, texture }
        }
    }

    pub fn poll(&self, input: &mut Input) -> bool {
        unsafe {
            let mut ev: SDL_Event = mem::zeroed();

            while SDL_PollEvent(&mut ev) {
                match SDL_EventType(ev.r#type) {
                    SDL_EVENT_QUIT => return false,

                    SDL_EVENT_KEY_DOWN | SDL_EVENT_KEY_UP => {
                        if ev.key.scancode == SDL_SCANCODE_ESCAPE {
                            return false;
                        }

                        if let Some(bit) = Input::key_bit(ev.key.scancode) {
                            if ev.key.down {
                                input.keys |= bit;
                            } else {
                                input.keys &= !bit;
                            }
                        }
                    }

                    SDL_EVENT_MOUSE_MOTION => {
                        SDL_ConvertEventToRenderCoordinates(self.renderer, &mut ev);
                        input.set_cursor(ev.motion.x as f64, ev.motion.y as f64);
                    }

                    SDL_EVENT_MOUSE_BUTTON_DOWN | SDL_EVENT_MOUSE_BUTTON_UP => {
                        let bit = match ev.button.button as i32 {
                            SDL_BUTTON_LEFT => bus::MOUSE_L,
                            SDL_BUTTON_RIGHT => bus::MOUSE_R,
                            _ => 0,
                        };

                        if ev.button.down {
                            input.buttons |= bit;
                        } else {
                            input.buttons &= !bit;
                        }
                    }

                    _ => {}
                }
            }
        }

        true
    }

    pub fn present(&self, frame: &[u32]) {
        unsafe {
            SDL_UpdateTexture(
                self.texture,
                ptr::null(),
                frame.as_ptr().cast(),
                (WIDTH * 4) as i32,
            );
            SDL_RenderClear(self.renderer);
            SDL_RenderTexture(self.renderer, self.texture, ptr::null(), ptr::null());
            SDL_RenderPresent(self.renderer);
        }
    }

    pub fn wait(&self, timeout_ms: i32) {
        unsafe {
            SDL_WaitEventTimeout(ptr::null_mut(), timeout_ms);
        }
    }

    fn auto_scale(monitor: Option<(u32, u32)>) -> usize {
        let Some((w, h)) = monitor else {
            return Self::FALLBACK_SCALE;
        };

        (w as usize / WIDTH)
            .min(h as usize / HEIGHT)
            .max(1)
    }

    fn fatal(what: &str) -> ! {
        let err = unsafe { CStr::from_ptr(SDL_GetError()) };

        crate::error(&format!("{what}: {}", err.to_string_lossy()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_scale_picks_the_largest_integer_fit() {
        let cases: [(u32, u32, usize); 9] = [
            (1366, 768, 4),
            (1920, 1080, 6),
            (2560, 1440, 8),
            (3440, 1440, 8),
            (3840, 2160, 12),
            (1280, 800, 4),
            (1024, 768, 3),
            (640, 480, 2),
            (320, 180, 1),
        ];

        for (w, h, scale) in cases {
            assert_eq!(Platform::auto_scale(Some((w, h))), scale, "monitor {w}x{h}");
        }

        assert_eq!(Platform::auto_scale(None), Platform::FALLBACK_SCALE, "no monitor");
    }
}

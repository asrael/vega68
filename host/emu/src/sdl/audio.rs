use std::ffi::CStr;
use std::mem;
use std::ptr;

use sdl3_sys::everything::*;

pub struct AudioOut {
    stream: *mut SDL_AudioStream,
}

impl AudioOut {
    const QUEUE_CAP: usize = 12_800;
    const QUEUE_TARGET: usize = 1_600;

    pub fn new() -> Option<AudioOut> {
        unsafe {
            if !SDL_Init(SDL_INIT_AUDIO) {
                eprintln!(
                    "audio: init failed ({}), running silent",
                    CStr::from_ptr(SDL_GetError()).to_string_lossy()
                );
                return None;
            }

            let spec = SDL_AudioSpec {
                format: SDL_AUDIO_S16,
                channels: 2,
                freq: 48_000,
            };
            let stream = SDL_OpenAudioDeviceStream(
                SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,
                &spec,
                None,
                ptr::null_mut(),
            );

            if stream.is_null() {
                eprintln!(
                    "audio: no output stream ({}), running silent",
                    CStr::from_ptr(SDL_GetError()).to_string_lossy()
                );
                return None;
            }

            SDL_ResumeAudioStreamDevice(stream);

            Some(AudioOut { stream })
        }
    }

    pub fn push(&self, samples: &[i16]) {
        unsafe {
            if self.queued() > Self::QUEUE_CAP {
                SDL_ClearAudioStream(self.stream);
            }

            SDL_PutAudioStreamData(
                self.stream,
                samples.as_ptr().cast(),
                mem::size_of_val(samples) as i32,
            );
        }
    }

    pub fn queued(&self) -> usize {
        unsafe { SDL_GetAudioStreamQueued(self.stream) as usize / 4 }
    }

    pub fn target(&self) -> usize {
        Self::QUEUE_TARGET
    }
}

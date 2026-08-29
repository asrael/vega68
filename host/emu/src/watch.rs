use vega68::System;

use std::time::SystemTime;

pub struct Watch {
    bytes: Vec<u8>,
    mtime: Option<SystemTime>,
    path: String,
}

impl Watch {
    pub fn new(path: String, bytes: Vec<u8>) -> Self {
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        Watch { bytes, mtime, path }
    }

    pub fn poll(&mut self, sys: &mut System) {
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

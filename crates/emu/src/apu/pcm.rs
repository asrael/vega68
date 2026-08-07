use super::{Apu, SAMPLE_RATE};

#[derive(Clone, Copy, Default)]
pub(super) struct Pcm {
    pub(super) playing: bool,
    pos: f64,
}

impl Apu {
    pub(super) fn pcm(&mut self, ch: usize, mem: &[u8]) -> f64 {
        let base = ch * 0x40;
        let i = ch - 12;

        if !self.pcm[i].playing {
            return 0.0;
        }

        let start = u32::from_be_bytes([
            self.regs[base],
            self.regs[base + 1],
            self.regs[base + 2],
            self.regs[base + 3],
        ]);
        let len = u32::from_be_bytes([
            self.regs[base + 4],
            self.regs[base + 5],
            self.regs[base + 6],
            self.regs[base + 7],
        ]);
        let loop_off = u32::from_be_bytes([
            self.regs[base + 8],
            self.regs[base + 9],
            self.regs[base + 10],
            self.regs[base + 11],
        ]);
        let rate = u16::from_be_bytes([self.regs[base + 0x0C], self.regs[base + 0x0D]]);
        let vol = self.regs[base + 0x0E];
        let ctrl = self.regs[base + 0x10];

        let pos = self.pcm[i].pos;
        let addr = start.wrapping_add(pos as u32);
        let byte = mem.get(addr as usize).copied().unwrap_or(0);
        let sample = (byte as i8) as f64 / 128.0 * (vol as f64 / 255.0);

        let mut new_pos = pos + rate.min(48_000) as f64 / SAMPLE_RATE;
        if new_pos >= len as f64 {
            if ctrl & 0x01 != 0 {
                new_pos = loop_off as f64 + (new_pos - len as f64);
                if new_pos >= len as f64 {
                    new_pos = loop_off as f64; // pathological loop_off >= len: don't run away
                }
            } else {
                self.pcm[i].playing = false;
            }
        }
        self.pcm[i].pos = new_pos;

        sample
    }

    // Nonzero op mask starts from START, zero stops.
    pub(super) fn pcm_key(&mut self, ch: usize, mask: u8) {
            let pcm = &mut self.pcm[ch - 12];

            pcm.playing = mask & 0xF0 != 0;

            if pcm.playing {
                pcm.pos = 0.0;
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apu::testkit::*;

    #[test]
    fn pcm_plays_signed_bytes_at_the_programmed_rate_and_stops() {
        let mut mem = vec![0u8; 0x100];
        mem[0x10..0x20].fill(0x7F); // 16 full-scale samples

        let mut a = Apu::new();
        pcm_setup(&mut a, 0x10, 16, 48000);
        a.write(KEYON_ADDR, 0x1C); // any nonzero op mask, ch 12

        a.run_line(&mem, 0);
        assert_eq!(
            a.frame[0], 10837,
            "first sample not at the expected full-scale level"
        );

        for line in 1..200 {
            a.run_line(&mem, line);
        }
        assert_eq!(
            status(&a) & (1 << 12),
            0,
            "16 samples at 48 kHz must stop within a frame"
        );
        assert_eq!(*a.frame.last().unwrap(), 0);
    }

    #[test]
    fn pcm_loops_when_enabled_and_keyoff_stops_it() {
        let mut mem = vec![0u8; 0x40];
        mem[0..8].fill(0x7F);

        let mut a = Apu::new();
        pcm_setup(&mut a, 0, 8, 8000);
        a.write(12 * 0x40 + 0x10, 0x01); // loop enable, LOOP = 0
        a.write(KEYON_ADDR, 0x1C);

        for _ in 0..10 {
            run_frame(&mut a, &mem);
        }

        assert_eq!(status(&a) & (1 << 12), 1 << 12, "looping channel stopped");

        a.write(KEYON_ADDR, 0x0C); // zero mask: stop
        run_frame(&mut a, &mem);
        assert_eq!(status(&a) & (1 << 12), 0);
    }

    #[test]
    fn pcm_loop_wrap_keeps_the_fractional_overshoot() {
        let mut mem = vec![0u8; 0x40];
        mem[0..8].fill(0x7F);

        let mut a = Apu::new();
        pcm_setup(&mut a, 0, 8, 40_000); // step = 40000/48000 = 5/6: does not divide 8 evenly
        a.write(12 * 0x40 + 0x08 + 3, 2); // LOOP = 2
        a.write(12 * 0x40 + 0x10, 0x01); // loop enable
        a.write(KEYON_ADDR, 0x1C);

        // pos crosses LEN=8 on the 10th sample (9*5/6=7.5,
        // +5/6=8.3333 >= 8), wrapping to LOOP(2) + (8.3333-8) = 2.3333...
        for _ in 0..10 {
            a.pcm(12, &mem);
        }

        assert!(
            (a.pcm[0].pos - (2.0 + 1.0 / 3.0)).abs() < 1e-9,
            "loop wrap truncated the fractional overshoot instead of carrying it past LOOP"
        );
    }

    #[test]
    fn pcm_loop_wrap_clamps_a_pathological_loop_offset_past_len() {
        let mut mem = vec![0u8; 0x40];
        mem[0..8].fill(0x7F);

        let mut a = Apu::new();
        pcm_setup(&mut a, 0, 8, 48_000); // step = 1.0/sample
        a.write(12 * 0x40 + 0x08 + 3, 20); // LOOP = 20, past LEN = 8: pathological
        a.write(12 * 0x40 + 0x10, 0x01); // loop enable
        a.write(KEYON_ADDR, 0x1C);

        for _ in 0..50 {
            a.pcm(12, &mem);
        }

        assert_eq!(
            a.pcm[0].pos, 20.0,
            "a loop offset at or past LEN must clamp to LOOP, not run away unbounded"
        );
    }

    #[test]
    fn pcm_fetch_outside_memory_reads_silence() {
        let mut a = Apu::new();
        pcm_setup(&mut a, 0xFFFF_0000, 64, 48000);
        a.write(KEYON_ADDR, 0x1C);

        run_frame(&mut a, &[]);
        assert!(a.frame.iter().all(|&s| s == 0));
    }
}

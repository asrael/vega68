use super::{Apu, SAMPLE_RATE};

pub(super) const PSG_CLOCK: f64 = 3_579_545.0;

fn atten_gain(reg: u8) -> Option<f64> {
    let atten = reg & 0x0F;

    if atten == 15 {
        None
    } else {
        Some(10f64.powf(-(atten as f64) / 10.0))
    }
}

impl Apu {
    pub(super) fn noise(&mut self) -> f64 {
        let base = 11 * 0x40;
        let ctrl = self.regs[base];
        let Some(gain) = atten_gain(self.regs[base + 2]) else {
            return 0.0;
        };

        let rate = ctrl & 0x03;
        let divisor = match rate {
            0 => 512.0,
            1 => 1024.0,
            2 => 2048.0,
            3 => {
                let period = (u16::from_be_bytes([self.regs[10 * 0x40], self.regs[10 * 0x40 + 1]])
                    & 0x3FF) as f64;
                if period == 0.0 {
                    return if self.lfsr & 1 != 0 { gain } else { -gain };
                }
                32.0 * period
            }
            _ => unreachable!(),
        };

        let shift_freq = PSG_CLOCK / divisor;
        let increment = shift_freq / SAMPLE_RATE;

        let old_phase = self.noise_phase;
        self.noise_phase = (self.noise_phase + increment).fract();

        if self.noise_phase < old_phase {
            let bit = if ctrl & 0x04 != 0 {
                (self.lfsr & 1) ^ ((self.lfsr >> 3) & 1)
            } else {
                self.lfsr & 1
            };

            self.lfsr = (self.lfsr >> 1) | (bit << 15);
        }

        if self.lfsr & 1 != 0 { gain } else { -gain }
    }

    pub(super) fn square(&mut self, ch: usize) -> f64 {
        let base = ch * 0x40;
        let period = (u16::from_be_bytes([self.regs[base], self.regs[base + 1]]) & 0x3FF) as f64;
        let Some(gain) = atten_gain(self.regs[base + 2]) else {
            return 0.0;
        };

        if period == 0.0 {
            return gain;
        }

        let i = ch - 8;
        self.phase[i] = (self.phase[i] + PSG_CLOCK / (32.0 * period) / SAMPLE_RATE).fract();

        if self.phase[i] < 0.5 { gain } else { -gain }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apu::testkit::*;

    #[test]
    fn square_frequency_matches_the_sn_formula() {
        let mut a = Apu::new();

        a.write(8 * 0x40, 0x00);
        a.write(8 * 0x40 + 1, 0xFE);
        a.write(8 * 0x40 + 2, 0);

        assert_eq!(cycles_per_second(&mut a), 440, "rising edges in 1 s");
    }

    #[test]
    fn atten_15_is_silent_and_period_0_is_dc() {
        let mut a = Apu::new();

        a.write(8 * 0x40 + 1, 100);
        run_frame(&mut a, &[]);
        assert!(
            a.frame.iter().all(|&s| s == 0),
            "atten 15 must gate the channel"
        );

        a.write(8 * 0x40 + 2, 0);
        run_frame(&mut a, &[]);
        assert!(a.frame.iter().any(|&s| s != 0));

        a.write(8 * 0x40 + 1, 0);
        run_frame(&mut a, &[]);
        assert!(
            a.frame.iter().all(|&s| s > 0),
            "period 0 must hold the output high"
        );
    }

    fn shift_outputs(frame: &[i16], n: usize) -> Vec<u16> {
        let stride = SAMPLE_RATE / (PSG_CLOCK / 512.0);

        let mut result = Vec::new();
        for i in 0..n {
            let sample_idx = ((i + 1) as f64 * stride).round() as usize;
            let frame_idx = sample_idx * 2;

            if frame_idx < frame.len() {
                let sample = frame[frame_idx];
                result.push(if sample > 0 { 1 } else { 0 });
            }
        }

        result
    }

    #[test]
    fn lfsr_sequence_matches_the_sms_taps() {
        let mut a = Apu::new();

        a.write(11 * 0x40, 0b100);
        a.write(11 * 0x40 + 2, 0);

        let want = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0];

        run_frame(&mut a, &[]);

        let shifts = shift_outputs(&a.frame, 16);
        assert_eq!(shifts, want, "LFSR taps or reset value are wrong");
    }

    #[test]
    fn periodic_noise_repeats_every_16_shifts() {
        let mut a = Apu::new();

        a.write(11 * 0x40, 0b000);
        a.write(11 * 0x40 + 2, 0);
        run_frame(&mut a, &[]);

        let s = shift_outputs(&a.frame, 32);
        assert_eq!(&s[..16], &s[16..32], "period is not 16 shifts");
        assert_eq!(
            s.iter().filter(|&&b| b == 1).count(),
            2,
            "exactly one high bit per 16"
        );
    }

    #[test]
    fn noise_rate_3_with_ch10_period_0_freezes_lfsr() {
        let mut a = Apu::new();

        a.write(11 * 0x40, 0b011);
        a.write(11 * 0x40 + 2, 0);
        a.write(10 * 0x40, 0);
        a.write(10 * 0x40 + 1, 0);

        run_frame(&mut a, &[]);

        let first = a.frame[0];
        assert!(first != 0, "output must be non-zero");
        assert!(
            a.frame.iter().all(|&s| s == first),
            "all samples must be equal"
        );
    }
}

use super::{tri, Apu, SAMPLE_RATE};

// FRES -> SVF Q, 0.5 (flat) to 16 (self-oscillation).
fn svf_q(fres: u8) -> f64 {
    0.5 * 2f64.powf(fres as f64 / 51.0)
}

// FLFO_RATE -> filter LFO Hz, 0.05 to ~51.2.
fn flfo_hz(rate: u8) -> f64 {
    0.05 * 2f64.powf(rate as f64 / 25.5)
}

impl Apu {
    pub fn filter_hz(fcut: u16) -> f64 {
        20.0 * 2f64.powf(fcut as f64 / 6553.6)
    }

    pub(super) fn filter(&mut self, ch: usize, x: f64) -> f64 {
        let base = ch * 0x40;
        let fctl = self.regs[base + 0x30];

        if fctl & 0x01 == 0 {
            return x;
        }

        let fcut = u16::from_be_bytes([self.regs[base + 0x32], self.regs[base + 0x33]]);
        let fres = self.regs[base + 0x34];
        let flfo_depth = self.regs[base + 0x36];
        let fenv_depth = self.regs[base + 0x37] as i8;

        let env = if ch < 8 { self.fm_env_peak(ch) } else { 0.0 };

        self.flfo_phase[ch] =
            (self.flfo_phase[ch] + flfo_hz(self.regs[base + 0x35]) / SAMPLE_RATE).fract();
        let tri_mod = tri(self.flfo_phase[ch]);

        let raw = fcut as f64 + tri_mod * flfo_depth as f64 * 64.0 + env * fenv_depth as f64 * 64.0;
        let fc = Self::filter_hz(raw.clamp(0.0, 65535.0) as u16);
        let q = svf_q(fres);

        let g = (std::f64::consts::PI * fc.min(20_480.0) / SAMPLE_RATE).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v1 = a1 * (self.ic1[ch] + g * (x - self.ic2[ch]));
        let v2 = self.ic2[ch] + g * v1;

        self.ic1[ch] = 2.0 * v1 - self.ic1[ch];
        self.ic2[ch] = 2.0 * v2 - self.ic2[ch];

        let mut y = 0.0;

        if fctl & 0b0010 != 0 {
            y += v2; // LP
        }

        if fctl & 0b0100 != 0 {
            y += v1; // BP
        }

        if fctl & 0b1000 != 0 {
            let hp = x - k * v1 - v2;
            y += hp; // HP
        }

        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apu::testkit::*;

    fn set_filter(a: &mut Apu, ch: u32, fctl: u8, fcut: u16, fres: u8) {
        let base = ch * 0x40;

        a.write(base + 0x30, fctl);
        a.write(base + 0x32, (fcut >> 8) as u8);
        a.write(base + 0x33, fcut as u8);
        a.write(base + 0x34, fres);
    }

    #[test]
    fn cutoff_map_is_exponential_20hz_to_20khz() {
        assert!((Apu::filter_hz(0) - 20.0).abs() < 0.001);
        // 20*2^(65535/6553.6); u16 max is one unit short of 10 octaves.
        assert!((Apu::filter_hz(65535) - 20477.834029605638).abs() < 0.01);
        assert!(
            (Apu::filter_hz(6554) - 40.0).abs() < 0.01,
            "one octave per 6553.6 units"
        );
    }

    #[test]
    fn lowpass_passes_dc_and_kills_treble_highpass_the_reverse() {
        // square 0 at ~110 Hz vs ~7 kHz through LP at 1 kHz
        let mut a = Apu::new();

        a.write(8 * 0x40 + 1, 16); // period 16: ~7 kHz
        a.write(8 * 0x40 + 2, 0);
        set_filter(&mut a, 8, 0b0011, 37130, 0); // enable+LP; 37130 → ≈1 kHz
        run_frame(&mut a, &[]);
        let treble_through_lp = rms(&a.frame);

        a.write(8 * 0x40 + 1, 0xFF); // period 1023: ~109 Hz
        a.write(8 * 0x40, 0x03);
        run_frame(&mut a, &[]);
        run_frame(&mut a, &[]); // settle
        let bass_through_lp = rms(&a.frame);

        assert!(
            bass_through_lp > treble_through_lp * 4.0,
            "LP slope missing"
        );

        set_filter(&mut a, 8, 0b1001, 37130, 0); // HP
        run_frame(&mut a, &[]);
        run_frame(&mut a, &[]);
        assert!(rms(&a.frame) < bass_through_lp / 4.0, "HP passed bass");
    }

    #[test]
    fn disabled_filter_is_bit_transparent() {
        let mut a = Apu::new();

        a.write(8 * 0x40 + 1, 100);
        a.write(8 * 0x40 + 2, 0);
        run_frame(&mut a, &[]);
        let dry = a.frame.clone();

        // The filter must be configured on the instance that actually runs
        // (`b`) — setting it on `a` after `a` has already produced its only
        // measured frame left the assertion comparing two dry runs no
        // matter what the filter did.
        let mut b = Apu::new();
        b.write(8 * 0x40 + 1, 100);
        b.write(8 * 0x40 + 2, 0);
        set_filter(&mut b, 8, 0b0010, 100, 200); // LP tap set but enable clear
        run_frame(&mut b, &[]);

        assert_eq!(dry, b.frame);
    }

    #[test]
    fn env_mod_is_inert_on_psg_and_flfo_sweeps_everywhere() {
        let mut a = Apu::new();

        a.write(8 * 0x40 + 1, 16);
        a.write(8 * 0x40 + 2, 0);
        set_filter(&mut a, 8, 0b0011, 30000, 0);
        a.write(8 * 0x40 + 0x37, 127); // FENV_DEPTH: no envelope on PSG → no effect
        run_frame(&mut a, &[]);
        let with_env_depth = a.frame.clone();

        // two-instance shape for the same reason.
        let mut b = Apu::new();
        b.write(8 * 0x40 + 1, 16);
        b.write(8 * 0x40 + 2, 0);
        set_filter(&mut b, 8, 0b0011, 30000, 0);
        b.write(8 * 0x40 + 0x37, 0);
        run_frame(&mut b, &[]);
        assert_eq!(b.frame, with_env_depth, "env mod moved a PSG filter");

        // Same two-instance shape for FLFO: a fresh `c` at the same frame
        // index as `with_env_depth`'s source, not `a` continuing to run —
        // reuse-across-frames passed even with tri_mod forced to 0.
        let mut c = Apu::new();
        c.write(8 * 0x40 + 1, 16); c.write(8 * 0x40 + 2, 0);
        set_filter(&mut c, 8, 0b0011, 30000, 0);
        c.write(8 * 0x40 + 0x35, 255); // FLFO fast
        c.write(8 * 0x40 + 0x36, 200); // deep
        run_frame(&mut c, &[]);
        assert_ne!(c.frame, with_env_depth, "FLFO had no effect");
    }
}

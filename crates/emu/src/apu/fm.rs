use super::{Apu, LFO, SAMPLE_RATE, tri};

pub(super) const EG_TICK_PER_SAMPLE: f64 = 7_670_453.0 / 144.0 / 3.0 / SAMPLE_RATE;
pub(super) const FM_OP_OFFSET: [u32; 4] = [0x00, 0x07, 0x0E, 0x15];

const AMS_ATT: [f64; 4] = [0.0, 15.0, 63.0, 126.0];
const FMS_DEPTH: [f64; 8] = [
    0.0,
    0.001965846759682366,
    0.0038775701558249054,
    0.005792941067853441,
    0.008119502920258315,
    0.011619440301922523,
    0.023373891996774976,
    0.04729412282062673,
];
const MOD_DEPTH: f64 = 0.25;

fn detune_hz(dt: u8, freq: f64) -> f64 {
    let sign = if dt & 0x04 != 0 { -1.0 } else { 1.0 };
    let mag = (dt & 0x03) as f64;

    if mag == 0.0 {
        return 0.0;
    }

    sign * mag * freq * 0.0006 * mag
}

fn op_eval(phase: f64, mod_in: f64, amp: f64) -> f64 {
    (std::f64::consts::TAU * (phase + mod_in)).sin() * amp
}

// SSG-EG mode bits [2:0] -> (invert, alternate, hold).
fn ssg_mode(ssg: u8) -> (bool, bool, bool) {
    match ssg & 0x07 {
        0 => (false, false, false),
        1 => (false, false, true),
        2 => (false, true, false),
        3 => (false, true, true),
        4 => (true, false, false),
        5 => (true, false, true),
        6 => (true, true, false),
        7 => (true, true, true),
        _ => unreachable!(),
    }
}

// Mod sources feeding operator `op` (0-indexed) and whether it is a carrier.
// Op 1 (index 0) never appears as a source here; its only modulation input is
// channel feedback, applied by the caller.
pub(super) fn algorithm(alg: u8, op: usize) -> (&'static [usize], bool) {
    match (alg, op) {
        (0, 0) => (&[], false),
        (0, 1) => (&[0], false),
        (0, 2) => (&[1], false),
        (0, 3) => (&[2], true),

        (1, 0) => (&[], false),
        (1, 1) => (&[], false),
        (1, 2) => (&[0, 1], false),
        (1, 3) => (&[2], true),

        (2, 0) => (&[], false),
        (2, 1) => (&[], false),
        (2, 2) => (&[1], false),
        (2, 3) => (&[0, 2], true),

        (3, 0) => (&[], false),
        (3, 1) => (&[0], false),
        (3, 2) => (&[], false),
        (3, 3) => (&[1, 2], true),

        (4, 0) => (&[], false),
        (4, 1) => (&[0], true),
        (4, 2) => (&[], false),
        (4, 3) => (&[2], true),

        (5, 0) => (&[], false),
        (5, 1) => (&[0], true),
        (5, 2) => (&[0], true),
        (5, 3) => (&[0], true),

        (6, 0) => (&[], false),
        (6, 1) => (&[0], true),
        (6, 2) => (&[], true),
        (6, 3) => (&[], true),

        (7, 0) => (&[], true),
        (7, 1) => (&[], true),
        (7, 2) => (&[], true),
        (7, 3) => (&[], true),

        _ => (&[], false),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    Attack,
    Decay,
    Sustain,
    #[default]
    Release,
}

#[derive(Clone, Copy)]
struct Eg {
    att: u16,
    phase: Phase,
    ssg_held: bool,
    ssg_inv: bool,
}

impl Default for Eg {
    fn default() -> Eg {
        Eg {
            att: 1023,
            phase: Phase::Release,
            ssg_held: false,
            ssg_inv: false,
        }
    }
}

impl Eg {
    // `ams_add` layers on top of the EG's own attenuation without mutating it.
    // SSG-EG invert reflects the attenuation around max before AMS is added.
    fn amp_with(&self, ams_add: f64) -> f64 {
        if self.phase == Phase::Release && self.att == 1023 && ams_add == 0.0 && !self.ssg_inv {
            return 0.0;
        }

        let base = if self.ssg_inv {
            1023 - self.att
        } else {
            self.att
        };
        let att = (base as f64 + ams_add).clamp(0.0, 1023.0);

        10f64.powf(-att * 0.09375 / 20.0)
    }

    // Applies one already rate/period-gated increment (see `Apu::eg_tick`).
    fn apply(&mut self, inc: u16, sl_att: u16) {
        match self.phase {
            Phase::Attack => {
                let dec = inc as u32 * ((self.att >> 4) as u32 + 1);
                self.att = if dec >= self.att as u32 {
                    0
                } else {
                    self.att - dec as u16
                };
                if self.att == 0 {
                    self.phase = Phase::Decay;
                }
            }
            Phase::Decay => {
                self.att = (self.att + inc).min(1023);
                if self.att >= sl_att {
                    self.phase = Phase::Sustain;
                }
            }
            Phase::Sustain | Phase::Release => {
                self.att = (self.att + inc).min(1023);
            }
        }
    }

    fn audible(&self) -> bool {
        self.phase == Phase::Attack || self.att < 1023
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct FmCh {
    env: [Eg; 4],
    fb: [f64; 2],
    phase: [f64; 4],
}

impl Apu {
    pub fn fm_freq_hz(fnum: u16, block: u8) -> f64 {
        fnum as f64 * 2f64.powi(block as i32 - 1) * (7_670_453.0 / 144.0) / 1_048_576.0
    }

    pub(super) fn eg_tick(&mut self) {
        self.eg_counter = self.eg_counter.wrapping_add(1);

        for ch in 0..8 {
            let base = ch * 0x40;
            let per_op_freq = self.regs[base + 0x28] & 0x01 != 0;
            let chan_freq_word =
                u16::from_be_bytes([self.regs[base + 0x1C], self.regs[base + 0x1D]]);

            for (op, &off) in FM_OP_OFFSET.iter().enumerate() {
                let o = base + off as usize;
                let freq_word = if per_op_freq {
                    let fo = base + 0x20 + 2 * op;
                    u16::from_be_bytes([self.regs[fo], self.regs[fo + 1]])
                } else {
                    chan_freq_word
                };

                let block = (freq_word >> 11) & 0x07;
                let fnum = freq_word & 0x7FF;
                let keycode = (block << 2) | (fnum >> 9);

                let ks = (self.regs[o + 2] >> 6) & 0x03;
                let ar = self.regs[o + 2] & 0x1F;
                let dr = self.regs[o + 3] & 0x1F;
                let sr = self.regs[o + 4] & 0x1F;
                let sl = (self.regs[o + 5] >> 4) & 0x0F;
                let rr = self.regs[o + 5] & 0x0F;
                let ks_add = keycode >> (3 - ks as u16);
                let sl_att = if sl == 15 { 1023 } else { (sl as u16) << 5 };

                let phase = self.fm[ch].env[op].phase;
                let rate = match phase {
                    Phase::Attack => ar,
                    Phase::Decay => dr,
                    Phase::Sustain => sr,
                    Phase::Release => 2 * rr + 1,
                };

                if rate == 0 {
                    continue; // phase param 0: this phase never moves
                }

                let ssg = self.regs[o + 6];
                let ssg_active = ssg & 0x08 != 0 && phase != Phase::Attack;

                let r = (2 * rate as u16 + ks_add).min(63);
                let inc = if r < 48 {
                    let mut shift = 11 - (r >> 2);
                    if ssg_active {
                        shift = shift.saturating_sub(2);
                    }
                    if self.eg_counter & ((1 << shift) - 1) != 0 {
                        continue; // not this tick's turn for this R's period
                    }
                    4 + (r & 3)
                } else {
                    let mut inc = (4 + (r & 3)) << ((r >> 2) - 11);
                    if ssg_active {
                        inc <<= 2;
                    }
                    inc
                };

                let eg = &mut self.fm[ch].env[op];
                if ssg_active && eg.ssg_held {
                    continue; // hold mode: frozen since the last threshold cross
                }

                eg.apply(inc, sl_att);

                if ssg_active && eg.att >= 0x200 {
                    let (_, alt, hold) = ssg_mode(ssg);
                    if hold {
                        // alt+hold: one alternation on the way into hold, then frozen.
                        if alt && !eg.ssg_held {
                            eg.ssg_inv = !eg.ssg_inv;
                        }

                        eg.ssg_held = true;
                    } else {
                        eg.att -= 0x200; // loop: wrap back below the threshold
                        if alt {
                            eg.ssg_inv = !eg.ssg_inv;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn fm(&mut self, ch: usize) -> f64 {
        let base = ch * 0x40;
        let alg = self.regs[base + 0x1E] & 0x07;
        let fb_reg = (self.regs[base + 0x1E] >> 3) & 0x07;
        let pan_ams_fms = self.regs[base + 0x1F];
        let ams = ((pan_ams_fms >> 4) & 0x03) as usize;
        let fms = (pan_ams_fms & 0x07) as usize;
        let ctrl = self.regs[base + 0x28];
        let per_op_freq = ctrl & 0x01 != 0;

        let lfo_word = self.regs[LFO as usize];
        let lfo_on = lfo_word & 0x08 != 0;
        let lfo_tri = if lfo_on { tri(self.lfo_phase) } else { 0.0 };

        let chan_freq_word = u16::from_be_bytes([self.regs[base + 0x1C], self.regs[base + 0x1D]]);

        let mut op_freq = [0.0; 4];
        let mut tl = [0u8; 4];
        let mut ams_add = [0.0; 4];

        for op in 0..4 {
            let o = base + FM_OP_OFFSET[op] as usize;
            let dt_mul = self.regs[o];
            let dt = (dt_mul >> 4) & 0x07;
            let mul = dt_mul & 0x0F;
            let mul_factor = if mul == 0 { 0.5 } else { mul as f64 };

            let freq_word = if per_op_freq {
                let fo = base + 0x20 + 2 * op;
                u16::from_be_bytes([self.regs[fo], self.regs[fo + 1]])
            } else {
                chan_freq_word
            };
            let block = ((freq_word >> 11) & 0x07) as u8;
            let fnum = freq_word & 0x7FF;
            let mut base_freq = Self::fm_freq_hz(fnum, block);
            if lfo_on {
                base_freq *= 1.0 + FMS_DEPTH[fms] * lfo_tri;
            }

            let f = base_freq * mul_factor;
            op_freq[op] = f + detune_hz(dt, f);
            tl[op] = self.regs[o + 1] & 0x7F;

            if lfo_on && self.regs[o + 3] & 0x80 != 0 {
                ams_add[op] = AMS_ATT[ams] * (lfo_tri + 1.0) / 2.0;
            }
        }

        if ctrl & 0x02 != 0 && self.sync_wrap[(ch + 7) % 8] {
            self.fm[ch].phase = [0.0; 4];
        }

        let op0_before = self.fm[ch].phase[0];
        for op in 0..4 {
            self.fm[ch].phase[op] = (self.fm[ch].phase[op] + op_freq[op] / SAMPLE_RATE).fract();
        }
        self.sync_wrap_next[ch] = self.fm[ch].phase[0] < op0_before;

        let fmch = &mut self.fm[ch];

        let fb_scale = if fb_reg == 0 {
            0.0
        } else {
            2f64.powi(fb_reg as i32 - 5)
        };
        let fb_mod_in = (fmch.fb[0] + fmch.fb[1]) / 2.0 * fb_scale * MOD_DEPTH;

        let mut out = [0.0; 4];
        for op in 0..4 {
            let mod_in = if op == 0 {
                fb_mod_in
            } else {
                let (srcs, _) = algorithm(alg, op);
                srcs.iter().map(|&s| out[s]).sum::<f64>() * MOD_DEPTH
            };
            let amp = fmch.env[op].amp_with(ams_add[op]) * 10f64.powf(-0.75 * tl[op] as f64 / 20.0);

            out[op] = op_eval(fmch.phase[op], mod_in, amp);
        }

        fmch.fb[1] = fmch.fb[0];
        fmch.fb[0] = out[0];

        let (carrier_sum, carrier_count) = (0..4)
            .filter(|&op| algorithm(alg, op).1)
            .fold((0.0, 0u32), |(sum, count), op| (sum + out[op], count + 1));
        let mut carrier_out = carrier_sum / carrier_count as f64;

        if ctrl & 0x04 != 0 {
            carrier_out *= self.prev_out[(ch + 7) % 8];
        }
        self.prev_out[ch] = carrier_out;

        carrier_out
    }

    pub(super) fn fm_audible(&self, ch: usize) -> bool {
        self.fm[ch].env.iter().any(Eg::audible)
    }

    pub(super) fn fm_env_peak(&self, ch: usize) -> f64 {
        let alg = self.regs[ch * 0x40 + 0x1E] & 0x07;

        (0..4)
            .filter(|&op| algorithm(alg, op).1)
            .map(|op| self.fm[ch].env[op].amp_with(0.0))
            .fold(0.0, f64::max)
    }

    pub(super) fn fm_key(&mut self, ch: usize, mask: u8) {
        let base = ch * 0x40;
        let ssg_init: [(bool, bool); 4] = std::array::from_fn(|op| {
            let ssg = self.regs[base + FM_OP_OFFSET[op] as usize + 6];
            let (invert, _, _) = ssg_mode(ssg);

            (ssg & 0x08 != 0, invert)
        });

        for (op, eg) in self.fm[ch].env.iter_mut().enumerate() {
            if mask & (0x10 << op) != 0 {
                eg.att = 1023;
                eg.phase = Phase::Attack;
                eg.ssg_held = false;
                eg.ssg_inv = ssg_init[op].0 && ssg_init[op].1;
            } else {
                eg.phase = Phase::Release;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apu::testkit::*;

    #[test]
    fn fm_pitch_formula_matches_the_spec() {
        assert!((Apu::fm_freq_hz(1083, 4) - 440.13).abs() < 0.01);
        assert!((Apu::fm_freq_hz(1083, 3) - 220.06).abs() < 0.01);
    }

    #[test]
    fn algorithm_7_sums_four_carriers_and_tl_gates_each() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);

        for op in 1..4 {
            a.write(op * 7 + 1, 127); // ops 2-4 TL max = silent
        }

        a.write(KEYON_ADDR, 0xF0); // all ops, ch 0

        run_frame(&mut a, &[]);
        let one_op = peak(&a.frame);

        a.write(7 + 1, 0); // op 2 TL 0
        run_frame(&mut a, &[]);

        assert!(peak(&a.frame) > one_op * 1.7, "second carrier did not add");
    }

    #[test]
    fn modulation_changes_the_waveform() {
        let mut a = Apu::new();
        fm_setup(&mut a, 0, 0, 1083, 4); // alg 0: 1->2->3->4 chain
        a.write(KEYON_ADDR, 0xF0);
        run_frame(&mut a, &[]);

        let mut b = Apu::new();
        fm_setup(&mut b, 0, 0, 1083, 4);
        b.write(0x01, 127); // silence the modulator chain's head
        b.write(KEYON_ADDR, 0xF0);
        run_frame(&mut b, &[]);

        assert_ne!(a.frame, b.frame, "op 1 TL must alter the carrier's timbre");
    }

    #[test]
    fn fm_channel_frequency_is_audible_at_440() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);

        for op in 1..4 {
            a.write(op * 7 + 1, 127);
        }

        a.write(KEYON_ADDR, 0xF0);

        assert_eq!(cycles_per_second(&mut a), 440);
    }

    #[test]
    fn keyon_starts_the_attack_and_keyoff_releases() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);
        a.write(0x05, 0x0F); // op 1 SL=0, RR=15
        a.write(KEYON_ADDR, 0x10); // op 1 only

        run_frame(&mut a, &[]);
        assert!(peak(&a.frame) > 0.0, "keyed channel is silent");

        a.write(KEYON_ADDR, 0x00); // key off

        for _ in 0..30 {
            run_frame(&mut a, &[]);
        }

        assert!(
            a.frame.iter().all(|&s| s == 0),
            "released channel still sounds after 0.5 s"
        );
    }

    #[test]
    fn fms_vibrato_widens_the_spectrum_and_ams_pulses_the_amplitude() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);
        a.write(KEYON_ADDR, 0xF0);
        run_frame(&mut a, &[]);
        let steady = a.frame.clone();

        let mut b = Apu::new();
        fm_setup(&mut b, 0, 7, 1083, 4);
        b.write(LFO_ADDR, 0x0F); // LFO on, 72.2 Hz
        b.write(0x1F, 0xC0 | 0x07); // pan L|R, FMS max
        b.write(KEYON_ADDR, 0xF0);
        run_frame(&mut b, &[]);
        assert_ne!(b.frame, steady, "FMS did not modulate");

        a.write(LFO_ADDR, 0x0F); // LFO on, 72.2 Hz
        a.write(0x1F, 0xC0 | 0x30); // AMS max, FMS off
        a.write(0x03, 0x80); // op 1 AM enable (AM/DR byte)

        let peaks: Vec<i16> = (0..8)
            .map(|_| {
                run_frame(&mut a, &[]);
                peak_i(&a.frame)
            })
            .collect();

        assert!(
            peaks.iter().max() > peaks.iter().min(),
            "AMS did not pulse the amplitude"
        );
    }

    #[test]
    fn ssg_eg_hold_mode_keeps_audible_a_channel_that_would_otherwise_drain_to_silence() {
        const FRAMES: usize = 1;

        let mut a = Apu::new();
        fm_setup(&mut a, 0, 7, 1083, 4);
        a.write(0x03, 31); // op 1 DR max (AM/DR byte, AM disabled)
        a.write(0x05, 0xF0); // op 1 SL=15 (decay target = full attenuation), RR=0
        a.write(KEYON_ADDR, 0x10);

        for _ in 0..FRAMES {
            run_frame(&mut a, &[]);
        }

        assert_eq!(status(&a) & 1, 0, "arm A (no SSG-EG) must drain to silence");

        let mut b = Apu::new();
        fm_setup(&mut b, 0, 7, 1083, 4);
        b.write(0x03, 31);
        b.write(0x05, 0xF0);
        b.write(0x06, 0b1101); // SSG-EG: enable, invert + hold
        b.write(KEYON_ADDR, 0x10);

        for _ in 0..FRAMES {
            run_frame(&mut b, &[]);
        }

        assert_eq!(status(&b) & 1, 1, "SSG hold mode must keep the op audible");
    }

    #[test]
    fn ssg_eg_alt_plus_hold_applies_one_toggle_before_freezing() {
        const SKIP: usize = 50; // stereo samples: past the pre-hold Attack/Decay transient

        fn drain_setup(a: &mut Apu, ssg: u8) {
            fm_setup(a, 0, 7, 1083, 4);
            a.write(0x03, 31); // op 1 DR max
            a.write(0x05, 0xF0); // op 1 SL=15, RR=0
            a.write(0x06, ssg);
            a.write(KEYON_ADDR, 0x10);
        }

        let mut hold_only = Apu::new();
        drain_setup(&mut hold_only, 0b1001); // SSG-EG: enable, hold, no alternate
        run_frame(&mut hold_only, &[]);
        let hold_tail = tail_peak(&hold_only.frame, SKIP);

        let mut alt_hold = Apu::new();
        drain_setup(&mut alt_hold, 0b1011); // SSG-EG: enable, alternate + hold
        run_frame(&mut alt_hold, &[]);
        let alt_hold_tail = tail_peak(&alt_hold.frame, SKIP);

        assert!(
            alt_hold_tail > hold_tail,
            "alt+hold's one toggle must hold at the louder, reflected level"
        );
    }

    #[test]
    fn ams_cannot_un_silence_a_tl_max_operator() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);
        a.write(0x01, 127); // op 1 TL max
        a.write(0x03, 0x80); // op 1 AM enable
        a.write(0x1F, 0xC0 | 0x30); // AMS max, FMS off
        a.write(LFO_ADDR, 0x0F); // LFO on, max rate
        a.write(KEYON_ADDR, 0x10); // op 1 only

        run_frame(&mut a, &[]);
        assert!(
            a.frame.iter().all(|&s| s == 0),
            "AMS must not un-silence a TL-max operator"
        );
    }

    #[test]
    fn per_op_freq_detaches_operators_from_the_channel_pitch() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);
        for op in 1..4 {
            a.write(op * 7 + 1, 127);
        }
        a.write(0x28, 0x01);
        write_be16(&mut a, 0x20, (3 << 11) | 1083); // op 1 freq: block 3 = 220 Hz
        a.write(KEYON_ADDR, 0xF0);

        assert_eq!(cycles_per_second(&mut a), 220, "per-op freq not honored");
    }

    #[test]
    fn per_op_freq_also_drives_the_envelope_key_scaling() {
        fn drain_frames(a: &mut Apu, per_op: bool, chan: (u16, u8), op_freq: (u16, u8)) -> usize {
            for &o in &FM_OP_OFFSET {
                a.write(o, 0x01); // DT 0, MUL 1
                a.write(o + 1, 0); // TL 0
                a.write(o + 2, 0xC0 | 0x1F); // KS 3 (full effect), AR 31
                a.write(o + 5, 0x06); // SL 0, RR 6
            }
            a.write(0x1E, 7); // alg 7, FB 0
            write_be16(a, 0x1C, ((chan.1 as u16) << 11) | chan.0);

            if per_op {
                a.write(0x28, 0x01);
                for op in 0..4u32 {
                    write_be16(a, 0x20 + 2 * op, ((op_freq.1 as u16) << 11) | op_freq.0);
                }
            }

            a.write(KEYON_ADDR, 0xF0); // ch 0, all ops
            run_frame(a, &[]);
            a.write(KEYON_ADDR, 0x00); // key off

            let mut n = 0;
            while status(a) & 1 != 0 {
                run_frame(a, &[]);
                n += 1;
                assert!(n < 4000, "never went silent");
            }
            n
        }

        let high = (1000u16, 7u8); // high keycode: fast KS-boosted release
        let low = (200u16, 0u8); // low keycode: slow release

        let mut a = Apu::new();
        let per_op_arm = drain_frames(&mut a, true, low, high);

        let mut b = Apu::new();
        let high_arm = drain_frames(&mut b, false, high, high);

        let mut c = Apu::new();
        let low_arm = drain_frames(&mut c, false, low, high);

        assert_eq!(per_op_arm, 1, "MEASURED: per-op freq's keycode drain");
        assert_eq!(
            high_arm, 1,
            "MEASURED: high-pitch channel freq's keycode drain"
        );
        assert_eq!(
            low_arm, 19,
            "MEASURED: low-pitch channel freq's keycode drain"
        );
        assert_eq!(
            per_op_arm, high_arm,
            "per-op freq must use the per-op keycode, not the channel's"
        );
        assert!(
            per_op_arm < low_arm,
            "high per-op keycode must drain faster than the low channel keycode"
        );
    }

    #[test]
    fn hard_sync_locks_the_follower_to_the_leader_period() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4); // leader: 440 Hz
        fm_setup(&mut a, 1, 7, 1200, 3); // follower alone would be ~244 Hz
        a.write(0x40 + 0x28, 0x02); // ch 1: sync
        a.write(KEYON_ADDR, 0xF0);
        a.write(KEYON_ADDR, 0xF1);

        // silence the leader's audio, keep its phase running: pan gates closed
        a.write(0x1F, 0x00);

        assert_eq!(
            cycles_per_second(&mut a),
            440,
            "follower did not reset at the leader rate"
        );
    }

    #[test]
    fn ring_mod_produces_sum_and_difference_not_the_carrier() {
        let mut a = Apu::new();

        fm_setup(&mut a, 0, 7, 1083, 4);
        fm_setup(&mut a, 1, 7, 1083, 5);
        run_frame(&mut a, &[]);
        write_be16(&mut a, 0x40 + 0x1C, (4 << 11) | 1083); // retune ch 1 to match ch 0
        a.write(0x40 + 0x28, 0x04); // ch 1: ring with ch 0
        a.write(0x1F, 0x00); // ch 0 inaudible, still running
        a.write(KEYON_ADDR, 0xF0);
        a.write(KEYON_ADDR, 0xF1);

        // sin×sin at the same frequency = DC; cos(2f): zero crossings double
        assert_eq!(
            cycles_per_second(&mut a),
            880,
            "ring mod did not fold to sum/difference"
        );
    }

    #[test]
    fn envelope_durations_match_the_measured_goldens() {
        for (rr, frames) in [(4u8, 56usize), (8, 4)] {
            let mut a = Apu::new();

            fm_setup(&mut a, 0, 7, 1083, 4);
            a.write(0x05, rr); // SL=0, RR=rr
            a.write(KEYON_ADDR, 0x10);
            run_frame(&mut a, &[]);
            a.write(KEYON_ADDR, 0x00);

            let mut n = 0;
            while status(&a) & 1 != 0 {
                run_frame(&mut a, &[]);
                n += 1;
                assert!(n < 4000, "never went silent");
            }

            assert_eq!(n, frames, "RR={rr}");
        }
    }
}

pub mod out;

mod fm;
mod pcm;
mod psg;
mod svf;
#[cfg(test)]
mod testkit;

pub const CH_COUNT: usize = 14;
pub const CH_STRIDE: u32 = 0x40;
pub const PAN_OFF: [usize; 14] = [
    0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, // FM ch 0-7
    0x03, 0x03, 0x03, 0x03, // PSG ch 8-11
    0x0F, 0x0F, // PCM ch 12-13
];
pub const SAMPLES_PER_LINE: usize = 4;

const GLOBAL: u32 = 0x400;
const KEYON: u32 = GLOBAL;
const LFO: u32 = GLOBAL + 1;
/// LFO reg [2:0] rate index -> Hz.
const LFO_RATE_HZ: [f64; 8] = [3.98, 5.56, 6.02, 6.37, 6.88, 9.63, 48.1, 72.2];
const MASTER: f64 = 1.0 / 6.0;
const REGS: usize = 0x500;
const SAMPLE_RATE: f64 = 48_000.0;
const STATUS: u32 = GLOBAL + 2;

// Triangle wave over phase [0,1), range [-1,1], 0 at phase 0.
fn tri(phase: f64) -> f64 {
    let p = phase.rem_euclid(1.0);

    if p < 0.25 {
        p * 4.0
    } else if p < 0.75 {
        1.0 - (p - 0.25) * 4.0
    } else {
        (p - 1.0) * 4.0
    }
}

pub struct Apu {
    eg_acc: f64,
    eg_counter: u32,
    flfo_phase: [f64; CH_COUNT],
    fm: [fm::FmCh; 8],
    pub frame: Vec<i16>,
    ic1: [f64; CH_COUNT],
    ic2: [f64; CH_COUNT],
    lfo_phase: f64,
    lfsr: u16,
    noise_phase: f64,
    pcm: [pcm::Pcm; 2],
    phase: [f64; 3],
    prev_out: [f64; 8],
    regs: [u8; REGS],
    sync_wrap: [bool; 8],
    sync_wrap_next: [bool; 8],
}

impl Default for Apu {
    fn default() -> Apu {
        Apu {
            eg_acc: 0.0,
            eg_counter: 0,
            flfo_phase: [0.0; CH_COUNT],
            fm: [fm::FmCh::default(); 8],
            frame: Vec::new(),
            ic1: [0.0; CH_COUNT],
            ic2: [0.0; CH_COUNT],
            lfo_phase: 0.0,
            lfsr: 0x8000,
            noise_phase: 0.0,
            pcm: [pcm::Pcm::default(); 2],
            phase: [0.0; 3],
            prev_out: [0.0; 8],
            regs: [0; REGS],
            sync_wrap: [false; 8],
            sync_wrap_next: [false; 8],
        }
    }
}

impl Apu {
    pub fn new() -> Apu {
        let mut a = Apu::default();
        a.reset();

        a
    }

    pub fn read(&self, offset: u32) -> u8 {
        if offset == STATUS {
            return (self.status_word() >> 8) as u8;
        }
        if offset == STATUS + 1 {
            return self.status_word() as u8;
        }
        if Self::reserved(offset) || offset == KEYON || offset >= REGS as u32 {
            return 0;
        }

        self.regs[offset as usize]
    }

    pub fn reset(&mut self) {
        *self = Apu::default();

        for ch in 0..8 {
            self.regs[(ch * 0x40 + 0x1F) as usize] = 0xC0; // FM pan L|R
        }

        for ch in 8..12 {
            self.regs[(ch * 0x40 + 0x02) as usize] = 15; // PSG silent
            self.regs[(ch * 0x40 + 0x03) as usize] = 0xC0;
        }

        for ch in 12..14 {
            self.regs[(ch * 0x40 + 0x0E) as usize] = 255; // PCM full volume
            self.regs[(ch * 0x40 + 0x0F) as usize] = 0xC0;
        }
    }

    pub fn run_line(&mut self, mem: &[u8], line: u32) {
        if line == 0 {
            self.frame.clear();
        }

        for _ in 0..SAMPLES_PER_LINE {
            let (l, r) = self.mix(mem);

            self.frame.push((l.clamp(-1.0, 1.0) * 32767.0) as i16);
            self.frame.push((r.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
    }

    pub fn write(&mut self, offset: u32, mask: u8) {
        if Self::reserved(offset) || offset >= REGS as u32 {
            return;
        }

        self.regs[offset as usize] = mask;

        if offset == 11 * 0x40 {
            self.lfsr = 0x8000;
        }

        if offset == KEYON {
            let ch = (mask & 0x0F) as usize;

            if ch < 8 {
                self.fm_key(ch, mask);
            } else if ch == 12 || ch == 13 {
                self.pcm_key(ch, mask);
            }
        }
    }

    fn mix(&mut self, mem: &[u8]) -> (f64, f64) {
        self.eg_acc += fm::EG_TICK_PER_SAMPLE;
        while self.eg_acc >= 1.0 {
            self.eg_acc -= 1.0;
            self.eg_tick();
        }

        let lfo_word = self.regs[LFO as usize];
        if lfo_word & 0x08 != 0 {
            let hz = LFO_RATE_HZ[(lfo_word & 0x07) as usize];
            self.lfo_phase = (self.lfo_phase + hz / SAMPLE_RATE).fract();
        }

        let (mut l, mut r) = (0.0, 0.0);

        for ch in 0..CH_COUNT {
            let src = match ch {
                0..=7 => self.fm(ch),
                8..=10 => self.square(ch),
                11 => self.noise(),
                12 | 13 => self.pcm(ch, mem),
                _ => 0.0,
            };
            let s = self.filter(ch, src);
            let pan = self.regs[ch * 0x40 + PAN_OFF[ch]];

            if pan & 0x80 != 0 {
                l += s;
            }
            if pan & 0x40 != 0 {
                r += s;
            }
        }

        self.sync_wrap = self.sync_wrap_next;

        (l * MASTER, r * MASTER)
    }

    fn reserved(offset: u32) -> bool {
        (14 * CH_STRIDE..GLOBAL).contains(&offset)
    }

    fn status_word(&self) -> u16 {
        let mut s: u16 = 0;

        for ch in 0..8 {
            if self.fm_audible(ch) {
                s |= 1 << ch;
            }
        }
        for ch in 8..12 {
            if self.regs[ch * 0x40 + 2] & 0x0F < 15 {
                s |= 1 << ch;
            }
        }
        for (i, pcm) in self.pcm.iter().enumerate() {
            if pcm.playing {
                s |= 1 << (12 + i);
            }
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    #[test]
    fn registers_read_back_byte_granular() {
        let mut a = Apu::new();

        a.write(0x00, 0x71); // ch 0 op 1 DT/MUL
        a.write(0x1C, 0x23); // ch 0 FREQ hi
        a.write(0x1D, 0xAB); // ch 0 FREQ lo

        assert_eq!(a.read(0x00), 0x71);
        assert_eq!(a.read(0x1C), 0x23);
        assert_eq!(a.read(0x1D), 0xAB);
    }

    #[test]
    fn reset_defaults_are_silent_until_keyed_audible_when_keyed() {
        let a = Apu::new();

        for ch in 0..14u32 {
            assert_eq!(
                a.read(ch * 0x40 + PAN_OFF[ch as usize] as u32),
                0xC0,
                "ch {ch} pan"
            );
        }

        for ch in 8..12u32 {
            assert_eq!(a.read(ch * 0x40 + 0x02), 15, "PSG ch {ch} atten");
        }

        assert_eq!(a.read(12 * 0x40 + 0x0E), 255, "PCM vol");
        assert_eq!(a.read(0x00), 0, "FM regs zero");
    }

    #[test]
    fn reserved_pages_ignore_writes_and_read_zero() {
        let mut a = Apu::new();

        a.write(14 * 0x40, 0xFF);
        a.write(15 * 0x40 + 0x3F, 0xFF);

        assert_eq!(a.read(14 * 0x40), 0);
        assert_eq!(a.read(15 * 0x40 + 0x3F), 0);
    }

    #[test]
    fn read_out_of_range_offset_returns_zero_instead_of_panicking() {
        let a = Apu::new();

        assert_eq!(a.read(REGS as u32), 0);
        assert_eq!(a.read(REGS as u32 + 1000), 0);
    }

    #[test]
    fn reset_clears_every_runtime_field_a_fresh_apu_starts_with() {
        // reset() must be the single place runtime state is cleared: run a
        // square through a frame (advances its oscillator phase), reset,
        // configure it identically again and run one more frame — it must
        // match a brand-new, identically configured Apu bit for bit. This
        // catches a field (e.g. the square oscillators' `phase`) that
        // `reset()` forgets to clear but `new()` happened to zero anyway.
        fn configure(a: &mut Apu) {
            a.write(8 * 0x40 + 1, 100); // period
            a.write(8 * 0x40 + 2, 0); // atten 0: audible
        }

        let mut a = Apu::new();
        configure(&mut a);
        run_frame(&mut a, &[]);

        a.reset();
        configure(&mut a);
        run_frame(&mut a, &[]);

        let mut fresh = Apu::new();
        configure(&mut fresh);
        run_frame(&mut fresh, &[]);

        assert_eq!(
            a.frame, fresh.frame,
            "reset() left runtime state (e.g. oscillator phase) that a fresh Apu does not have"
        );
    }

    #[test]
    fn a_frame_is_800_stereo_pairs() {
        let mut a = Apu::new();

        run_frame(&mut a, &[]);
        assert_eq!(a.frame.len(), 1600);

        run_frame(&mut a, &[]);
        assert_eq!(a.frame.len(), 1600, "line 0 did not clear the frame");
    }

    #[test]
    fn silent_apu_outputs_zeros() {
        let mut a = Apu::new();

        run_frame(&mut a, &[]);
        assert!(a.frame.iter().all(|&s| s == 0));
    }

    #[test]
    fn pan_gates_left_and_right() {
        let mut a = Apu::new();

        a.write(8 * 0x40 + 1, 100);
        a.write(8 * 0x40 + 2, 0);
        a.write(8 * 0x40 + 3, 0x80); // L only

        run_frame(&mut a, &[]);
        assert!(a.frame.chunks(2).all(|s| s[1] == 0));
        assert!(a.frame.chunks(2).any(|s| s[0] != 0));
    }

    #[test]
    fn status_tracks_fm_and_psg_audibility() {
        let mut a = Apu::new();

        assert_eq!(status(&a), 0);

        fm_setup(&mut a, 2, 7, 1083, 4);
        a.write(KEYON_ADDR, 0xF2);
        run_frame(&mut a, &[]);
        assert_eq!(status(&a) & (1 << 2), 1 << 2);

        a.write(8 * 0x40 + 2, 3); // square 0 audible
        assert_eq!(status(&a) & (1 << 8), 1 << 8);

        a.write(KEYON_ADDR, 0x02);

        for _ in 0..1050 {
            run_frame(&mut a, &[]); // RR=0's period-gated release drains in ~16.5 s
        }

        assert_eq!(
            status(&a) & (1 << 2),
            0,
            "released FM channel still reads audible"
        );
    }
}

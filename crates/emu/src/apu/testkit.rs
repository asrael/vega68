use super::fm::FM_OP_OFFSET;
use super::Apu;

pub(super) const KEYON_ADDR: u32 = 0x400;
pub(super) const LFO_ADDR: u32 = 0x401;
pub(super) const STATUS_OFF: u32 = 0x402;

pub(super) fn status(a: &Apu) -> u16 {
    ((a.read(STATUS_OFF) as u16) << 8) | a.read(STATUS_OFF + 1) as u16
}

pub(super) fn run_frame(a: &mut Apu, mem: &[u8]) {
    for line in 0..200 {
        a.run_line(mem, line);
    }
}

pub(super) fn peak(frame: &[i16]) -> f64 {
    frame.iter().map(|&s| (s as f64).abs()).fold(0.0, f64::max) / 32767.0
}

pub(super) fn peak_i(frame: &[i16]) -> i16 {
    frame.iter().map(|&s| s.abs()).max().unwrap()
}

// Peak over the frame's tail (from `skip` stereo samples on), past any
// pre-hold transient at the start of a frame.
pub(super) fn tail_peak(frame: &[i16], skip: usize) -> i16 {
    frame[skip * 2..].iter().map(|&s| s.abs()).max().unwrap()
}

pub(super) fn rms(frame: &[i16]) -> f64 {
    (frame.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / frame.len() as f64).sqrt()
}

pub(super) fn write_be16(a: &mut Apu, offset: u32, v: u16) {
    a.write(offset, (v >> 8) as u8);
    a.write(offset + 1, v as u8);
}

pub(super) fn fm_setup(a: &mut Apu, ch: usize, alg: u8, fnum: u16, block: u8) {
    let base = (ch * 0x40) as u32;

    for &o in &FM_OP_OFFSET {
        a.write(base + o, 0x01); // DT 0, MUL 1
        a.write(base + o + 1, 0); // TL 0
        a.write(base + o + 2, 0x1F); // KS 0, AR 31
        a.write(base + o + 5, 0); // SL 0, RR 0
    }

    a.write(base + 0x1E, alg); // FB 0
    write_be16(a, base + 0x1C, ((block as u16) << 11) | fnum);
}

pub(super) fn cycles_per_second(a: &mut Apu) -> usize {
    let (mut edges, mut last) = (0, 0i16);

    for _ in 0..60 {
        run_frame(a, &[]);

        for s in a.frame.chunks(2) {
            if last < 0 && s[0] > 0 {
                edges += 1;
            }

            if s[0] != 0 {
                last = s[0];
            }
        }
    }

    edges
}

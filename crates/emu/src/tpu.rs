use crate::bus::TPU_RAM_BASE;

pub const PIXEL_BUDGET: u32 = 833_333;

const BAYER: [[i32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

const FILL_COLOR: u8 = 0x01;
const FILL_WORDS: u16 = 5;
const FILL_Z: u8 = 0x02;
const OP_FILL: u8 = 0x02;

const OP_TRI: u8 = 0x01;
const TRI_BLEND: u8 = 0x01;
const TRI_WORDS: u16 = 25;
const TRI_ZGREATER: u8 = 0x02;
const TRI_ZTEST_OFF: u8 = 0x04;
const TRI_ZWRITE_OFF: u8 = 0x08;

pub fn run(state: &mut Tpu, mem: &mut [u8]) {
    let block = read_state_block(mem);

    if !valid_ring(block.ring_words) {
        state.head = state.tail;
        return;
    }

    let mut left = state.tail.wrapping_sub(state.head);
    while left > 0 && state.budget > 0 {
        let word0 = ring_word(mem, &block, state.head, 0);
        let opcode = (word0 >> 24) as u8;

        let (cost, words) = match opcode {
            OP_FILL => (exec_fill(mem, &block, state.head), FILL_WORDS),
            OP_TRI => (exec_tri(mem, &block, state.head), TRI_WORDS),
            _ => (0, 1),
        };

        charge(state, cost);

        if words > left {
            state.head = state.tail;
            break;
        }
        left -= words;
        state.head = state.head.wrapping_add(words);
    }
}

fn charge(state: &mut Tpu, cost: u32) {
    state.pixels += cost;

    if cost > state.budget {
        state.deficit += cost - state.budget;
        state.budget = 0;
    } else {
        state.budget -= cost;
    }
}

fn covered(e: &[i64; 3], tl: &[bool; 3]) -> bool {
    inside(e[0], tl[0]) && inside(e[1], tl[1]) && inside(e[2], tl[2])
}

fn edge_at(a: &Vertex, b: &Vertex, px: i32, py: i32) -> i64 {
    (b.x - a.x) as i64 * (py - a.y) as i64 - (b.y - a.y) as i64 * (px - a.x) as i64
}

fn edges(vs: &[Vertex; 3], cx: i32, cy: i32) -> [i64; 3] {
    [
        edge_at(&vs[1], &vs[2], cx, cy),
        edge_at(&vs[2], &vs[0], cx, cy),
        edge_at(&vs[0], &vs[1], cx, cy),
    ]
}

fn exec_fill(mem: &mut [u8], block: &StateBlock, head: u16) -> u32 {
    let w0 = ring_word(mem, block, head, 0);
    let w1 = ring_word(mem, block, head, 1);
    let w2 = ring_word(mem, block, head, 2);
    let w3 = ring_word(mem, block, head, 3);
    let w4 = ring_word(mem, block, head, 4);

    let flags = w0 as u8;
    let x0 = (w1 >> 16) as u16 as i16 as i32;
    let y0 = w1 as u16 as i16 as i32;
    let x1 = (w2 >> 16) as u16 as i16 as i32;
    let y1 = w2 as u16 as i16 as i32;
    let color = w3 as u8;
    let z = w4 as u16;

    let width = block.width as i32;
    let height = block.height as i32;
    let cx0 = x0.clamp(0, width);
    let cy0 = y0.clamp(0, height);
    let cx1 = x1.clamp(0, width);
    let cy1 = y1.clamp(0, height);

    if cx1 <= cx0 || cy1 <= cy0 {
        return 0;
    }

    let do_color = flags & FILL_COLOR != 0;
    let do_z = flags & FILL_Z != 0;

    for y in cy0..cy1 {
        for x in cx0..cx1 {
            let px = y as u64 * block.width as u64 + x as u64;

            if do_color {
                write_u8(mem, block.color_base as u64 + px, color);
            }
            if do_z {
                write_u16(mem, block.z_base as u64 + px * 2, z);
            }
        }
    }

    let covered = (cx1 - cx0) as u32 * (cy1 - cy0) as u32;
    covered.div_ceil(4)
}

fn exec_tri(mem: &mut [u8], block: &StateBlock, head: u16) -> u32 {
    let w0 = ring_word(mem, block, head, 0);
    let w1 = ring_word(mem, block, head, 1);
    let colormap = ring_word(mem, block, head, 2) as u64;
    let blend_base = ring_word(mem, block, head, 3) as u64;

    let flags = w0 as u8;
    let lod_bias = (w0 >> 8) as u8 as i8 as i64;
    let tex = Texture {
        base: ((w1 >> 12) as u64) << 3,
        levels: ((w1 & 0xF) as usize).clamp(1, 8),
        log2h: (w1 >> 4) & 0xF,
        log2w: (w1 >> 8) & 0xF,
    };

    let mut vs = [
        read_vertex(mem, block, head, 0),
        read_vertex(mem, block, head, 1),
        read_vertex(mem, block, head, 2),
    ];

    let area = edge_at(&vs[0], &vs[1], vs[2].x, vs[2].y);
    if area == 0 {
        return 0;
    }
    if area < 0 {
        vs.swap(1, 2);
    }

    let denom = area.abs();
    let tl = [
        top_left(&vs[1], &vs[2]),
        top_left(&vs[2], &vs[0]),
        top_left(&vs[0], &vs[1]),
    ];
    let shades = [vs[0].shade as i64, vs[1].shade as i64, vs[2].shade as i64];
    let zs = [vs[0].z as i64, vs[1].z as i64, vs[2].z as i64];
    let uvq = [
        [vs[0].uq as i64, vs[1].uq as i64, vs[2].uq as i64],
        [vs[0].vq as i64, vs[1].vq as i64, vs[2].vq as i64],
        [vs[0].q as i64, vs[1].q as i64, vs[2].q as i64],
    ];

    let x0 = (vs[0].x.min(vs[1].x).min(vs[2].x) >> 4).max(0);
    let x1 = (vs[0].x.max(vs[1].x).max(vs[2].x) >> 4).min(block.width as i32 - 1);
    let y0 = (vs[0].y.min(vs[1].y).min(vs[2].y) >> 4).max(0);
    let y1 = (vs[0].y.max(vs[1].y).max(vs[2].y) >> 4).min(block.height as i32 - 1);

    let blend = flags & TRI_BLEND != 0;
    let ztest = flags & TRI_ZTEST_OFF == 0;
    let zgreater = flags & TRI_ZGREATER != 0;
    let zwrite = flags & TRI_ZWRITE_OFF == 0;

    let mut cost = 0;
    for py in y0..=y1 {
        let cy = (py << 4) + 8;
        let Some((first, last)) = row_run(&vs, &tl, x0, x1, cy) else {
            continue;
        };
        let mut xa = first;

        while xa <= last {
            let xb = (xa + 8).min(last + 1);
            let span = (xb - xa) as i64;
            let (ua, va) = uv_at(&vs, &uvq, denom, xa, cy);
            let (ub, vb) = uv_at(&vs, &uvq, denom, xb, cy);
            let du = ub.wrapping_sub(ua);
            let dv = vb.wrapping_sub(va);
            let lod = lod_44(
                du.unsigned_abs().max(dv.unsigned_abs()) / span as u64,
                lod_bias,
            );
            let level = (lod >> 4).min(tex.levels as i64 - 1) as usize;

            for px in xa..xb {
                let e = edges(&vs, (px << 4) + 8, cy);

                if !covered(&e, &tl) {
                    continue;
                }

                cost += 1;

                let at = py as u64 * block.width as u64 + px as u64;
                let z = (interp(&e, &zs, denom) >> 16) as u16;
                let dst = read_u16(mem, block.z_base as u64 + at * 2);
                let pass = !ztest || if zgreater { z > dst } else { z < dst };

                if !pass {
                    continue;
                }

                let bayer = BAYER[(py & 3) as usize][(px & 3) as usize] as i64;
                let k = (px - xa) as i64;
                let sampled = (level + ((lod & 0xF) > bayer) as usize).min(tex.levels - 1);
                let u = ua.wrapping_add(du.wrapping_mul(k).div_euclid(span));
                let v = va.wrapping_add(dv.wrapping_mul(k).div_euclid(span));
                let tx = texel(mem, &tex, sampled, u, v) as u64;

                let shade = interp(&e, &shades, denom);
                let lit =
                    ((shade >> 16) + ((shade & 0xFFFF) > bayer << 12) as i64).clamp(0, 63) as u64;
                let mut color = read_u8(mem, colormap + lit * 256 + tx);

                if blend {
                    let under = read_u8(mem, block.color_base as u64 + at) as u64;
                    color = read_u8(mem, blend_base + under * 256 + color as u64);
                }

                write_u8(mem, block.color_base as u64 + at, color);
                if zwrite {
                    write_u16(mem, block.z_base as u64 + at * 2, z);
                }
            }

            xa = xb;
        }
    }

    cost
}

fn inside(e: i64, top_left: bool) -> bool {
    if top_left { e >= 0 } else { e > 0 }
}

fn interp(e: &[i64; 3], a: &[i64; 3], denom: i64) -> i64 {
    let n = e[0] as i128 * a[0] as i128 + e[1] as i128 * a[1] as i128 + e[2] as i128 * a[2] as i128;
    n.div_euclid(denom as i128) as i64
}

fn lod_44(step: u64, bias: i64) -> i64 {
    if step == 0 {
        return 0;
    }

    let k = 63 - step.leading_zeros() as i64;
    let frac = if k >= 4 {
        ((step >> (k - 4)) & 0xF) as i64
    } else {
        0
    };

    (((k - 16) << 4) + frac + bias).max(0)
}

fn read_state_block(mem: &[u8]) -> StateBlock {
    StateBlock {
        ring_base: read_u32(mem, 0),
        ring_words: read_u32(mem, 4),
        color_base: read_u32(mem, 8),
        z_base: read_u32(mem, 12),
        width: read_u16(mem, 16),
        height: read_u16(mem, 18),
    }
}

fn read_u16(mem: &[u8], offset: u64) -> u16 {
    match usize::try_from(TPU_RAM_BASE as u64 + offset) {
        Ok(a) if a + 2 <= mem.len() => u16::from_be_bytes([mem[a], mem[a + 1]]),
        _ => 0,
    }
}

fn read_u32(mem: &[u8], offset: u64) -> u32 {
    match usize::try_from(TPU_RAM_BASE as u64 + offset) {
        Ok(a) if a + 4 <= mem.len() => u32::from_be_bytes(mem[a..a + 4].try_into().unwrap()),
        _ => 0,
    }
}

fn read_u8(mem: &[u8], offset: u64) -> u8 {
    match usize::try_from(TPU_RAM_BASE as u64 + offset) {
        Ok(a) if a < mem.len() => mem[a],
        _ => 0,
    }
}

fn read_vertex(mem: &[u8], block: &StateBlock, head: u16, i: u32) -> Vertex {
    let w = 4 + i * 7;

    Vertex {
        q: ring_word(mem, block, head, w + 5),
        shade: ring_word(mem, block, head, w + 6) as i32,
        uq: ring_word(mem, block, head, w + 3) as i32,
        vq: ring_word(mem, block, head, w + 4) as i32,
        x: (ring_word(mem, block, head, w) as i32) >> 12,
        y: (ring_word(mem, block, head, w + 1) as i32) >> 12,
        z: ring_word(mem, block, head, w + 2),
    }
}

fn ring_word(mem: &[u8], block: &StateBlock, head: u16, i: u32) -> u32 {
    let idx = (head as u64 + i as u64) % block.ring_words as u64;
    read_u32(mem, block.ring_base as u64 + idx * 4)
}

fn row_run(vs: &[Vertex; 3], tl: &[bool; 3], x0: i32, x1: i32, cy: i32) -> Option<(i32, i32)> {
    let hit = |px: i32| covered(&edges(vs, (px << 4) + 8, cy), tl);
    let first = (x0..=x1).find(|&px| hit(px))?;

    Some((first, (first..=x1).rev().find(|&px| hit(px))?))
}

fn texel(mem: &[u8], tex: &Texture, level: usize, u: i64, v: i64) -> u8 {
    let n = level as u32;
    let mut off = tex.base;

    for i in 0..n {
        off += (1u64 << tex.log2w.saturating_sub(i)) * (1u64 << tex.log2h.saturating_sub(i));
    }

    let w = 1u64 << tex.log2w.saturating_sub(n);
    let h = 1u64 << tex.log2h.saturating_sub(n);

    read_u8(
        mem,
        off + ((v >> 16) as u64 & (h - 1)) * w + ((u >> 16) as u64 & (w - 1)),
    )
}

fn top_left(a: &Vertex, b: &Vertex) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}

fn uv_at(vs: &[Vertex; 3], uvq: &[[i64; 3]; 3], denom: i64, px: i32, cy: i32) -> (i64, i64) {
    let e = edges(vs, (px << 4) + 8, cy);
    let q = interp(&e, &uvq[2], denom);

    if q == 0 {
        return (0, 0);
    }

    (
        (interp(&e, &uvq[0], denom) << 16).wrapping_div_euclid(q),
        (interp(&e, &uvq[1], denom) << 16).wrapping_div_euclid(q),
    )
}

fn valid_ring(ring_words: u32) -> bool {
    ring_words != 0 && ring_words <= 0x1_0000 && ring_words.is_power_of_two()
}

fn write_u16(mem: &mut [u8], offset: u64, v: u16) {
    if let Ok(a) = usize::try_from(TPU_RAM_BASE as u64 + offset)
        && a + 2 <= mem.len()
    {
        mem[a..a + 2].copy_from_slice(&v.to_be_bytes());
    }
}

fn write_u8(mem: &mut [u8], offset: u64, v: u8) {
    if let Ok(a) = usize::try_from(TPU_RAM_BASE as u64 + offset)
        && a < mem.len()
    {
        mem[a] = v;
    }
}

pub struct Tpu {
    pub deficit: u32,
    pub head: u16,
    pub pixels: u32,
    pub tail: u16,
    budget: u32,
}

struct StateBlock {
    ring_base: u32,
    ring_words: u32,
    color_base: u32,
    z_base: u32,
    width: u16,
    height: u16,
}

struct Texture {
    base: u64,
    levels: usize,
    log2h: u32,
    log2w: u32,
}

struct Vertex {
    q: u32,
    shade: i32,
    uq: i32,
    vq: i32,
    x: i32,
    y: i32,
    z: u32,
}

impl Default for Tpu {
    fn default() -> Tpu {
        Tpu::new()
    }
}

impl Tpu {
    pub fn new() -> Tpu {
        Tpu {
            deficit: 0,
            head: 0,
            pixels: 0,
            tail: 0,
            budget: PIXEL_BUDGET,
        }
    }

    pub fn frame_start(&mut self, mem: &mut [u8]) {
        self.budget = PIXEL_BUDGET.saturating_sub(self.deficit);
        self.deficit = 0;
        self.pixels = 0;

        run(self, mem);
    }

    pub fn reset(&mut self) {
        *self = Tpu::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLEND: u32 = 0x1_0000;
    const CMAP: u32 = 0x4000;
    const COLOR: u32 = 0x1000;
    const RING: u32 = 0x100;
    const TEX: u32 = 0x3000;
    const Z: u32 = 0x2000;

    const TEX_DESC: u32 = (TEX >> 3) << 12 | 0x001;
    const TEX_MIP: u32 = (TEX >> 3) << 12 | 0x334;
    const TEX_WIDE: u32 = (TEX >> 3) << 12 | 0x501;

    const UVQ_1: [[i32; 3]; 3] = [[0, 0, 0x0001_0000]; 3];

    fn fill(flags: u8, x0: u16, y0: u16, x1: u16, y1: u16, color: u8, z: u16) -> [u32; 5] {
        [
            0x0200_0000 | flags as u32,
            (x0 as u32) << 16 | y0 as u32,
            (x1 as u32) << 16 | y1 as u32,
            color as u32,
            z as u32,
        ]
    }

    fn ring_with(ring_words: u32, cmds: &[u32]) -> Vec<u8> {
        let mut m = vec![0u8; TPU_RAM_BASE as usize + 0x2_0000];

        put32(&mut m, 0, RING);
        put32(&mut m, 4, ring_words);
        put32(&mut m, 8, COLOR);
        put32(&mut m, 12, Z);
        put16(&mut m, 16, 16);
        put16(&mut m, 18, 16);

        for (i, &w) in cmds.iter().enumerate() {
            put_ring_word(&mut m, ring_words, i as u32, w);
        }

        m
    }

    fn tri(flags: u8, vs: [(i32, i32, u16, i32); 3]) -> Vec<u32> {
        tri_uv(flags, 0, TEX_DESC, 0, vs, UVQ_1)
    }

    fn tri_uv(
        flags: u8,
        bias: i8,
        desc: u32,
        blend: u32,
        vs: [(i32, i32, u16, i32); 3],
        uvq: [[i32; 3]; 3],
    ) -> Vec<u32> {
        let w0 = 0x0100_0000 | (bias as u8 as u32) << 8 | flags as u32;
        let mut w = vec![w0, desc, CMAP, blend];

        for (i, (x, y, z, shade)) in vs.into_iter().enumerate() {
            w.extend([
                (x << 16) as u32,
                (y << 16) as u32,
                (z as u32) << 16,
                uvq[i][0] as u32,
                uvq[i][1] as u32,
                uvq[i][2] as u32,
                (shade << 16) as u32,
            ]);
        }

        w
    }

    fn tri_ram(ring_words: u32, cmds: &[u32]) -> Vec<u8> {
        let mut m = ring_with(ring_words, cmds);

        put8(&mut m, TEX, 1);
        for r in 0..64u32 {
            put8(&mut m, CMAP + r * 256 + 1, (r + 1) as u8);
        }

        m
    }

    fn put8(m: &mut [u8], off: u32, v: u8) {
        m[TPU_RAM_BASE as usize + off as usize] = v;
    }

    fn put16(m: &mut [u8], off: u32, v: u16) {
        let a = TPU_RAM_BASE as usize + off as usize;
        m[a..a + 2].copy_from_slice(&v.to_be_bytes());
    }

    fn put32(m: &mut [u8], off: u32, v: u32) {
        let a = TPU_RAM_BASE as usize + off as usize;
        m[a..a + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn put_ring_word(m: &mut [u8], ring_words: u32, raw_index: u32, w: u32) {
        let idx = raw_index % ring_words;
        put32(m, RING + idx * 4, w);
    }

    fn color_at(mem: &[u8], x: u32, y: u32) -> u8 {
        mem[TPU_RAM_BASE as usize + COLOR as usize + (y * 16 + x) as usize]
    }

    fn z_at(mem: &[u8], x: u32, y: u32) -> u16 {
        let a = TPU_RAM_BASE as usize + Z as usize + (y * 16 + x) as usize * 2;
        u16::from_be_bytes([mem[a], mem[a + 1]])
    }

    #[test]
    fn fill_writes_color_and_z_half_open() {
        let cmd = fill(FILL_COLOR | FILL_Z, 1, 1, 3, 3, 7, 0x00FF);
        let mut mem = ring_with(8, &cmd);
        let mut t = Tpu::new();
        t.tail = 5;

        run(&mut t, &mut mem);

        for y in 1..3 {
            for x in 1..3 {
                assert_eq!(color_at(&mem, x, y), 7, "color at ({x},{y})");
                assert_eq!(z_at(&mem, x, y), 0x00FF, "z at ({x},{y})");
            }
        }
        assert_eq!(
            color_at(&mem, 0, 0),
            0,
            "fill wrote left of its half-open rect"
        );
        assert_eq!(
            color_at(&mem, 3, 3),
            0,
            "fill wrote at its exclusive far corner"
        );
        assert_eq!(t.head, t.tail, "head must reach tail after a full drain");
    }

    #[test]
    fn fill_costs_quarter_pixels_rounded_up() {
        let cmd = fill(FILL_COLOR, 0, 0, 3, 3, 1, 0);
        let mut mem = ring_with(8, &cmd);
        let mut t = Tpu::new();
        t.tail = 5;

        run(&mut t, &mut mem);

        assert_eq!(t.pixels, 3, "ceil(9/4) must be 3");
    }

    #[test]
    fn ring_words_that_is_not_a_power_of_two_drains_nothing() {
        let cmd = fill(FILL_COLOR, 0, 0, 3, 3, 7, 0);
        let mut mem = ring_with(10, &cmd);
        let mut t = Tpu::new();
        t.tail = 5;

        run(&mut t, &mut mem);

        assert_eq!(
            t.head, t.tail,
            "an invalid ring must snap head straight to tail"
        );
        assert_eq!(
            color_at(&mem, 0, 0),
            0,
            "an invalid ring must not execute any command"
        );
    }

    #[test]
    fn unknown_opcode_advances_head_by_one_word_and_executes_nothing() {
        let mut mem = ring_with(8, &[0x00FF_FFFF]);
        let mut t = Tpu::new();
        t.tail = 1;

        run(&mut t, &mut mem);

        assert_eq!(
            t.head, 1,
            "unknown opcode must advance head by exactly one word"
        );
        assert_eq!(t.pixels, 0, "unknown opcode must cost nothing");
        assert_eq!(color_at(&mem, 0, 0), 0, "unknown opcode must not paint");
    }

    #[test]
    fn misaligned_tail_terminates_instead_of_spinning() {
        const RING_WORDS: u32 = 8;
        let mut mem = ring_with(RING_WORDS, &[]);
        put_ring_word(&mut mem, RING_WORDS, 6, 0x0200_0000);

        let mut t = Tpu::new();
        t.tail = 7;

        run(&mut t, &mut mem);

        assert_eq!(
            t.head, t.tail,
            "a misaligned/corrupt ring must still terminate with head == tail"
        );
    }

    #[test]
    fn fill_command_words_straddling_the_ring_wrap_execute_correctly() {
        const RING_WORDS: u32 = 8;
        let cmd = fill(FILL_COLOR, 0, 0, 2, 2, 6, 0);
        let mut mem = ring_with(RING_WORDS, &[]);

        for (j, &w) in cmd.iter().enumerate() {
            put_ring_word(&mut mem, RING_WORDS, 6 + j as u32, w);
        }

        let mut t = Tpu::new();
        t.head = 6;
        t.tail = 11;

        run(&mut t, &mut mem);

        assert_eq!(
            t.head, t.tail,
            "head must land on tail after a wrap-straddling command"
        );
        assert_eq!(
            color_at(&mem, 0, 0),
            6,
            "the wrapped command must have executed"
        );
        assert_eq!(
            t.pixels, 1,
            "cost must be charged for a command that straddles the wrap"
        );
    }

    #[test]
    fn drain_survives_multiple_ring_wraps_without_rerunning_stale_commands() {
        const RING_WORDS: u32 = 8;
        let mut mem = ring_with(RING_WORDS, &[]);
        let mut t = Tpu::new();

        for (i, &color) in [11u8, 22, 33, 44].iter().enumerate() {
            let cmd = fill(FILL_COLOR, 0, 0, 1, 1, color, 0);
            let base = i as u32 * 5;

            for (j, &w) in cmd.iter().enumerate() {
                put_ring_word(&mut mem, RING_WORDS, base + j as u32, w);
            }

            t.tail = t.tail.wrapping_add(5);
            run(&mut t, &mut mem);

            assert_eq!(
                t.head, t.tail,
                "tail write {i}: head must land exactly on tail"
            );
            assert_eq!(
                color_at(&mem, 0, 0),
                color,
                "tail write {i}: a stale command re-ran instead"
            );
        }
    }

    #[test]
    fn budget_lets_a_command_finish_then_stops_and_carries() {
        let cmd1 = fill(FILL_COLOR, 0, 0, 16, 16, 5, 0);
        let cmd2 = fill(FILL_COLOR, 0, 0, 2, 2, 9, 0);
        let cmds: Vec<u32> = cmd1.into_iter().chain(cmd2).collect();
        let mut mem = ring_with(16, &cmds);
        let mut t = Tpu::new();
        t.tail = 10;
        t.budget = 10;

        run(&mut t, &mut mem);

        assert_eq!(
            t.head, 5,
            "command 1 must complete and advance head past it"
        );
        assert_ne!(t.head, t.tail, "command 2 must not run in this drain");
        assert_eq!(t.deficit, 54, "overshoot (64 - 10) must carry as deficit");
        assert_eq!(
            color_at(&mem, 0, 0),
            5,
            "command 1 must have written despite overshooting"
        );

        t.frame_start(&mut mem);

        assert_eq!(
            t.head, t.tail,
            "frame_start must re-drain the deferred command"
        );
        assert_eq!(
            color_at(&mem, 0, 0),
            9,
            "deferred command 2 must have run after frame_start"
        );
        assert_eq!(t.deficit, 0, "deficit clears once granted to the new frame");
    }

    #[test]
    fn right_triangle_covers_the_textbook_six_pixels() {
        let cmd = tri(TRI_ZTEST_OFF, [(0, 0, 0, 0), (4, 0, 0, 0), (0, 4, 0, 0)]);
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        let want = [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (0, 2)];
        for y in 0..16 {
            for x in 0..16 {
                let expect = u8::from(want.contains(&(x, y)));
                assert_eq!(color_at(&mem, x, y), expect, "coverage at ({x},{y})");
            }
        }
        assert_eq!(t.pixels, 6, "exactly six fragments may be charged");
    }

    #[test]
    fn reversed_winding_covers_the_same_six_pixels() {
        let cmd = tri(TRI_ZTEST_OFF, [(0, 0, 0, 0), (0, 4, 0, 0), (4, 0, 0, 0)]);
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        let want = [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (0, 2)];
        for y in 0..16 {
            for x in 0..16 {
                let expect = u8::from(want.contains(&(x, y)));
                assert_eq!(color_at(&mem, x, y), expect, "coverage at ({x},{y})");
            }
        }
        assert_eq!(
            t.pixels, 6,
            "winding normalisation must not change the covered set"
        );
    }

    #[test]
    fn degenerate_triangle_is_skipped_at_zero_cost() {
        let cmd = tri(TRI_ZTEST_OFF, [(0, 0, 0, 0), (4, 0, 0, 0), (8, 0, 0, 0)]);
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        assert_eq!(t.pixels, 0, "a zero-area triangle has no fragments");
        assert_eq!(
            color_at(&mem, 0, 0),
            0,
            "a zero-area triangle must paint nothing"
        );
        assert_eq!(
            t.head, t.tail,
            "a skipped triangle still advances head past its 25 words"
        );
    }

    #[test]
    fn shared_edge_paints_every_pixel_exactly_once() {
        let mut cmds = tri(TRI_ZTEST_OFF, [(0, 0, 0, 0), (4, 0, 0, 0), (4, 4, 0, 0)]);
        cmds.extend(tri(
            TRI_ZTEST_OFF,
            [(0, 0, 0, 1), (4, 4, 0, 1), (0, 4, 0, 1)],
        ));
        let mut mem = tri_ram(64, &cmds);
        let mut t = Tpu::new();
        t.tail = 50;

        run(&mut t, &mut mem);

        for y in 0..4 {
            for x in 0..4 {
                let expect = if x >= y { 1 } else { 2 };
                assert_eq!(color_at(&mem, x, y), expect, "quad pixel ({x},{y})");
            }
        }
        assert_eq!(
            t.pixels, 16,
            "a gap or a double-paint would move this off 16"
        );
        assert_eq!(
            color_at(&mem, 4, 4),
            0,
            "the quad must not paint outside 4x4"
        );
    }

    #[test]
    fn fragments_outside_the_scissor_are_not_fragments() {
        let cmd = tri(TRI_ZTEST_OFF, [(14, 0, 0, 0), (18, 0, 0, 0), (14, 4, 0, 0)]);
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        assert_eq!(
            t.pixels, 5,
            "only the five in-scissor fragments may be charged"
        );
        assert_eq!(
            color_at(&mem, 14, 0),
            1,
            "the in-scissor part must still paint"
        );
        assert_eq!(
            color_at(&mem, 0, 1),
            0,
            "an out-of-scissor fragment wrapped onto row 1"
        );
    }

    #[test]
    fn z_less_rejects_and_zgreater_flips() {
        let verts = |z, shade| [(0, 0, z, shade), (4, 0, z, shade), (0, 4, z, shade)];
        let mut cmds = fill(FILL_Z, 0, 0, 16, 16, 0, 0x0100).to_vec();
        cmds.extend(tri(0, verts(0x0200, 4)));
        cmds.extend(tri(TRI_ZGREATER, verts(0x0200, 9)));
        cmds.extend(tri(TRI_ZGREATER, verts(0x0200, 20)));
        cmds.extend(tri(TRI_ZGREATER | TRI_ZWRITE_OFF, verts(0x0300, 30)));
        let mut mem = tri_ram(128, &cmds);
        let mut t = Tpu::new();

        t.tail = 30;
        run(&mut t, &mut mem);
        assert_eq!(
            color_at(&mem, 0, 0),
            0,
            "0x0200 < 0x0100 is false: pass 1 must reject"
        );
        assert_eq!(
            z_at(&mem, 0, 0),
            0x0100,
            "a rejected fragment must not write z"
        );
        assert_eq!(t.pixels, 70, "64 for the FILL plus 6 z-failed fragments");

        t.tail = 55;
        run(&mut t, &mut mem);
        assert_eq!(
            color_at(&mem, 0, 0),
            10,
            "ZGREATER must accept 0x0200 > 0x0100"
        );
        assert_eq!(
            z_at(&mem, 0, 0),
            0x0200,
            "an accepted fragment writes its z"
        );
        assert_eq!(t.pixels, 76, "six more fragments");

        t.tail = 80;
        run(&mut t, &mut mem);
        assert_eq!(
            color_at(&mem, 0, 0),
            10,
            "a tie must fail under ZGREATER too"
        );
        assert_eq!(t.pixels, 82, "a z-failed fragment still costs one pixel");

        t.tail = 105;
        run(&mut t, &mut mem);
        assert_eq!(color_at(&mem, 0, 0), 31, "pass 4 passes the depth test");
        assert_eq!(
            z_at(&mem, 0, 0),
            0x0200,
            "ZWRITE_OFF must leave the z-buffer alone"
        );
    }

    #[test]
    fn shades_interpolate_and_dither_between_colormap_rows() {
        const WANT: [(u32, u32, u8); 6] = [
            (0, 0, 13),
            (1, 0, 21),
            (2, 0, 29),
            (0, 1, 28),
            (1, 1, 37),
            (0, 2, 45),
        ];

        let cmd = tri(TRI_ZTEST_OFF, [(0, 0, 0, 0), (4, 0, 0, 32), (0, 4, 0, 63)]);
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        for (x, y, color) in WANT {
            assert_eq!(color_at(&mem, x, y), color, "dithered shade at ({x},{y})");
        }
    }

    #[test]
    fn z_interpolates_across_the_vertex_gradient() {
        const WANT: [(u32, u32, u16); 6] = [
            (0, 0, 1536),
            (1, 0, 2560),
            (2, 0, 3584),
            (0, 1, 3584),
            (1, 1, 4608),
            (0, 2, 5632),
        ];

        let cmd = tri(
            TRI_ZTEST_OFF,
            [(0, 0, 0, 0), (4, 0, 4096, 0), (0, 4, 8192, 0)],
        );
        let mut mem = tri_ram(32, &cmd);
        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        for (x, y, z) in WANT {
            assert_eq!(z_at(&mem, x, y), z, "interpolated z at ({x},{y})");
        }
    }

    #[test]
    fn perspective_divide_is_exact_at_subspan_boundaries() {
        const UQ: i32 = 16 << 16;
        const WANT: [u8; 16] = [16, 15, 14, 13, 13, 12, 11, 11, 10, 10, 9, 9, 9, 8, 8, 8];

        let cmd = tri_uv(
            TRI_ZTEST_OFF,
            0,
            TEX_WIDE,
            0,
            [(0, 0, 0, 0), (20, 0, 0, 0), (0, 20, 0, 0)],
            [[UQ, 0, 63488], [UQ, 0, 145408], [UQ, 0, 63488]],
        );
        let mut mem = ring_with(32, &cmd);

        for i in 0..32u32 {
            put8(&mut mem, TEX + i, i as u8);
            put8(&mut mem, CMAP + i, i as u8);
        }

        let mut t = Tpu::new();
        t.tail = 25;

        run(&mut t, &mut mem);

        for (x, want) in WANT.into_iter().enumerate() {
            assert_eq!(
                color_at(&mem, x as u32, 0),
                want,
                "perspective texel at x={x}"
            );
        }
    }

    #[test]
    fn mip_level_follows_uv_step_and_lod_bias() {
        const VS: [(i32, i32, u16, i32); 3] = [(0, 0, 0, 0), (20, 0, 0, 0), (0, 20, 0, 0)];
        const MIPS: [(u32, u32, u8); 4] = [(0, 64, 1), (64, 16, 2), (80, 4, 3), (84, 1, 4)];

        let uvq = |uq: i32| {
            [
                [0, 0, 0x0001_0000],
                [uq, 0, 0x0001_0000],
                [0, 0, 0x0001_0000],
            ]
        };
        let mut cmds = tri_uv(TRI_ZTEST_OFF, 0, TEX_MIP, 0, VS, uvq(80 << 16));
        cmds.extend(tri_uv(TRI_ZTEST_OFF, 0x10, TEX_MIP, 0, VS, uvq(80 << 16)));
        cmds.extend(tri_uv(TRI_ZTEST_OFF, 0, TEX_MIP, 0, VS, uvq(120 << 16)));
        let mut mem = ring_with(128, &cmds);

        for (off, len, tag) in MIPS {
            for i in 0..len {
                put8(&mut mem, TEX + off + i, tag);
            }
        }
        for i in 0..8u32 {
            put8(&mut mem, CMAP + i, i as u8);
        }

        let mut t = Tpu::new();

        t.tail = 25;
        run(&mut t, &mut mem);
        for x in 0..16 {
            assert_eq!(
                color_at(&mem, x, 0),
                3,
                "4:1 minification must fetch level 2 at x={x}"
            );
        }

        t.tail = 50;
        run(&mut t, &mut mem);
        for x in 0..16 {
            assert_eq!(
                color_at(&mem, x, 0),
                4,
                "a +1.0 bias must fetch level 3 at x={x}"
            );
        }

        t.tail = 75;
        run(&mut t, &mut mem);
        for x in 0..16 {
            let want = if x % 2 == 0 { 4 } else { 3 };
            assert_eq!(color_at(&mem, x, 0), want, "dithered lod fraction at x={x}");
        }
    }

    #[test]
    fn blend_flag_routes_through_the_table() {
        let mut cmds = tri(
            TRI_ZTEST_OFF,
            [(0, 0, 0x0100, 9), (4, 0, 0x0100, 9), (0, 4, 0x0100, 9)],
        );
        cmds.extend(tri_uv(
            TRI_ZTEST_OFF | TRI_BLEND | TRI_ZWRITE_OFF,
            0,
            TEX_DESC,
            BLEND,
            [(0, 0, 0x0300, 19), (3, 0, 0x0300, 19), (0, 3, 0x0300, 19)],
            UVQ_1,
        ));
        cmds.extend(tri(
            TRI_ZTEST_OFF,
            [(8, 0, 0, 63), (12, 0, 0, 63), (8, 4, 0, 63)],
        ));
        cmds.extend(tri_uv(
            TRI_ZTEST_OFF | TRI_BLEND,
            0,
            TEX_DESC,
            BLEND,
            [(8, 0, 0, 19), (11, 0, 0, 19), (8, 3, 0, 19)],
            UVQ_1,
        ));
        let mut mem = tri_ram(128, &cmds);

        put8(&mut mem, CMAP + 63 * 256 + 1, 250);
        for d in 0..256u32 {
            for s in 0..256u32 {
                put8(&mut mem, BLEND + d * 256 + s, (d + 2 * s).min(255) as u8);
            }
        }

        let mut t = Tpu::new();
        t.tail = 100;

        run(&mut t, &mut mem);

        assert_eq!(
            color_at(&mem, 0, 0),
            50,
            "BLEND must route the colormap output through the table"
        );
        assert_eq!(
            color_at(&mem, 1, 0),
            50,
            "every pixel of the blending triangle goes through it"
        );
        assert_eq!(
            color_at(&mem, 2, 0),
            10,
            "a pixel the blending triangle misses keeps its colour"
        );
        assert_eq!(
            z_at(&mem, 0, 0),
            0x0100,
            "ZWRITE_OFF must still hold under BLEND"
        );
        assert_eq!(
            color_at(&mem, 8, 0),
            255,
            "the table's saturation must show through"
        );
        assert_eq!(
            color_at(&mem, 10, 0),
            250,
            "only the blending triangle's pixels change"
        );
    }

    #[test]
    fn row_clamps_to_the_colormap_range() {
        let mut cmds = tri(
            TRI_ZTEST_OFF,
            [(0, 0, 0, 100), (4, 0, 0, 100), (0, 4, 0, 100)],
        );
        cmds.extend(tri(
            TRI_ZTEST_OFF,
            [(8, 0, 0, -5), (12, 0, 0, -5), (8, 4, 0, -5)],
        ));
        let mut mem = tri_ram(64, &cmds);
        let mut t = Tpu::new();
        t.tail = 50;

        run(&mut t, &mut mem);

        assert_eq!(
            color_at(&mem, 0, 0),
            64,
            "a row above 63 must clamp to row 63"
        );
        assert_eq!(
            color_at(&mem, 8, 0),
            1,
            "a negative row must clamp to row 0"
        );
    }
}

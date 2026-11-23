use crate::bus::{HI_TILES_BASE, PALETTE_BASE, VRAM_BASE};

pub const SPRITE_COUNT: usize = 128;
pub const SPRITE_STRIDE: usize = 8;
pub const TILEMAP_COLS: usize = 128;
pub const TILEMAP_ROWS: usize = 128;
pub const TILEMAP_PLANES: usize = 4;
pub const TILEMAP_STRIDE: usize = TILEMAP_COLS * TILEMAP_ROWS * 2;

const TILEMAPS: usize = VRAM_BASE as usize + 0x4_0000;
const SPRITES: usize = VRAM_BASE as usize + 0x6_0000;
const SCROLL: usize = VRAM_BASE as usize + 0x6_1000;
const SCROLL_COL_V: usize = SCROLL + 0x5B0;

pub struct Mode {
    pub h: usize,
    line_h: usize,
    pub lines: u32,
    pub samples: usize,
    shift: usize,
    tiles: usize,
    pub w: usize,
}

pub const LO: Mode = Mode {
    h: 180,
    line_h: SCROLL + 0x10,
    lines: 200,
    samples: 4,
    shift: 3,
    tiles: VRAM_BASE as usize,
    w: 320,
};

pub const HI: Mode = Mode {
    h: 360,
    line_h: VRAM_BASE as usize + 0x6_2000,
    lines: 400,
    samples: 2,
    shift: 4,
    tiles: HI_TILES_BASE as usize,
    w: 640,
};

pub static MODES: [Mode; 2] = [LO, HI];

pub fn render(mem: &[u8], mode: &Mode, brightness: u8, out: &mut [u32]) {
    assert!(out.len() >= mode.w * mode.h);

    for y in 0..mode.h {
        render_line(mem, mode, brightness, y, out);
    }
}

pub fn render_line(mem: &[u8], mode: &Mode, brightness: u8, y: usize, out: &mut [u32]) {
    let row = &mut out[y * mode.w..][..mode.w];

    row.fill(palette(mem, 0));

    for n in 0..TILEMAP_PLANES {
        paint_plane_line(mem, mode, n, false, y, row);
    }

    paint_sprites_line(mem, mode, false, y, row);

    for n in 0..TILEMAP_PLANES - 1 {
        paint_plane_line(mem, mode, n, true, y, row);
    }

    paint_sprites_line(mem, mode, true, y, row);
    paint_plane_line(mem, mode, TILEMAP_PLANES - 1, true, y, row);

    if brightness != 255 {
        for px in row {
            *px = scale(*px, brightness);
        }
    }
}

fn be16(mem: &[u8], a: usize) -> u16 {
    u16::from_be_bytes([mem[a], mem[a + 1]])
}

fn paint_plane_line(mem: &[u8], mode: &Mode, n: usize, hi: bool, y: usize, row: &mut [u32]) {
    let map = TILEMAPS + n * TILEMAP_STRIDE;
    let plane_h = be16(mem, SCROLL + n * 4);
    let plane_v = be16(mem, SCROLL + n * 4 + 2);
    let h = plane_h.wrapping_add(be16(mem, mode.line_h + n * mode.h * 2 + y * 2)) as usize;
    let t = 1 << mode.shift;
    let wrap = (TILEMAP_COLS << mode.shift) - 1;

    for (x, px) in row.iter_mut().enumerate() {
        let v =
            plane_v.wrapping_add(be16(mem, SCROLL_COL_V + n * 80 + (x >> mode.shift) * 2)) as usize;
        let (sx, sy) = ((x + h) & wrap, (y + v) & wrap);
        let entry = be16(
            mem,
            map + ((sy >> mode.shift) * TILEMAP_COLS + (sx >> mode.shift)) * 2,
        );

        if (entry & 0x4000 != 0) != hi {
            continue;
        }

        let (mut tx, mut ty) = (sx & (t - 1), sy & (t - 1));

        if entry & 0x1000 != 0 {
            tx = t - 1 - tx;
        }

        if entry & 0x2000 != 0 {
            ty = t - 1 - ty;
        }

        let index = tile_pixel(mem, mode, (entry & 0x0FFF) as usize, tx, ty);

        if index != 0 {
            *px = palette(mem, index);
        }
    }
}

fn paint_sprites_line(mem: &[u8], mode: &Mode, hi: bool, y: usize, row: &mut [u32]) {
    let t = 1 << mode.shift;

    for s in (0..SPRITE_COUNT).rev() {
        let e = SPRITES + s * SPRITE_STRIDE;
        let ctrl = be16(mem, e + 4);

        if ctrl & 0x8000 == 0 || (ctrl & 0x4000 != 0) != hi {
            continue;
        }

        let x0 = i16::from_be_bytes([mem[e], mem[e + 1]]) as i32;
        let y0 = i16::from_be_bytes([mem[e + 2], mem[e + 3]]) as i32;
        let attr = be16(mem, e + 6);
        let (w, h) = (
            ((attr & 7) as i32 + 1) << mode.shift,
            (((attr >> 3) & 7) as i32 + 1) << mode.shift,
        );
        let sy = y as i32 - y0;

        if !(0..h).contains(&sy) {
            continue;
        }

        let tile = (ctrl & 0x0FFF) as usize;
        let offset = (attr >> 8) as usize;

        for sx in 0..w {
            let px = x0 + sx;

            if !(0..mode.w as i32).contains(&px) {
                continue;
            }

            let fx = if ctrl & 0x1000 != 0 { w - 1 - sx } else { sx } as usize;
            let fy = if ctrl & 0x2000 != 0 { h - 1 - sy } else { sy } as usize;
            let cell = (fy >> mode.shift) * (w as usize >> mode.shift) + (fx >> mode.shift);
            let index = tile_pixel(mem, mode, tile + cell, fx & (t - 1), fy & (t - 1));

            if index != 0 {
                row[px as usize] = palette(mem, (index + offset) & 0xFF);
            }
        }
    }
}

fn palette(mem: &[u8], i: usize) -> u32 {
    let a = PALETTE_BASE as usize + i * 4;

    u32::from_be_bytes([mem[a], mem[a + 1], mem[a + 2], mem[a + 3]]) & 0x00FF_FFFF
}

fn scale(rgb: u32, brightness: u8) -> u32 {
    let f = brightness as u32 + 1;
    let r = (((rgb >> 16) & 0xFF) * f) >> 8;
    let g = (((rgb >> 8) & 0xFF) * f) >> 8;
    let b = ((rgb & 0xFF) * f) >> 8;

    (r << 16) | (g << 8) | b
}

fn tile_pixel(mem: &[u8], mode: &Mode, tile: usize, tx: usize, ty: usize) -> usize {
    let t = 1 << mode.shift;

    mem[mode.tiles + ((tile & 0xFFF) << (2 * mode.shift)) + ty * t + tx] as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MEM_END;

    const TILEMAP_0: usize = TILEMAPS;
    const WHITE: u32 = 0x00FF_FFFF;

    fn checker_tile(mem: &mut [u8]) {
        for ty in 0..8 {
            for tx in 0..4 {
                mem[VRAM_BASE as usize + 64 + ty * 8 + tx] = 1;
            }
        }
    }

    fn frame(mem: &[u8]) -> Vec<u32> {
        let mut out = vec![0u32; LO.w * LO.h];
        render(mem, &LO, 255, &mut out);

        out
    }

    fn set_entry(mem: &mut [u8], col: usize, row: usize, entry: u16) {
        mem[TILEMAP_0 + (row * TILEMAP_COLS + col) * 2..][..2]
            .copy_from_slice(&entry.to_be_bytes());
    }

    fn set_palette(mem: &mut [u8], i: usize, rgb: u32) {
        mem[PALETTE_BASE as usize + i * 4..][..4].copy_from_slice(&rgb.to_be_bytes());
    }

    fn set_scroll(mem: &mut [u8], n: usize, h: u16, v: u16) {
        mem[SCROLL + n * 4..][..2].copy_from_slice(&h.to_be_bytes());
        mem[SCROLL + n * 4 + 2..][..2].copy_from_slice(&v.to_be_bytes());
    }

    fn set_sprite(mem: &mut [u8], i: usize, x: i16, y: i16, ctrl: u16, attr: u16) {
        let e = SPRITES + i * 8;

        mem[e..e + 2].copy_from_slice(&x.to_be_bytes());
        mem[e + 2..e + 4].copy_from_slice(&y.to_be_bytes());
        mem[e + 4..e + 6].copy_from_slice(&ctrl.to_be_bytes());
        mem[e + 6..e + 8].copy_from_slice(&attr.to_be_bytes());
    }

    #[test]
    fn backdrop_fills_frame() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0012_3456);
        let f = frame(&m);

        assert!(f.iter().all(|&p| p == 0x0012_3456));
    }

    #[test]
    fn tile_renders_at_cell() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0010_2040);
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 1, 1, 1);
        let f = frame(&m);

        assert_eq!(f[8 * LO.w + 8], 0x00FF_FFFF);
        assert_eq!(f[8 * LO.w + 12], 0x0010_2040);
        assert_eq!(f[0], 0x0010_2040);
    }

    #[test]
    fn flips_mirror_tile() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 1, 0, 0x1001);
        set_entry(&mut m, 0, 1, 0x2001);
        set_entry(&mut m, 1, 1, 0x3001);
        let f = frame(&m);
        let white = 0x00FF_FFFF;

        assert_eq!(f[0], white);
        assert_ne!(f[7], white);
        assert_ne!(f[8], white);
        assert_eq!(f[15], white);
        assert_eq!(f[8 * LO.w], white);
        assert_eq!(f[8 * LO.w + 15], white);
    }

    #[test]
    fn brightness_scales_output() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0080_4020);
        let mut out = vec![0u32; LO.w * LO.h];

        render(&m, &LO, 127, &mut out);
        assert_eq!(out[0], 0x0040_2010);

        render(&m, &LO, 0, &mut out);
        assert_eq!(out[0], 0);
    }

    #[test]
    fn plane_scroll_shifts_and_wraps() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 127, 0, 1);

        set_scroll(&mut m, 0, 4, 0);
        assert_ne!(frame(&m)[0], 0x00FF_FFFF);

        set_scroll(&mut m, 0, 1016, 0);
        assert_eq!(frame(&m)[0], 0x00FF_FFFF);
    }

    #[test]
    fn line_and_column_scroll_tables_add() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 1, 0, 1);

        m[LO.line_h..][..2].copy_from_slice(&4u16.to_be_bytes());
        let f = frame(&m);
        assert_ne!(f[0], 0x00FF_FFFF);
        assert_eq!(f[LO.w], 0x00FF_FFFF);

        m[LO.line_h..][..2].copy_from_slice(&0u16.to_be_bytes());
        m[SCROLL_COL_V..][..2].copy_from_slice(&8u16.to_be_bytes());
        let f = frame(&m);
        assert_ne!(f[0], 0x00FF_FFFF);
        assert_eq!(f[8], 0x00FF_FFFF);
    }

    #[test]
    fn plane_order_by_depth() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x0011_1111);
        set_palette(&mut m, 2, 0x0022_2222);
        checker_tile(&mut m);
        m[VRAM_BASE as usize + 128..VRAM_BASE as usize + 192].fill(2);
        set_entry(&mut m, 0, 0, 1);
        m[TILEMAPS + 0x8000..][..2].copy_from_slice(&2u16.to_be_bytes());
        let f = frame(&m);

        assert_eq!(f[0], 0x0022_2222);
    }

    #[test]
    fn sprite_renders_clips_and_offsets_palette() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0010_2040);
        set_palette(&mut m, 1, 0x00FF_FFFF);
        set_palette(&mut m, 17, 0x00FF_0000);
        checker_tile(&mut m);
        set_sprite(&mut m, 0, 10, 20, 0x8001, 0);
        set_sprite(&mut m, 1, -2, 40, 0x8001, 0);
        set_sprite(&mut m, 2, 30, 60, 0x8001, 16 << 8);
        let f = frame(&m);

        assert_eq!(f[20 * LO.w + 10], 0x00FF_FFFF);
        assert_eq!(f[20 * LO.w + 16], 0x0010_2040);
        assert_eq!(f[40 * LO.w], 0x00FF_FFFF);
        assert_eq!(f[60 * LO.w + 30], 0x00FF_0000);
    }

    #[test]
    fn sprite_priority_and_ui_slot() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x0011_1111);
        set_palette(&mut m, 2, 0x0022_2222);
        set_palette(&mut m, 3, 0x0033_3333);
        checker_tile(&mut m);
        m[VRAM_BASE as usize + 128..VRAM_BASE as usize + 192].fill(2);
        m[VRAM_BASE as usize + 192..VRAM_BASE as usize + 256].fill(3);
        set_entry(&mut m, 0, 0, 0x4002);
        set_sprite(&mut m, 0, 0, 0, 0x8001, 0);
        set_sprite(&mut m, 1, 4, 0, 0xC001, 0);
        let map3 = TILEMAPS + 3 * 0x8000;
        m[map3 + 2..map3 + 4].copy_from_slice(&0x4003u16.to_be_bytes());
        let f = frame(&m);

        assert_eq!(f[0], 0x0022_2222);
        assert_eq!(f[4], 0x0011_1111);
        assert_eq!(f[8], 0x0033_3333);
    }

    #[test]
    fn lower_sprite_index_wins() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x0011_1111);
        set_palette(&mut m, 17, 0x0022_2222);
        checker_tile(&mut m);
        set_sprite(&mut m, 0, 0, 0, 0x8001, 0);
        set_sprite(&mut m, 1, 0, 0, 0x8001, 16 << 8);
        let f = frame(&m);

        assert_eq!(f[0], 0x0011_1111);
    }

    fn hi_tile(mem: &mut [u8], tile: usize, color: u8, px: impl Fn(usize, usize) -> bool) {
        for ty in 0..16 {
            for tx in 0..16 {
                if px(tx, ty) {
                    mem[HI_TILES_BASE as usize + tile * 256 + ty * 16 + tx] = color;
                }
            }
        }
    }

    fn frame_hi(mem: &[u8]) -> Vec<u32> {
        let mut out = vec![0u32; HI.w * HI.h];
        render(mem, &HI, 255, &mut out);

        out
    }

    #[test]
    fn hi_res_tiles_come_from_the_hi_bank_at_16_px() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, WHITE);
        hi_tile(&mut m, 1, 1, |tx, _| tx < 8);
        m[VRAM_BASE as usize + 64..][..64].fill(1);
        set_entry(&mut m, 1, 1, 1);
        let f = frame_hi(&m);

        assert_eq!(f[16 * HI.w + 16], WHITE);
        assert_eq!(f[31 * HI.w + 23], WHITE);
        assert_ne!(
            f[16 * HI.w + 24],
            WHITE,
            "right half of the 16 px tile drew"
        );
        assert_ne!(
            f[16 * HI.w + 15],
            WHITE,
            "cell (1,1) bled into the cell left of it"
        );
    }

    #[test]
    fn hi_res_flips_mirror_a_16_px_tile() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, WHITE);
        hi_tile(&mut m, 1, 1, |tx, _| tx < 8);
        hi_tile(&mut m, 2, 1, |_, ty| ty < 8);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 1, 0, 0x1001);
        set_entry(&mut m, 0, 1, 2);
        set_entry(&mut m, 1, 1, 0x2002);
        let f = frame_hi(&m);

        assert_eq!(f[0], WHITE);
        assert_eq!(f[31], WHITE);
        assert_ne!(f[16], WHITE);
        assert_eq!(f[16 * HI.w], WHITE);
        assert_eq!(f[31 * HI.w + 16], WHITE);
        assert_ne!(f[16 * HI.w + 16], WHITE);
    }

    #[test]
    fn hi_res_wraps_tilemaps_at_2048_px() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, WHITE);
        hi_tile(&mut m, 1, 1, |tx, _| tx < 8);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 127, 0, 1);
        set_entry(&mut m, 0, 127, 1);

        set_scroll(&mut m, 0, 8, 0);
        assert_ne!(frame_hi(&m)[0], WHITE);

        set_scroll(&mut m, 0, 2032, 0);
        assert_eq!(frame_hi(&m)[0], WHITE);
        assert_eq!(
            frame_hi(&m)[16],
            WHITE,
            "x past 2048 did not wrap to column 0"
        );

        set_scroll(&mut m, 0, 0, 2032);
        assert_eq!(frame_hi(&m)[0], WHITE);
        assert_eq!(
            frame_hi(&m)[16 * HI.w],
            WHITE,
            "y past 2048 did not wrap to row 0"
        );
    }

    #[test]
    fn hi_res_reads_its_own_line_table_and_16_px_columns() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, WHITE);
        hi_tile(&mut m, 1, 1, |tx, _| tx < 8);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 1, 0, 1);
        m[TILEMAPS + TILEMAP_STRIDE..][..2].copy_from_slice(&1u16.to_be_bytes());

        m[HI.line_h..][..2].copy_from_slice(&8u16.to_be_bytes());
        m[HI.line_h + 720..][..2].copy_from_slice(&8u16.to_be_bytes());
        m[LO.line_h + 2..][..2].copy_from_slice(&8u16.to_be_bytes());
        let f = frame_hi(&m);
        assert_ne!(f[0], WHITE);
        assert_eq!(f[HI.w], WHITE, "hi-res read the lo-res line table");

        m[HI.line_h..][..2].fill(0);
        m[HI.line_h + 720..][..2].fill(0);
        m[TILEMAPS + TILEMAP_STRIDE..][..2].fill(0);
        m[LO.line_h + 2..][..2].fill(0);
        m[SCROLL_COL_V..][..2].copy_from_slice(&16u16.to_be_bytes());
        let f = frame_hi(&m);
        assert_ne!(f[0], WHITE);
        assert_eq!(f[16], WHITE, "column 1 (16 px wide) took column 0's scroll");

        m[SCROLL_COL_V..][..2].fill(0);
        m[SCROLL_COL_V + 2..][..2].copy_from_slice(&16u16.to_be_bytes());
        let f = frame_hi(&m);
        assert_ne!(f[16], WHITE, "a column is 16 px wide, not 8");
    }

    #[test]
    fn hi_res_crops_the_bottom_tile_row_at_line_359() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, WHITE);
        hi_tile(&mut m, 1, 1, |_, _| true);
        set_entry(&mut m, 0, 22, 1);
        let f = frame_hi(&m);

        assert_eq!(f[359 * HI.w], WHITE);
        assert_ne!(f[351 * HI.w], WHITE);
    }

    #[test]
    fn hi_res_sprites_size_in_16_px_tiles_and_clip_at_the_edges() {
        let mut m = vec![0u8; MEM_END as usize];
        let red = 0x00FF_0000;
        set_palette(&mut m, 1, WHITE);
        set_palette(&mut m, 17, red);

        for tile in 1..=64 {
            hi_tile(&mut m, tile, if tile == 2 { 17 } else { 1 }, |_, _| true);
        }

        set_sprite(&mut m, 0, 620, 340, 0x8001, (1 << 3) | 1);
        set_sprite(&mut m, 1, -8, -8, 0x8001, 0);
        set_sprite(&mut m, 2, 100, 100, 0x8001, 16 << 8);
        set_sprite(&mut m, 3, 200, 0, 0x8001, 0x3F);
        set_sprite(&mut m, 4, 400, 200, 0x9001, (1 << 3) | 1);
        let f = frame_hi(&m);

        assert_eq!(f[359 * HI.w + 639], WHITE);
        assert_eq!(f[7 * HI.w + 7], WHITE);
        assert_ne!(
            f[8 * HI.w + 8],
            WHITE,
            "off-screen half of sprite 1 shifted on screen"
        );
        assert_eq!(f[100 * HI.w + 100], red);
        assert_eq!(f[127 * HI.w + 327], WHITE);
        assert_ne!(f[128 * HI.w + 200], WHITE, "sprite 3 taller than 8 tiles");
        assert_eq!(f[216], red, "sprite cells step 16 px");
        assert_eq!(
            f[200 * HI.w + 400],
            red,
            "H flip did not swap a 2x2 sprite's cells"
        );
        assert_eq!(
            f[200 * HI.w + 416],
            WHITE,
            "H flip did not swap a 2x2 sprite's cells"
        );
    }
}

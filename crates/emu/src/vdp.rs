//! Layout:
//!   tiles      VRAM_BASE + 0x0       4096 x 64 B, 8bpp row-major indices
//!   tilemap n  VRAM_BASE + 0x4_0000 + n*0x8000   128x128 x u16 BE
//!     entry: [11:0] tile index, [12] H flip, [13] V flip, [14] priority
//!   sprites    VRAM_BASE + 0x6_0000  128 x 8 B
//!     x i16, y i16, ctrl ([15] enable, [14] priority, [13] V flip,
//!     [12] H flip, [11:0] tile), attr ([15:8] palette offset,
//!     [5:3] height-1, [2:0] width-1, in tiles)
//!   palette    PALETTE_BASE          256 x u32 BE 0x00RRGGBB
//!
//! Color index 0 = transparent -> backdrop = palette[0]. Compositing order:
//! backdrop, planes 0-3 lo, sprites lo, planes 0-2 hi, sprites hi,
//! plane 3 hi (the UI slot). Lower sprite index wins overlaps.

use crate::bus::{PALETTE_BASE, VRAM_BASE};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 180;

pub const TILEMAP_COLS: usize = 128;
pub const TILEMAP_ROWS: usize = 128;
pub const TILEMAP_PLANES: usize = 4;
pub const SPRITE_COUNT: usize = 128;
pub const SPRITE_STRIDE: usize = 8;
pub const TILEMAP_STRIDE: usize = TILEMAP_COLS * TILEMAP_ROWS * 2;

const TILEMAPS: usize = VRAM_BASE as usize + 0x4_0000;
const SPRITES: usize = VRAM_BASE as usize + 0x6_0000;
const SCROLL: usize = VRAM_BASE as usize + 0x6_1000;
const SCROLL_LINE_H: usize = SCROLL + 0x10;
const SCROLL_COL_V: usize = SCROLL + 0x5B0;

pub fn render(mem: &[u8], brightness: u8, out: &mut [u32]) {
    assert!(out.len() >= WIDTH * HEIGHT);

    out[..WIDTH * HEIGHT].fill(palette(mem, 0));

    for n in 0..TILEMAP_PLANES {
        paint_plane(mem, n, false, out);
    }

    paint_sprites(mem, false, out);

    for n in 0..TILEMAP_PLANES - 1 {
        paint_plane(mem, n, true, out);
    }

    paint_sprites(mem, true, out);
    paint_plane(mem, TILEMAP_PLANES - 1, true, out);

    if brightness != 255 {
        for px in &mut out[..WIDTH * HEIGHT] {
            *px = scale(*px, brightness);
        }
    }
}

fn be16(mem: &[u8], a: usize) -> u16 {
    u16::from_be_bytes([mem[a], mem[a + 1]])
}

fn paint_plane(mem: &[u8], n: usize, hi: bool, out: &mut [u32]) {
    let map = TILEMAPS + n * TILEMAP_STRIDE;
    let plane_h = be16(mem, SCROLL + n * 4);
    let plane_v = be16(mem, SCROLL + n * 4 + 2);

    for y in 0..HEIGHT {
        let h = plane_h.wrapping_add(be16(mem, SCROLL_LINE_H + n * 360 + y * 2)) as usize;

        for x in 0..WIDTH {
            let v = plane_v.wrapping_add(be16(mem, SCROLL_COL_V + n * 80 + (x / 8) * 2)) as usize;
            let (sx, sy) = ((x + h) & 1023, (y + v) & 1023);
            let entry = be16(mem, map + ((sy / 8) * TILEMAP_COLS + sx / 8) * 2);

            if (entry & 0x4000 != 0) != hi {
                continue;
            }

            let (mut tx, mut ty) = (sx % 8, sy % 8);

            if entry & 0x1000 != 0 {
                tx = 7 - tx;
            }

            if entry & 0x2000 != 0 {
                ty = 7 - ty;
            }

            let index = tile_pixel(mem, (entry & 0x0FFF) as usize, tx, ty);

            if index != 0 {
                out[y * WIDTH + x] = palette(mem, index);
            }
        }
    }
}

fn paint_sprites(mem: &[u8], hi: bool, out: &mut [u32]) {
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
            (((attr & 7) + 1) * 8) as i32,
            ((((attr >> 3) & 7) + 1) * 8) as i32,
        );
        let tile = (ctrl & 0x0FFF) as usize;
        let offset = (attr >> 8) as usize;

        for sy in 0..h {
            let py = y0 + sy;

            if !(0..HEIGHT as i32).contains(&py) {
                continue;
            }

            for sx in 0..w {
                let px = x0 + sx;

                if !(0..WIDTH as i32).contains(&px) {
                    continue;
                }

                let fx = if ctrl & 0x1000 != 0 { w - 1 - sx } else { sx } as usize;
                let fy = if ctrl & 0x2000 != 0 { h - 1 - sy } else { sy } as usize;
                let t = tile + (fy / 8) * (w as usize / 8) + fx / 8;
                let index = tile_pixel(mem, t, fx % 8, fy % 8);

                if index != 0 {
                    let index = if offset == 0 {
                        index
                    } else {
                        (index + offset) & 0xFF
                    };
                    out[py as usize * WIDTH + px as usize] = palette(mem, index);
                }
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

fn tile_pixel(mem: &[u8], tile: usize, tx: usize, ty: usize) -> usize {
    mem[VRAM_BASE as usize + (tile & 0xFFF) * 64 + ty * 8 + tx] as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MEM_END;

    const TILEMAP_0: usize = TILEMAPS;

    fn checker_tile(mem: &mut [u8]) {
        for ty in 0..8 {
            for tx in 0..4 {
                mem[VRAM_BASE as usize + 64 + ty * 8 + tx] = 1;
            }
        }
    }

    fn frame(mem: &[u8]) -> Vec<u32> {
        let mut out = vec![0u32; WIDTH * HEIGHT];
        render(mem, 255, &mut out);

        out
    }

    fn set_entry(mem: &mut [u8], col: usize, row: usize, entry: u16) {
        mem[TILEMAP_0 + (row * TILEMAP_COLS + col) * 2..][..2]
            .copy_from_slice(&entry.to_be_bytes());
    }

    fn set_palette(mem: &mut [u8], i: usize, rgb: u32) {
        mem[PALETTE_BASE as usize + i * 4..][..4].copy_from_slice(&rgb.to_be_bytes());
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
        set_entry(&mut m, 1, 1, 1); // tile 1 at cell (1,1) = pixels (8..16, 8..16)
        let f = frame(&m);

        assert_eq!(f[8 * WIDTH + 8], 0x00FF_FFFF); // left half: index 1
        assert_eq!(f[8 * WIDTH + 12], 0x0010_2040); // right half: transparent
        assert_eq!(f[0], 0x0010_2040); // outside the cell: backdrop
    }

    #[test]
    fn flips_mirror_tile() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1); // plain
        set_entry(&mut m, 1, 0, 0x1001); // H flip: white half moves right
        set_entry(&mut m, 0, 1, 0x2001); // V flip: columns unchanged
        set_entry(&mut m, 1, 1, 0x3001); // H+V
        let f = frame(&m);
        let white = 0x00FF_FFFF;

        assert_eq!(f[0], white); // plain: left white
        assert_ne!(f[7], white); // plain: right transparent
        assert_ne!(f[8], white); // H flip: left transparent
        assert_eq!(f[15], white); // H flip: right white
        assert_eq!(f[8 * WIDTH], white); // V flip: still left white
        assert_eq!(f[8 * WIDTH + 15], white); // H+V: right white
    }

    #[test]
    fn brightness_scales_output() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0080_4020);
        let mut out = vec![0u32; WIDTH * HEIGHT];

        render(&m, 127, &mut out);
        assert_eq!(out[0], 0x0040_2010);

        render(&m, 0, &mut out);
        assert_eq!(out[0], 0);
    }

    fn set_scroll(mem: &mut [u8], n: usize, h: u16, v: u16) {
        mem[SCROLL + n * 4..][..2].copy_from_slice(&h.to_be_bytes());
        mem[SCROLL + n * 4 + 2..][..2].copy_from_slice(&v.to_be_bytes());
    }

    #[test]
    fn plane_scroll_shifts_and_wraps() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1); // white left half at cell (0,0)
        set_entry(&mut m, 127, 0, 1); // and at the map's last column

        set_scroll(&mut m, 0, 4, 0); // shift left by 4: transparent half lands at x=0
        assert_ne!(frame(&m)[0], 0x00FF_FFFF);

        set_scroll(&mut m, 0, 1016, 0); // wrap: cell 127 appears at x=0
        assert_eq!(frame(&m)[0], 0x00FF_FFFF);
    }

    #[test]
    fn line_and_column_scroll_tables_add() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x00FF_FFFF);
        checker_tile(&mut m);
        set_entry(&mut m, 0, 0, 1);
        set_entry(&mut m, 1, 0, 1);

        // line 0 shifted by 4, line 1 untouched
        m[SCROLL_LINE_H..][..2].copy_from_slice(&4u16.to_be_bytes());
        let f = frame(&m);
        assert_ne!(f[0], 0x00FF_FFFF);
        assert_eq!(f[WIDTH], 0x00FF_FFFF);

        // column 0 shifted down a tile row, column 1 (cell (1,0)) untouched
        m[SCROLL_LINE_H..][..2].copy_from_slice(&0u16.to_be_bytes());
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
        m[VRAM_BASE as usize + 128..VRAM_BASE as usize + 192].fill(2); // tile 2: solid index 2
        set_entry(&mut m, 0, 0, 1); // plane 0: checker
        m[TILEMAPS + 0x8000..][..2].copy_from_slice(&2u16.to_be_bytes()); // plane 1 cell (0,0): solid
        let f = frame(&m);

        assert_eq!(f[0], 0x0022_2222); // plane 1 covers plane 0
    }

    #[test]
    fn sprite_renders_clips_and_offsets_palette() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 0, 0x0010_2040);
        set_palette(&mut m, 1, 0x00FF_FFFF);
        set_palette(&mut m, 17, 0x00FF_0000);
        checker_tile(&mut m);
        set_sprite(&mut m, 0, 10, 20, 0x8001, 0); // 8x8, tile 1
        set_sprite(&mut m, 1, -2, 40, 0x8001, 0); // clipped at left edge
        set_sprite(&mut m, 2, 30, 60, 0x8001, 16 << 8); // palette offset 16
        let f = frame(&m);

        assert_eq!(f[20 * WIDTH + 10], 0x00FF_FFFF); // index-1 half
        assert_eq!(f[20 * WIDTH + 16], 0x0010_2040); // transparent half: backdrop
        assert_eq!(f[40 * WIDTH], 0x00FF_FFFF); // clipped sprite still draws on-screen part
        assert_eq!(f[60 * WIDTH + 30], 0x00FF_0000); // 1 + 16 = palette[17]
    }

    #[test]
    fn sprite_priority_and_ui_slot() {
        let mut m = vec![0u8; MEM_END as usize];
        set_palette(&mut m, 1, 0x0011_1111);
        set_palette(&mut m, 2, 0x0022_2222);
        set_palette(&mut m, 3, 0x0033_3333);
        checker_tile(&mut m);
        m[VRAM_BASE as usize + 128..VRAM_BASE as usize + 192].fill(2); // tile 2: solid index 2
        m[VRAM_BASE as usize + 192..VRAM_BASE as usize + 256].fill(3); // tile 3: solid index 3
        set_entry(&mut m, 0, 0, 0x4002); // plane 0 hi: solid tile 2
        set_sprite(&mut m, 0, 0, 0, 0x8001, 0); // lo sprite: under hi plane
        set_sprite(&mut m, 1, 4, 0, 0xC001, 0); // hi sprite: over hi plane
        let map3 = TILEMAPS + 3 * 0x8000;
        m[map3 + 2..map3 + 4].copy_from_slice(&0x4003u16.to_be_bytes()); // plane 3 hi cell (1,0)
        let f = frame(&m);

        assert_eq!(f[0], 0x0022_2222); // hi plane covers lo sprite
        assert_eq!(f[4], 0x0011_1111); // hi sprite covers hi plane
        assert_eq!(f[8], 0x0033_3333); // plane 3 hi (UI) covers hi sprite
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

        assert_eq!(f[0], 0x0011_1111); // sprite 0 in front of sprite 1
    }
}

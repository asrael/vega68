#ifndef V68_GFX_H
#define V68_GFX_H

#include "sys.h"

#define V68_VRAM_SIZE         0x80000
#define V68_TILE_COUNT        4096
#define V68_TILEMAP_COLS      128
#define V68_TILEMAP_ROWS      128
#define V68_TILEMAP_CELLS     (V68_TILEMAP_COLS * V68_TILEMAP_ROWS)
#define V68_TILEMAP_PLANES    4
#define V68_SPRITE_COUNT      128
#define V68_PALETTE_SIZE      256
#define V68_BRIGHTNESS_LEVELS 256
#define V68_TPU_RAM_SIZE      0x400000

typedef struct { i16 x, y; u16 ctrl, attr; } V68Sprite;
typedef struct { u8 px[8][8]; } V68Tile;
typedef struct { u16 cell[V68_TILEMAP_ROWS][V68_TILEMAP_COLS]; } V68Tilemap;

typedef struct {
    struct { u16 h, v; } plane[4];
    u16 line_h[4][180];
    u16 col_v[4][40];
} V68Scroll;

typedef struct {
    u32 ring_base;
    u32 ring_words;
    u32 color_base;
    u32 z_base;
    u16 width;
    u16 height;
} V68TpuState;

#define V68_VRAM      ((volatile u8 *)0x03000000)
#define V68_TILES     ((volatile V68Tile *)0x03000000)
#define V68_TILEMAPS  ((volatile V68Tilemap *)0x03040000)
#define V68_SPRITES   ((volatile V68Sprite *)0x03060000)
#define V68_SCROLL    ((volatile V68Scroll *)0x03061000)
#define V68_VDP_MODE  ((volatile u16 *)0x03061800)
#define V68_FB_BASE   ((volatile u32 *)0x03061804)
#define V68_PALETTE   ((volatile u32 *)0x03080000)
#define V68_TPU_RAM   ((volatile u8 *)0x04000000)
#define V68_TPU_STATE ((volatile V68TpuState *)V68_TPU_RAM)

#define V68_BRIGHTNESS ((volatile u16 *)0xFF000010)

#define V68_TPU_TAIL      ((volatile u16 *)0xFF000A00)
#define V68_TPU_STATUS    ((volatile u16 *)0xFF000A04)
#define V68_TPU_HEAD      ((volatile u16 *)0xFF000A08)
#define V68_TPU_PIXELS_LO ((volatile u16 *)0xFF000A0C)
#define V68_TPU_PIXELS_HI ((volatile u16 *)0xFF000A10)

#define V68_MODE_HIRES 0x0001
#define V68_MODE_FB    0x0002

#define V68_TPU_BUSY 0x8000

#define V68_TPU_OP_TRI     0x01
#define V68_TPU_OP_FILL    0x02
#define V68_TPU_TRI_WORDS  25
#define V68_TPU_FILL_WORDS 5

#define V68_TRI_BLEND      0x01
#define V68_TRI_ZGREATER   0x02
#define V68_TRI_ZTEST_OFF  0x04
#define V68_TRI_ZWRITE_OFF 0x08

#define V68_FILL_COLOR 0x01
#define V68_FILL_Z     0x02

typedef struct { i32 x, y, z, uq, vq, q, shade; } V68Vert;

static inline u32 v68_3d_tex(u32 tex_off, u32 log2w, u32 log2h, u32 levels) {
    return (tex_off >> 3) << 12 | (log2w & 0xF) << 8 | (log2h & 0xF) << 4 | (levels & 0xF);
}

static inline u32 v68_3d_flags(i32 lod_bias, u32 flags) {
    return (u32)V68_TPU_OP_TRI << 24 | ((u32)lod_bias & 0xFF) << 8 | (flags & 0xFF);
}

void v68_3d_init(u32 ring_off, u32 ring_words, u32 color_off, u32 z_off, u16 w, u16 h);
void v68_mode(i32 hires, i32 fb);
void v68_fb(u32 fb_off);
void v68_3d_tri(u32 w0, u32 w1, u32 cmap_off, u32 blend_off, const V68Vert *a,
                const V68Vert *b, const V68Vert *c);
void v68_3d_fill(u16 x0, u16 y0, u16 x1, u16 y1, u8 flags, u8 color, u16 z);
void v68_3d_submit(void);
void v68_3d_wait(void);

void v68_2d_sprite(i32 i, i32 x, i32 y, u16 ctrl, u16 attr);
void v68_2d_scroll(i32 plane, u16 h, u16 v);
void v68_2d_palette(i32 i, u32 rgb);

#endif

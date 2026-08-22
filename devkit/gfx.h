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
#define V68_CANVAS_COLS       40
#define V68_CANVAS_ROWS       23
#define V68_CANVAS_TILES      (V68_CANVAS_COLS * V68_CANVAS_ROWS)

typedef struct { i16 x, y; u16 ctrl, attr; } V68Sprite;

typedef struct {
    i16 x, y;
    u16 tile;
    u16 flags;
    u16 w, h;
    u16 pal;
    bool off;
} V68SpriteDesc;
typedef struct { u8 px[8][8]; } V68Tile;
typedef struct { u16 cell[V68_TILEMAP_ROWS][V68_TILEMAP_COLS]; } V68Tilemap;

typedef struct {
    struct { u16 h, v; } plane[4];
    u16 line_h[4][180];
    u16 col_v[4][40];
} V68Scroll;

#define V68_VRAM      ((volatile u8 *)0x03000000)
#define V68_TILES     ((volatile V68Tile *)0x03000000)
#define V68_TILEMAPS  ((volatile V68Tilemap *)0x03040000)
#define V68_SPRITES   ((volatile V68Sprite *)0x03060000)
#define V68_SCROLL    ((volatile V68Scroll *)0x03061000)
#define V68_PALETTE   ((volatile u32 *)0x03080000)

#define V68_BRIGHTNESS ((volatile u16 *)0xFF000010)

#define V68_HFLIP 0x1000
#define V68_VFLIP 0x2000
#define V68_HI    0x4000

void v68_canvas(i32 plane, i32 tile_base);
void v68_fill(i32 x, i32 y, i32 w, i32 h, u8 color);
void v68_palette(i32 i, u32 rgb);
volatile u8 *v68_pixel(i32 x, i32 y);
void v68_scroll(i32 plane, u16 h, u16 v);
void v68_sprite(i32 i, V68SpriteDesc desc);

#endif

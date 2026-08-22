#include "gfx.h"

static u32 canvas_base;

void v68_canvas(i32 plane, i32 tile_base) {
    canvas_base = (u32)tile_base * 64;

    for (i32 r = 0; r < V68_CANVAS_ROWS; r++) {
        for (i32 c = 0; c < V68_CANVAS_COLS; c++) {
            V68_TILEMAPS[plane].cell[r][c] = (u16)(tile_base + r * V68_CANVAS_COLS + c);
        }
    }
}

void v68_fill(i32 x, i32 y, i32 w, i32 h, u8 color) {
    u32 quad = (u32)color * 0x01010101;
    i32 x1 = x + w;
    i32 y1 = y + h;

    for (i32 cy = y >> 3; cy <= (y1 - 1) >> 3; cy++) {
        i32 r0 = cy == y >> 3 ? y & 7 : 0;
        i32 r1 = cy == (y1 - 1) >> 3 ? ((y1 - 1) & 7) + 1 : 8;

        for (i32 cx = x >> 3; cx <= (x1 - 1) >> 3; cx++) {
            i32 c0 = cx == x >> 3 ? x & 7 : 0;
            i32 c1 = cx == (x1 - 1) >> 3 ? ((x1 - 1) & 7) + 1 : 8;
            volatile u8 *cell = V68_VRAM + canvas_base + ((u32)cy * V68_CANVAS_COLS + cx) * 64;

            if (c0 == 0 && c1 == 8) {
                volatile u32 *q = (volatile u32 *)(cell + r0 * 8);

                for (i32 n = (r1 - r0) * 2; n > 0; n--) {
                    *q++ = quad;
                }
            } else {
                for (i32 r = r0; r < r1; r++) {
                    for (i32 c = c0; c < c1; c++) {
                        cell[r * 8 + c] = color;
                    }
                }
            }
        }
    }
}

void v68_palette(i32 i, u32 rgb) {
    V68_PALETTE[i] = rgb;
}

volatile u8 *v68_pixel(i32 x, i32 y) {
    return &V68_VRAM[canvas_base + ((u32)(y >> 3) * V68_CANVAS_COLS + (u32)(x >> 3)) * 64
                     + (u32)((y & 7) * 8 + (x & 7))];
}

void v68_scroll(i32 plane, u16 h, u16 v) {
    V68_SCROLL->plane[plane].h = h;
    V68_SCROLL->plane[plane].v = v;
}

void v68_sprite(i32 i, V68SpriteDesc desc) {
    volatile V68Sprite *s = &V68_SPRITES[i];
    i32 w = desc.w ? desc.w : 8;
    i32 h = desc.h ? desc.h : 8;

    s->x = desc.x;
    s->y = desc.y;
    s->ctrl = (desc.off ? 0 : 0x8000) | desc.flags | desc.tile;
    s->attr = (u16)(desc.pal << 8 | (h / 8 - 1) << 3 | (w / 8 - 1));
}

#include "gfx.h"

static volatile u8 *canvas_base = V68_VRAM;
static u8 canvas_shift = 3;
static u8 requested_mode;

void v68_canvas(i32 plane, i32 tile_base) {
    canvas_shift = 3 + requested_mode;
    canvas_base = (volatile u8 *)(requested_mode ? (u32)V68_TILES_HI : (u32)V68_VRAM)
                  + ((u32)tile_base << 2 * canvas_shift);

    for (i32 r = 0; r < V68_CANVAS_ROWS; r++) {
        for (i32 c = 0; c < V68_CANVAS_COLS; c++) {
            V68_TILEMAPS[plane].cell[r][c] = (u16)(tile_base + r * V68_CANVAS_COLS + c);
        }
    }
}

void v68_fill(i32 x, i32 y, i32 w, i32 h, u8 color) {
    u32 quad = (u32)color * 0x01010101;
    i32 s = canvas_shift;
    i32 t = 1 << s;
    i32 x1 = x + w;
    i32 y1 = y + h;

    for (i32 cy = y >> s; cy <= (y1 - 1) >> s; cy++) {
        i32 r0 = cy == y >> s ? y & (t - 1) : 0;
        i32 r1 = cy == (y1 - 1) >> s ? ((y1 - 1) & (t - 1)) + 1 : t;

        for (i32 cx = x >> s; cx <= (x1 - 1) >> s; cx++) {
            i32 c0 = cx == x >> s ? x & (t - 1) : 0;
            i32 c1 = cx == (x1 - 1) >> s ? ((x1 - 1) & (t - 1)) + 1 : t;
            volatile u8 *cell = canvas_base + (((u32)cy * V68_CANVAS_COLS + cx) << 2 * s);

            if (c0 == 0 && c1 == t) {
                volatile u32 *q = (volatile u32 *)(cell + (r0 << s));

                for (i32 n = (r1 - r0) << (s - 2); n > 0; n--) {
                    *q++ = quad;
                }
            } else {
                for (i32 r = r0; r < r1; r++) {
                    for (i32 c = c0; c < c1; c++) {
                        cell[(r << s) + c] = color;
                    }
                }
            }
        }
    }
}

void v68_mode(u8 mode) {
    requested_mode = mode;
    *V68_SCREEN_MODE = mode;
}

void v68_palette(i32 i, u32 rgb) {
    V68_PALETTE[i] = rgb;
}

volatile u8 *v68_pixel(i32 x, i32 y) {
    i32 s = canvas_shift;
    i32 m = (1 << s) - 1;

    return &canvas_base[(((u32)(y >> s) * V68_CANVAS_COLS + (u32)(x >> s)) << 2 * s)
                        + (u32)(((y & m) << s) + (x & m))];
}

i32 v68_screen_h(void) {
    return 180 << requested_mode;
}

i32 v68_screen_w(void) {
    return 320 << requested_mode;
}

void v68_scroll(i32 plane, u16 h, u16 v) {
    V68_SCROLL->plane[plane].h = h;
    V68_SCROLL->plane[plane].v = v;
}

void v68_sprite(i32 i, V68_SpriteDesc desc) {
    volatile V68_Sprite *s = &V68_SPRITES[i];
    i32 shift = 3 + requested_mode;
    i32 w = desc.w ? (desc.w - 1) >> shift : 0;
    i32 h = desc.h ? (desc.h - 1) >> shift : 0;

    s->x = desc.x;
    s->y = desc.y;
    s->ctrl = (desc.off ? 0 : 0x8000) | desc.flags | desc.tile;
    s->attr = (u16)(desc.pal << 8 | h << 3 | w);
}

i32 v68_tile_px(void) {
    return 8 << requested_mode;
}

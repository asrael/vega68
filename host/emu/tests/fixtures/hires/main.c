#include "v68.h"

static void tiles(i32 first, i32 count, u8 color) {
    i32 t = v68_tile_px();
    volatile u8 *bank = t == 16 ? (volatile u8 *)V68_TILES_HI : (volatile u8 *)V68_TILES;

    for (i32 i = first * t * t; i < (first + count) * t * t; i++) {
        bank[i] = color;
    }
}

static void draw(void) {
    i32 cells = (40 + v68_tile_px() - 1) / v68_tile_px();

    v68_canvas(0, 1);
    *v68_pixel(0, 0) = 1;
    *v68_pixel(v68_screen_w() - 1, v68_screen_h() - 1) = 2;

    for (i32 i = 0; i < 32; i++) {
        *v68_pixel(4 + i, 4 + i) = 3;
    }

    v68_fill(200, 100, 30, 20, 4);
    tiles(1000, cells * cells, 5);
    tiles(1100, 1, 2);
    v68_sprite(0, (V68_SpriteDesc){ .x = 300, .y = 200, .tile = 1000, .w = 40, .h = 40 });
}

void main(void) {
    u8 mode = V68_MODE_HI;
    u16 held = 0;
    bool follow = false;

    v68_irq_init();
    v68_vblank_enable();
    v68_mode(mode);

    if (v68_screen_w() != 640 || v68_screen_h() != 360 || v68_tile_px() != 16 ||
        *V68_SCREEN_MODE != V68_MODE_LO) {
        v68_puts("bad\n");
    }

    v68_palette(1, 0x00FF0000);
    v68_palette(2, 0x0000FF00);
    v68_palette(3, 0x000000FF);
    v68_palette(4, 0x00FFFF00);
    v68_palette(5, 0x00FFFFFF);
    draw();

    if (*v68_pixel(0, 0) != 1 || *v68_pixel(639, 359) != 2 || *v68_pixel(20, 20) != 3 ||
        *v68_pixel(1, 0) != 0 || *v68_pixel(200, 100) != 4 || *v68_pixel(229, 119) != 4) {
        v68_puts("bad\n");
    } else {
        v68_puts("ok\n");
    }

    *V68_DEBUG_PUTC = 0x04;

    while (true) {
        u16 pad = *V68_PAD_1;

        v68_wait_vblank();

        if (pad & ~held & V68_PAD_A) {
            mode = !mode;
            v68_mode(mode);
            draw();
            follow = true;
        }

        held = pad;

        if (follow) {
            v68_sprite(1, (V68_SpriteDesc){ .x = *V68_MOUSE_X, .y = *V68_MOUSE_Y, .tile = 1100 });
        }
    }
}

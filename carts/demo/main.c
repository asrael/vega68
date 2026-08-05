#include "vega68_hw.h"

#define BG_TILE     5
#define BRIGHT_STEP 4
#define FADE_STEP   8
#define HOME_X      152
#define HOME_Y      82
#define MOVE_STEP   2
#define SPRITE_BODY 0x00FF8000
#define SPRITE_EDGE 0x00FFFFFF
#define SPRITE_TILE 1

static u32 dim(u32 rgb, u32 b) {
    return ((((rgb >> 16) & 0xFF) * b / 255) << 16) |
           ((((rgb >> 8) & 0xFF) * b / 255) << 8) |
           ((rgb & 0xFF) * b / 255);
}

static void load_box(i32 tile) {
    for (i32 t = 0; t < 4; t++)
        for (i32 py = 0; py < 8; py++)
            for (i32 px = 0; px < 8; px++) {
                i32 gx = (t & 1) * 8 + px;
                i32 gy = (t >> 1) * 8 + py;
                bool edge = gx == 0 || gx == 15 || gy == 0 || gy == 15;

                V68_VRAM[(tile + t) * 64 + py * 8 + px] = edge ? 3 : 2;
            }
}

static void load_checker(i32 tile) {
    for (i32 i = 0; i < 64; i++)
        V68_VRAM[tile * 64 + i] = (u8)(((i & 7) / 4 + (i >> 3) / 4) & 1);
}

static void setup(void) {
    V68_PALETTE[0] = 0x00102040;
    V68_PALETTE[1] = 0x003A3050;

    load_checker(BG_TILE);
    load_box(SPRITE_TILE);

    volatile u16 *map = V68_TILEMAP(0);

    for (i32 i = 0; i < V68_TILEMAP_CELLS; i++)
        map[i] = BG_TILE;

    V68_SPRITES[0] = HOME_X;
    V68_SPRITES[1] = HOME_Y;
    V68_SPRITES[2] = 0x8000 | SPRITE_TILE;
    V68_SPRITES[3] = (1 << 3) | 1;
}

void main(void) {
    v68_irq_init();
    v68_vblank_on();
    setup();

    for (u32 b = 0; b < 255; b += FADE_STEP) {
        V68_PALETTE[2] = dim(SPRITE_BODY, b);
        V68_PALETTE[3] = dim(SPRITE_EDGE, b);
        v68_wait_vblank();
    }

    V68_PALETTE[2] = SPRITE_BODY;
    V68_PALETTE[3] = SPRITE_EDGE;

    i32 x = HOME_X;
    i32 y = HOME_Y;
    u16 bright = 255;
    u16 scroll = 0;

    while (true) {
        v68_wait_vblank();

        u16 pad = *V68_PAD_1;

        if (pad & V68_PAD_LEFT) x -= MOVE_STEP;
        if (pad & V68_PAD_RIGHT) x += MOVE_STEP;
        if (pad & V68_PAD_UP) y -= MOVE_STEP;
        if (pad & V68_PAD_DOWN) y += MOVE_STEP;

        if ((pad & V68_PAD_A) && bright >= BRIGHT_STEP) bright -= BRIGHT_STEP;
        if ((pad & V68_PAD_B) && bright <= 255 - BRIGHT_STEP) bright += BRIGHT_STEP;

        if (pad & V68_PAD_START) {
            bright = 255;
            x = HOME_X;
            y = HOME_Y;
        }

        scroll++;

        *V68_BRIGHTNESS = bright;
        V68_SCROLL[0] = (u16)(scroll / 2);
        V68_SCROLL[1] = (u16)((scroll + 1) / 2);
        V68_SPRITES[0] = (u16)x;
        V68_SPRITES[1] = (u16)y;
    }
}

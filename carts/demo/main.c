#include "vega68_hw.h"
#include "vega68_sfx.h"

#define BG_TILE     5
#define BRIGHT_STEP 4
#define FADE_STEP   8
#define HOME_X      152
#define HOME_Y      82
#define MOVE_STEP   2
#define SPRITE_BODY 0x00FF8000
#define SPRITE_EDGE 0x00FFFFFF
#define SPRITE_TILE 1

// intro (once): brass call, harp answers with a rising 16th flourish
static const char *intro_lead[] = { "c4 e4 g4 c5", "c5" };
static const char *intro_harp[] = {
    "~",
    "[c5 e5 g5 c6] [e5 g5 c6 e6] [g5 c6 e6 g6] [c6 e6 g6 c6]",
};
static const V68Track intro_tracks[] = {
    { .bars = intro_lead, .bar_count = 2, .patch = V68_PATCH_BRASS, .ch = 0 },
    { .bars = intro_harp, .bar_count = 2, .patch = V68_PATCH_HARP, .ch = 1 },
};
static const V68Section intro_section = { .tracks = intro_tracks, .track_count = 2, .bar_frames = 90 };

static const char *body_lead[] = {
    "c4 e4 g4 c5",
    "f4 a4 c5 f5",
    "g4 b4 d5 g5",
    "e5@2 d5 c5",
};
static const char *body_harp[] = {
    "[c3 e3 g3 c4] [c3 e3 g3 c4] [c3 e3 g3 c4] [c3 e3 g3 c4]",
    "[f3 a3 c4 f4] [f3 a3 c4 f4] [f3 a3 c4 f4] [f3 a3 c4 f4]",
    "[g3 b3 d4 g4] [g3 b3 d4 g4] [g3 b3 d4 g4] [g3 b3 d4 g4]",
    "[c4 g3 e3 c3] [c4 g3 e3 c3] [c4 g3 e3 c3] [c4 g3 e3 c3]",
};
static const char *body_strings[] = { "c4", "-", "g3", "e4" };
static const char *body_bass[] = {
    "c2 g2 c2 g2",
    "f2 c3 f2 c3",
    "g2 d3 g2 d3",
    "c2 g2 c2 g2",
};
static const char *body_perc[] = {
    "k ~ h ~ k s h ~",
    "k ~ h s k s h ~",
    "k ~ h ~ k s h ~",
    "k ~ h s k h s h",
};
static const V68Track body_tracks[] = {
    { body_lead, 4, V68_PATCH_BRASS, 0, 0 },
    { body_harp, 4, V68_PATCH_HARP, 1, 0 },
    { body_strings, 4, V68_PATCH_STRINGS, 2, 0 },
    { body_bass, 4, V68_PATCH_BASS, 3, 0 },
    { body_perc, 4, V68_PATCH_PERC, 11, 0, 6 },
};
static const V68Section body_section = { .tracks = body_tracks, .track_count = 5, .bar_frames = 90 };

static const V68Section fanfare_sections[] = { intro_section, body_section };
static const V68Song fanfare = { .sections = fanfare_sections, .section_count = 2, .loop_section = 1 };

static const u8 echo_fir[8] = { 90, 40, 18, 8, 4, 2, 1, 1 }; // dark set: hall tail, no ring

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

                V68_TILES[tile + t].px[py][px] = edge ? 3 : 2;
            }
}

static void load_checker(i32 tile) {
    for (i32 i = 0; i < 64; i++)
        V68_TILES[tile].px[i >> 3][i & 7] = (u8)(((i & 7) / 4 + (i >> 3) / 4) & 1);
}

static void setup(void) {
    V68_PALETTE[0] = 0x00102040;
    V68_PALETTE[1] = 0x003A3050;

    *V68_AUDIO_EDELAY = 20; // 80 ms
    *V68_AUDIO_EFB = 70;
    *V68_AUDIO_EVOL_L = 80;
    *V68_AUDIO_EVOL_R = 80;
    for (u8 i = 0; i < 8; i++)
        V68_AUDIO_EFIR[i] = echo_fir[i];

    load_checker(BG_TILE);
    load_box(SPRITE_TILE);

    volatile u16 *map = &V68_TILEMAPS[0].cell[0][0];

    for (i32 i = 0; i < V68_TILEMAP_CELLS; i++)
        map[i] = BG_TILE;

    V68_SPRITES[0].x = HOME_X;
    V68_SPRITES[0].y = HOME_Y;
    V68_SPRITES[0].ctrl = 0x8000 | SPRITE_TILE;
    V68_SPRITES[0].attr = (1 << 3) | 1;
}

void main(void) {
    v68_irq_init();
    v68_vblank_enable();
    setup();

    if (v68_song_start(&fanfare) != 0)
        v68_puts("demo: song failed\n");

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
        V68_SCROLL->plane[0].h = (u16)(scroll / 2);
        V68_SCROLL->plane[0].v = (u16)((scroll + 1) / 2);
        V68_SPRITES[0].x = (i16)x;
        V68_SPRITES[0].y = (i16)y;
    }
}

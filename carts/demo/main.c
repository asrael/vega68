#include "v68.h"

#define BG_TILE     5
#define BRIGHT_STEP 4
#define FADE_STEP   8
#define HOME_X      152
#define HOME_Y      82
#define MOVE_STEP   2
#define SPRITE_BODY 0x00FF8000
#define SPRITE_EDGE 0x00FFFFFF
#define SPRITE_TILE 1

static const char *body_lead[] = {
    "g4 c5 ~ c5 e5 c5 ~ g4",
    "a4 c5 ~ c5 f5 c5 ~ a4",
    "b4 d5 ~ d5 g5 d5 ~ b4",
    "e5@2 d5 c5@3 ~ g4",
};
static const char *body_bass[] = {
    "c2 c3 c2 c2 c3 c2 g2 c3",
    "f2 f3 f2 f2 f3 f2 c3 f3",
    "g2 g3 g2 g2 g3 g2 d3 g3",
    "c2 c3 c2 g2 c3 c2 g2 c3",
};
static const char *body_perc[] = {
    "k h s h k h s h",
    "k h s h k h s h",
    "k h s h k h s h",
    "k h s h k s s s",
};
static const V68Track groove_tracks[] = {
    { body_bass, 4, V68_PATCH_BASS, 3, 0 },
    { body_perc, 4, V68_PATCH_PERC, 11, 0, 6 },
};
static const V68Track full_tracks[] = {
    { body_lead, 4, V68_PATCH_BRASS, 0, 0 },
    { body_bass, 4, V68_PATCH_BASS, 3, 0 },
    { body_perc, 4, V68_PATCH_PERC, 11, 0, 6 },
};

static const V68Section sections[] = {
    { .tracks = groove_tracks, .track_count = 2, .bar_frames = 90 },
    { .tracks = full_tracks, .track_count = 3, .bar_frames = 90 },
};
static const V68Song song = { .sections = sections, .section_count = 2, .loop_section = 1 };

static const u8 echo_fir[8] = { 64, 32, 16, 8, 4, 2, 1, 1 };

static V68SpriteDesc box = {
    .x = HOME_X, .y = HOME_Y, .tile = SPRITE_TILE, .w = 16, .h = 16,
};

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

static void load_solid(i32 tile) {
    for (i32 i = 0; i < 64; i++)
        V68_TILES[tile].px[i >> 3][i & 7] = 1;
}

static void setup(void) {
    v68_palette(0, 0x00102040);
    v68_palette(1, 0x00182848);

    *V68_AUDIO_EDELAY = 6;
    *V68_AUDIO_EFB = 60;
    *V68_AUDIO_EVOL_L = 60;
    *V68_AUDIO_EVOL_R = 60;
    for (u8 i = 0; i < 8; i++)
        V68_AUDIO_EFIR[i] = echo_fir[i];

    load_solid(BG_TILE);
    load_box(SPRITE_TILE);

    volatile u16 *map = &V68_TILEMAPS[0].cell[0][0];

    for (i32 i = 0; i < V68_TILEMAP_CELLS; i++)
        map[i] = ((i >> 7) + i) & 1 ? BG_TILE : 0;

    v68_sprite(0, box);
}

void main(void) {
    v68_irq_init();
    v68_vblank_enable();
    setup();

    if (v68_song_start(&song) != 0)
        v68_puts("demo: song failed\n");

    for (u32 b = 0; b < 255; b += FADE_STEP) {
        v68_palette(2, dim(SPRITE_BODY, b));
        v68_palette(3, dim(SPRITE_EDGE, b));
        v68_wait_vblank();
    }

    v68_palette(2, SPRITE_BODY);
    v68_palette(3, SPRITE_EDGE);

    i32 x = HOME_X;
    i32 y = HOME_Y;
    u16 bright = 255;

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

        *V68_BRIGHTNESS = bright;
        v68_scroll(0, (u16)(x / 2), (u16)(y / 2));
        box.x = (i16)x;
        box.y = (i16)y;
        v68_sprite(0, box);
    }
}

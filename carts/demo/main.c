#include "v68.h"

#define BANK_A    1
#define BANK_B    (1 + V68_CANVAS_TILES)
#define BODY      1
#define EDGE      2
#define OUTLINE   3
#define STEEL     4
#define SHADE     5
#define GOLD      6
#define FX_FRAMES 8
#define HALF      12
#define MOVE_STEP 2
#define RADIUS    17
#define SPIN_STEP 6

static const char *hand_point[16] = {
    "...OO...........",
    "..OSSO..........",
    "..OSSO..........",
    "..OSSO..........",
    "..OSSO..........",
    "..OSSOOOOO......",
    "..OSSOSSSSOOO...",
    "..OSSOSSSSOSSO..",
    "OOOSSSSSSSSSSO..",
    "OSDSSSSSSSSSSO..",
    "OSDSSSSSSSSSO...",
    ".OSSSSSSSSSSO...",
    ".OOOOOOOOOOOO...",
    "..OGGGGGGGGO....",
    "..OGGGGGGGGO....",
    "..OOOOOOOOOO....",
};

static const char *hand_grab[16] = {
    "................",
    "................",
    "..OOOOOOOOOO....",
    ".OSSOSSOSSOSO...",
    "OSSSSSSSSSSSSO..",
    "OSSSSSSSSSSSSO..",
    "ODSSSSSSSSSSDO..",
    "ODSSSSSSSSSSDO..",
    "ODSSSSSSSSSSDO..",
    ".ODSSSSSSSSDO...",
    ".OODSSSSSSDOO...",
    "..OOOOOOOOOO....",
    "..OGGGGGGGGO....",
    "..OGGGGGGGGO....",
    "..OOOOOOOOOO....",
    "................",
};

static const u8 fx_grab[][2] = { { 5, 4 }, { 5, 9 } };
static const u8 fx_drop[][2] = { { 4, 9 }, { 4, 4 } };

static u32 angle;
static u32 draw_base;
static u8 fx_len;
static const u8 (*fx_seq)[2];
static u8 fx_step;
static u8 fx_timer;
static bool held;
static i32 prev_h[2][2];
static i32 prev_s[2][2] = { { 160, 90 }, { 160, 90 } };
static i32 sq_x = 160;
static i32 sq_y = 90;

static volatile u8 *px(i32 x, i32 y) {
    u32 cell = (u32)(y >> 3) * V68_CANVAS_COLS + (u32)(x >> 3);

    return V68_VRAM + draw_base + cell * 64 + (u32)(y & 7) * 8 + (u32)(x & 7);
}

static void fill_clipped(i32 x, i32 y, i32 w, i32 h, u8 color) {
    i32 x1 = x + w;
    i32 y1 = y + h;

    if (x < 0) x = 0;
    if (y < 0) y = 0;
    if (x1 > 320) x1 = 320;
    if (y1 > 180) y1 = 180;

    for (i32 cy = y; cy < y1; cy++)
        for (i32 cx = x; cx < x1; cx++)
            *px(cx, cy) = color;
}

static void draw_square(void) {
    i32 c = v68_fcos(angle);
    i32 s = v68_fsin(angle);

    for (i32 dy = -RADIUS; dy <= RADIUS; dy++)
        for (i32 dx = -RADIUS; dx <= RADIUS; dx++) {
            i32 u = (dx * c + dy * s) >> 16;
            i32 v = (dy * c - dx * s) >> 16;

            if (u < -HALF || u > HALF || v < -HALF || v > HALF)
                continue;

            bool edge = u < -HALF + 2 || u > HALF - 2 || v < -HALF + 2 || v > HALF - 2;

            *px(sq_x + dx, sq_y + dy) = edge ? EDGE : BODY;
        }
}

static void draw_hand(const char **art, i32 ox, i32 oy) {
    for (i32 y = 0; y < 16; y++)
        for (i32 x = 0; x < 16; x++) {
            char ch = art[y][x];
            i32 cx = ox + x;
            i32 cy = oy + y;

            if (ch == '.' || cx < 0 || cx > 319 || cy < 0 || cy > 179)
                continue;

            u8 color = ch == 'O' ? OUTLINE
                     : ch == 'S' ? STEEL
                     : ch == 'D' ? SHADE
                                 : GOLD;

            *px(cx, cy) = color;
        }
}

static void note_on(u8 oct, u8 pc) {
    u16 fnum = v68_fnum[pc];

    V68_AUDIO_CH(0)[0x1C] = (u8)((oct << 3) | (fnum >> 8));
    V68_AUDIO_CH(0)[0x1D] = (u8)(fnum & 0xFF);
    *V68_AUDIO_KEYON = 0xF0;
}

static void fx_play(const u8 (*seq)[2], u8 len) {
    fx_seq = seq;
    fx_len = len;
    fx_step = 0;
    fx_timer = FX_FRAMES;
    note_on(seq[0][0], seq[0][1]);
}

static void fx_tick(void) {
    if (!fx_timer || --fx_timer)
        return;

    if (++fx_step < fx_len) {
        fx_timer = FX_FRAMES;
        note_on(fx_seq[fx_step][0], fx_seq[fx_step][1]);
    } else {
        *V68_AUDIO_KEYON = 0;
    }
}

void main(void) {
    v68_irq_init();
    v68_vblank_enable();

    v68_palette(0, 0x00102040);
    v68_palette(BODY, 0x00FF8000);
    v68_palette(EDGE, 0x00FFC080);
    v68_palette(OUTLINE, 0x00101018);
    v68_palette(STEEL, 0x00C8D0E0);
    v68_palette(SHADE, 0x008890A8);
    v68_palette(GOLD, 0x00D8A028);
    v68_canvas(0, BANK_A);
    v68_fm_patch(0, &v68_patches[V68_PATCH_HARP]);

    i32 back = 1;
    u16 bank[2] = { BANK_A, BANK_B };
    u16 prev_btn = 0;

    while (true) {
        u16 pad = *V68_PAD_1;
        u16 btn = *V68_MOUSE_BTN;
        i32 mx = *V68_MOUSE_X;
        i32 my = *V68_MOUSE_Y;

        if ((btn & V68_MOUSE_L) && !(prev_btn & V68_MOUSE_L) &&
            mx >= sq_x - RADIUS && mx <= sq_x + RADIUS &&
            my >= sq_y - RADIUS && my <= sq_y + RADIUS) {
            held = true;
            fx_play(fx_grab, 2);
        }

        if (held && !(btn & V68_MOUSE_L)) {
            held = false;
            fx_play(fx_drop, 2);
        }

        bool driven = pad & (V68_PAD_LEFT | V68_PAD_RIGHT | V68_PAD_UP | V68_PAD_DOWN);

        if (held) {
            sq_x = mx;
            sq_y = my;
        } else {
            if (pad & V68_PAD_LEFT) sq_x -= MOVE_STEP;
            if (pad & V68_PAD_RIGHT) sq_x += MOVE_STEP;
            if (pad & V68_PAD_UP) sq_y -= MOVE_STEP;
            if (pad & V68_PAD_DOWN) sq_y += MOVE_STEP;
        }

        if (sq_x < RADIUS) sq_x = RADIUS;
        if (sq_x > 319 - RADIUS) sq_x = 319 - RADIUS;
        if (sq_y < RADIUS) sq_y = RADIUS;
        if (sq_y > 179 - RADIUS) sq_y = 179 - RADIUS;

        if (!held && !driven)
            angle += SPIN_STEP;

        fx_tick();

        i32 hx = held ? mx - 7 : mx - 3;
        i32 hy = held ? my - 7 : my;

        draw_base = (u32)bank[back] * 64;

        fill_clipped(prev_s[back][0] - RADIUS, prev_s[back][1] - RADIUS,
                     2 * RADIUS + 1, 2 * RADIUS + 1, 0);
        fill_clipped(prev_h[back][0], prev_h[back][1], 16, 16, 0);
        draw_square();
        draw_hand(held ? hand_grab : hand_point, hx, hy);

        prev_s[back][0] = sq_x;
        prev_s[back][1] = sq_y;
        prev_h[back][0] = hx;
        prev_h[back][1] = hy;
        prev_btn = btn;

        v68_wait_vblank();
        v68_canvas(0, bank[back]);

        back ^= 1;
    }
}

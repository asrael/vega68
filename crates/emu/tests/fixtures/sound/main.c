#include "vega68_sfx.h"

static const V68Patch ping = {
    .op = { { 0x01, 0, 0x1F, 0, 0, 0x05, 0 },
            { 0x01, 127, 0x1F, 0, 0, 0, 0 },
            { 0x01, 127, 0x1F, 0, 0, 0, 0 },
            { 0x01, 127, 0x1F, 0, 0, 0, 0 } },
    .fb_alg = 0x07,
    .echo = 1,
};
// fixture-local patch via patch_ptr: preset tuning must never move this golden

// bar 1: hold across barline, rest, hold extending THE REST (pins semantic: a hold on a
// rest keeps it silent), then a plain note
static const char *lead[] = { "a4 [c5 e5] ~ g4@1", "- ~ - e4" };
static const char *bass[] = { "a2 ~ a2 ~", "e2 - ~ a2" };
// PSG period must stay <= 1023: a2/b2 are the low end of what octave 2 allows
static const char *square[] = { "a2 c3 e3 a2", "b2 d3 f3 a2" };
static const char *perc[] = { "k h s h", "k h s h" };
static const V68Track tracks[] = {
    { .bars = lead, .bar_count = 2, .ch = 0, .patch_ptr = &ping },
    { .bars = bass, .bar_count = 2, .ch = 1, .level = 13, .patch_ptr = &ping },
    { .bars = square, .bar_count = 2, .ch = 8, .level = 13, .patch_ptr = &ping },
    { .bars = perc, .bar_count = 2, .ch = 11, .patch_ptr = &ping },
};
static const V68Section body[] = { { .tracks = tracks, .track_count = 4, .bar_frames = 24 } };
static const V68Song song = { .sections = body, .section_count = 1, .loop_section = 0 };

void main(void) {
    v68_irq_init();
    v68_vblank_enable();

    if (v68_song_start(&song) != 0) {
        *V68_DEBUG_PUTC = 0x04;
        while (true) {}
    }

    for (i32 i = 0; i < 48; i++)
        v68_wait_vblank(); // two bars exactly

    v68_puts("ok\n");
    *V68_DEBUG_PUTC = 0x04;
    while (true) {}
}

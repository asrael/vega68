#include "vega68_sfx.h"

static const char *tricky[] = { "a2@3 [c3 [d3 e3]] ~ -", "- b2 ~ ~" };
static const V68Track t_ok[] = { { tricky, 2, 0, 0, 0 } };
static const V68Section s_ok[] = { { t_ok, 1, 96 } };
static const V68Song ok = { s_ok, 1, 0 };

static const char *bad_tok[] = { "a2 x9 c3" };
static const char *bad_hold[] = { "- a2" };
static const char *bad_psg[] = { "c1 c1 c1 c1" };
static const char *bad_group[] = {
    "~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~"
};
static const char *bad_empty_bar[] = { "" };
static const char *bad_empty_bracket[] = { "a2 [] c3" };
static const char *bad_weight[] = { "a2@0" };
static const char *bad_bracket[] = { "a2 [c3 d3" };
static const char *uneven_1bar[] = { "a2 a2 a2 a2" };
static const char *uneven_2bar[] = { "a2 a2 a2 a2", "a2 a2 a2 a2" };
static const char *loop_ok[] = { "a2 a2 a2 a2" };
static const V68Track t_b1[] = { { bad_tok, 1, 0, 0, 0 } };
static const V68Track t_b2[] = { { bad_hold, 1, 0, 1, 0 } };
static const V68Track t_b3[] = { { bad_psg, 1, 0, 8, 0 } };
static const V68Track t_b4[] = { { bad_group, 1, 0, 0, 0 } };
static const V68Track t_b5[] = { { bad_empty_bar, 1, 0, 0, 0 } };
static const V68Track t_b6[] = { { bad_empty_bracket, 1, 0, 0, 0 } };
static const V68Track t_b7[] = { { loop_ok, 1, 0, 12, 0 } };
static const V68Track t_b8[] = { { loop_ok, 1, 12, 0, 0 } };
static const V68Track t_b9[] = { { bad_weight, 1, 0, 0, 0 } };
static const V68Track t_b10[] = { { bad_bracket, 1, 0, 0, 0 } };
static const V68Track t_b11[] = {
    { uneven_1bar, 1, 0, 0, 0 },
    { uneven_2bar, 2, 0, 1, 0 },
};
static const V68Track t_b12[] = { { loop_ok, 1, 0, 0, 0 } };
static const V68Section s_b1[] = { { t_b1, 1, 96 } };
static const V68Section s_b2[] = { { t_b2, 1, 96 } };
static const V68Section s_b3[] = { { t_b3, 1, 96 } };
static const V68Section s_b4[] = { { t_b4, 1, 96 } };
static const V68Section s_b5[] = { { t_b5, 1, 96 } };
static const V68Section s_b6[] = { { t_b6, 1, 96 } };
static const V68Section s_b7[] = { { t_b7, 1, 96 } };
static const V68Section s_b8[] = { { t_b8, 1, 96 } };
static const V68Section s_b9[] = { { t_b9, 1, 96 } };
static const V68Section s_b10[] = { { t_b10, 1, 96 } };
static const V68Section s_b11[] = { { t_b11, 2, 96 } };
static const V68Section s_b12[] = { { t_b12, 1, 96 } };
static const V68Song b1 = { s_b1, 1, 0 };
static const V68Song b2 = { s_b2, 1, 0 };
static const V68Song b3 = { s_b3, 1, 0 };
static const V68Song b4 = { s_b4, 1, 0 };
static const V68Song b5 = { s_b5, 1, 0 };
static const V68Song b6 = { s_b6, 1, 0 };
static const V68Song b7 = { s_b7, 1, 0 };
static const V68Song b8 = { s_b8, 1, 0 };
static const V68Song b9 = { s_b9, 1, 0 };
static const V68Song b10 = { s_b10, 1, 0 };
static const V68Song b11 = { s_b11, 1, 0 };
static const V68Song b12 = { s_b12, 1, 3 };

void main(void) {
    if (v68_song_start(&ok) == 0)
        v68_puts("parse ok\n");
    v68_song_stop();

    if (v68_song_start(&b1) < 0 && v68_song_start(&b2) < 0 && v68_song_start(&b3) < 0 &&
        v68_song_start(&b4) < 0 && v68_song_start(&b5) < 0 && v68_song_start(&b6) < 0 &&
        v68_song_start(&b7) < 0 && v68_song_start(&b8) < 0 && v68_song_start(&b9) < 0 &&
        v68_song_start(&b10) < 0 && v68_song_start(&b11) < 0 && v68_song_start(&b12) < 0)
        v68_puts("rejects ok\n");

    *V68_DEBUG_PUTC = 0x04;
    while (true) {}
}

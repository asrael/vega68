#include "vega68_sfx.h"

#define V68_SOUND_MAX_TRACKS  64 // flat (section,track) cap across a song; generous headroom
#define V68_GROUP_MAX         32 // top-level or bracket element cap per notation group
#define V68_SOUND_MAX_ACTIVE   8 // per-section track cap, enforced in parse_section

typedef struct { u8 note; u16 frames; } V68Ev;
typedef struct { u16 first; u16 count; } V68TrackLayout;
typedef struct { u16 next_idx; u16 frames_left; } V68TrackState; // per active track, this section

// one notation element, bounds into the bar string; brackets carry inner content bounds.
// elements the scanner rejects outright (unterminated bracket, @n out of range) set bad;
// [start, full_end) is the whole token for the diagnostic.
typedef struct {
    bool is_bracket;
    const char *start;
    const char *end;
    const char *full_end;
    i32 weight;
    bool bad;
} Elem;

// per-track parse state, threaded through the group/leaf recursion
typedef struct {
    u8 ch;
    i8 transpose;
    bool perc;
    i32 last_event; // pool index of the last appended event (note or rest), or -1
    bool overflow;
    bool empty_group;
    const char *bad_tok;
    i32 bad_len;
} PCtx;

// bit n set = op n is a carrier under algorithm n; vol attenuates carriers only
static const u8 alg_carriers[8] = { 0x08, 0x08, 0x08, 0x08, 0x0A, 0x0E, 0x0E, 0x0F };

// PSG plays full scale where FM carries patch TL headroom; this base drop
// puts a full-level PSG track at the same loudness as a typical FM patch.
#define V68_PSG_LEVEL_BASE  6
#define V68_PERC_LEVEL_BASE 3 // transients read quieter than sustained tones; halve their drop

static V68Ev pool[V68_SOUND_POOL];
static u16 pool_used;
static V68TrackLayout layout[V68_SOUND_MAX_TRACKS];
static u16 layout_used;
static const V68Song *playing;

// active-playback state: current section and its track cursors
static u8  section;
static u16 section_base;        // layout index where this section's tracks start
static u8  section_tracks;      // == playing->sections[section].track_count
static u32 section_frames_left;
static V68TrackState state[V68_SOUND_MAX_ACTIVE];

/// block-4 fnums C4..B4, fnum = round(freq * 2^20 / (53267.03 * 8)), A4 = 440 Hz
const u16 v68_fnum[12] = {
    644, 682, 723, 766, 811, 859, 910, 965, 1022, 1083, 1147, 1215,
};

/// octave-4 PSG periods, period = round(3579545 / (32 * freq))
const u16 v68_psg_period[12] = {
    428, 404, 381, 360, 339, 320, 302, 285, 269, 254, 240, 226,
};

// first-pass FM voices for the SF/CT/LttP palette; byte layout and algorithm
// wiring per ~spec/2026-08-06-sound-system.md. Tuning is ear-work for later.
const V68Patch v68_patches[12] = {
    [V68_PATCH_BRASS] = {
        .op = {
            { 0x52, 0x1E, 0x12, 0x0A, 0x05, 0x36, 0x00 },
            { 0x41, 0x14, 0x1C, 0x0A, 0x05, 0x27, 0x00 },
            { 0x33, 0x1C, 0x14, 0x0A, 0x05, 0x36, 0x00 },
            { 0x41, 0x12, 0x1C, 0x0A, 0x05, 0x27, 0x00 },
        },
        .fb_alg = 0x24,
        .echo = 1,
    },
    [V68_PATCH_STRINGS] = {
        .op = {
            { 0x61, 0x23, 0x10, 0x06, 0x03, 0x16, 0x00 },
            { 0x41, 0x0C, 0x0F, 0x04, 0x02, 0x16, 0x00 },
            { 0x31, 0x0A, 0x0E, 0x04, 0x02, 0x16, 0x00 },
            { 0x51, 0x0A, 0x12, 0x04, 0x02, 0x16, 0x00 },
        },
        .fb_alg = 0x0E,
        .echo = 1,
    },
    [V68_PATCH_CHOIR] = {
        .op = {
            { 0x51, 0x14, 0x0C, 0x05, 0x03, 0x26, 0x00 },
            { 0x41, 0x16, 0x0C, 0x05, 0x03, 0x26, 0x00 },
            { 0x31, 0x18, 0x0C, 0x05, 0x03, 0x26, 0x00 },
            { 0x61, 0x1A, 0x0C, 0x05, 0x03, 0x26, 0x00 },
        },
        .fb_alg = 0x07,
        .echo = 1,
    },
    [V68_PATCH_EPIANO] = {
        .op = {
            { 0x77, 0x20, 0x19, 0x0E, 0x04, 0x38, 0x00 },
            { 0x41, 0x14, 0x1C, 0x10, 0x03, 0x48, 0x00 },
            { 0x32, 0x18, 0x1C, 0x10, 0x03, 0x48, 0x00 },
            { 0x54, 0x1E, 0x1C, 0x10, 0x03, 0x48, 0x00 },
        },
        .fb_alg = 0x1D,
        .echo = 0,
    },
    [V68_PATCH_BELL] = {
        .op = {
            { 0x62, 0x14, 0x1F, 0x12, 0x06, 0x49, 0x00 },
            { 0x47, 0x0E, 0x1F, 0x14, 0x06, 0x49, 0x00 },
            { 0x33, 0x16, 0x1F, 0x12, 0x06, 0x49, 0x00 },
            { 0x5B, 0x0C, 0x1F, 0x14, 0x06, 0x49, 0x00 },
        },
        .fb_alg = 0x14,
        .echo = 1,
    },
    [V68_PATCH_FLUTE] = {
        .op = {
            { 0x51, 0x14, 0x14, 0x06, 0x03, 0x26, 0x00 },
            { 0x31, 0x1A, 0x14, 0x06, 0x03, 0x26, 0x00 },
            { 0x41, 0x7F, 0x14, 0x06, 0x03, 0x26, 0x00 },
            { 0x41, 0x7F, 0x14, 0x06, 0x03, 0x26, 0x00 },
        },
        .fb_alg = 0x0F,
        .echo = 1,
    },
    [V68_PATCH_HARP] = {
        .op = {
            { 0x41, 0x0A, 0x1F, 0x19, 0x08, 0x6A, 0x00 },
            { 0x42, 0x14, 0x1F, 0x14, 0x06, 0x59, 0x00 },
            { 0x43, 0x1A, 0x1F, 0x12, 0x05, 0x59, 0x00 },
            { 0x41, 0x10, 0x1F, 0x16, 0x06, 0x8A, 0x00 },
        },
        .fb_alg = 0x11,
        .echo = 0,
    },
    [V68_PATCH_ORGAN] = {
        .op = {
            { 0x41, 0x0F, 0x1F, 0x01, 0x00, 0x08, 0x00 },
            { 0x42, 0x14, 0x1F, 0x01, 0x00, 0x08, 0x00 },
            { 0x44, 0x19, 0x1F, 0x01, 0x00, 0x08, 0x00 },
            { 0x48, 0x23, 0x1F, 0x01, 0x00, 0x08, 0x00 },
        },
        .fb_alg = 0x07,
        .echo = 0,
    },
    [V68_PATCH_BASS] = {
        .op = {
            { 0x41, 0x14, 0x1F, 0x0E, 0x04, 0x37, 0x00 },
            { 0x41, 0x0A, 0x1F, 0x12, 0x04, 0x37, 0x00 },
            { 0x41, 0x19, 0x1F, 0x0C, 0x04, 0x37, 0x00 },
            { 0x41, 0x08, 0x1F, 0x08, 0x03, 0x27, 0x00 },
        },
        .fb_alg = 0x1B,
        .echo = 0,
    },
    [V68_PATCH_LEAD] = {
        .op = {
            { 0x53, 0x14, 0x1F, 0x08, 0x03, 0x26, 0x00 },
            { 0x41, 0x08, 0x1F, 0x08, 0x03, 0x26, 0x00 },
            { 0x32, 0x18, 0x1F, 0x08, 0x03, 0x26, 0x00 },
            { 0x61, 0x06, 0x1F, 0x08, 0x03, 0x26, 0x00 },
        },
        .fb_alg = 0x24,
        .echo = 1,
    },
    [V68_PATCH_PLUCK] = {
        .op = {
            { 0x41, 0x0F, 0x1F, 0x19, 0x08, 0x9A, 0x00 },
            { 0x42, 0x19, 0x1F, 0x19, 0x08, 0x9A, 0x00 },
            { 0x43, 0x1E, 0x1F, 0x19, 0x08, 0x9A, 0x00 },
            { 0x41, 0x0C, 0x1F, 0x1C, 0x0A, 0xAB, 0x00 },
        },
        .fb_alg = 0x2A,
        .echo = 0,
    },
    // ch-11 tracks never touch the FM op array (perc_ctrl/perc_decay drive the noise
    // kit directly); these are BASS's op bytes with FB bumped so the fingerprint stays
    // distinct. Only .echo is live for a V68_PATCH_PERC track.
    [V68_PATCH_PERC] = {
        .op = {
            { 0x41, 0x14, 0x1F, 0x0E, 0x04, 0x37, 0x00 },
            { 0x41, 0x0A, 0x1F, 0x12, 0x04, 0x37, 0x00 },
            { 0x41, 0x19, 0x1F, 0x0C, 0x04, 0x37, 0x00 },
            { 0x41, 0x08, 0x1F, 0x08, 0x03, 0x27, 0x00 },
        },
        .fb_alg = 0x33,
        .echo = 0,
    },
};

static void sound_clear(void) {
    playing                = 0;
    pool_used           = 0;
    layout_used         = 0;
    section             = 0;
    section_base        = 0;
    section_tracks      = 0;
    section_frames_left = 0;
}

static void putdec(u32 v) {
    char buf[3]; // u8 max 255
    i32 n = 0;

    if (v == 0) {
        *V68_DEBUG_PUTC = '0';
        return;
    }

    while (v > 0) {
        buf[n++] = (char)('0' + v % 10);
        v /= 10;
    }

    while (n > 0)
        *V68_DEBUG_PUTC = buf[--n];
}

static void diag_bad_token(u8 sec, u8 trk, u8 bar, const char *tok, i32 len) {
    if (len > 7)
        len = 7;

    v68_puts("sound: s");
    putdec(sec);
    v68_puts(" t");
    putdec(trk);
    v68_puts(" bar");
    putdec(bar);
    v68_puts(": bad token '");

    for (i32 i = 0; i < len; i++)
        *V68_DEBUG_PUTC = tok[i];

    v68_puts("'\n");
}

static void diag_empty_group(u8 sec, u8 trk, u8 bar) {
    v68_puts("sound: s");
    putdec(sec);
    v68_puts(" t");
    putdec(trk);
    v68_puts(" bar");
    putdec(bar);
    v68_puts(": empty group\n");
}

// letter -> semitone class within an octave (c=0 .. b=11); -1 if not a note letter
static i32 pitch_class(char c) {
    switch (c) {
    case 'c': return 0;
    case 'd': return 2;
    case 'e': return 4;
    case 'f': return 5;
    case 'g': return 7;
    case 'a': return 9;
    case 'b': return 11;
    default: return -1;
    }
}

// percussion letter -> pool note slot (k/s/h); -1 if not a perc token
static i32 perc_slot(char c) {
    switch (c) {
    case 'k': return 2;
    case 's': return 3;
    case 'h': return 4;
    default: return -1;
    }
}

// parse one element (bracket-or-leaf, plus an optional trailing "@n" weight) at *pp,
// which must point at a non-space char before `end`; advances *pp past it. An
// unterminated bracket or an out-of-range "@n" (must be 1-32) sets e.bad instead of
// silently accepting a default.
static Elem scan_element(const char **pp, const char *end) {
    const char *p = *pp;
    Elem e;
    e.bad = false;
    e.weight = 1;

    if (*p == '[') {
        const char *br_start = p;
        p++;
        e.is_bracket = true;
        e.start = p;

        i32 depth = 1;
        while (p < end && depth > 0) {
            if (*p == '[') depth++;
            else if (*p == ']' && --depth == 0) break;
            p++;
        }

        if (depth > 0) {
            e.start = br_start;
            e.full_end = end;
            e.bad = true;
            *pp = end;
            return e;
        }

        e.end = p;
        p++; // skip ']'

        if (p < end && *p == '@') {
            p++;
            i32 w = 0;
            while (p < end && *p >= '0' && *p <= '9') {
                w = w * 10 + (*p - '0');
                p++;
            }
            if (w < 1 || w > 32) {
                e.start = br_start;
                e.full_end = p;
                e.bad = true;
                *pp = p;
                return e;
            }
            e.weight = w;
        }

        e.full_end = p;
        *pp = p;
        return e;
    }

    e.is_bracket = false;
    e.start = p;
    while (p < end && *p != ' ')
        p++;
    e.end = p;
    e.full_end = p;

    const char *at = 0;
    for (const char *q = e.start; q < e.end; q++) {
        if (*q == '@') {
            at = q;
            break;
        }
    }

    if (at) {
        i32 w = 0;
        for (const char *q = at + 1; q < e.end; q++) {
            if (*q >= '0' && *q <= '9')
                w = w * 10 + (*q - '0');
        }
        if (w < 1 || w > 32) {
            e.bad = true;
        } else {
            e.weight = w;
            e.end = at;
        }
    }

    *pp = p;
    return e;
}

// scan one group's top-level elements between [start,end) (bracket content or a whole bar
// string). More than V68_GROUP_MAX elements is an error (silently truncating would retime
// the bar), reported as a bad token naming the overflowing element.
static bool scan_group(PCtx *ctx, const char *start, const char *end, Elem *out, i32 *count) {
    const char *p = start;
    i32 n = 0;

    while (p < end) {
        while (p < end && *p == ' ')
            p++;
        if (p >= end)
            break;

        Elem e = scan_element(&p, end);

        if (e.bad) {
            ctx->bad_tok = e.start;
            ctx->bad_len = (i32)(e.full_end - e.start);
            *count = n;
            return false;
        }

        if (n >= V68_GROUP_MAX) {
            ctx->bad_tok = e.start;
            ctx->bad_len = (i32)(e.end - e.start);
            *count = n;
            return false;
        }

        out[n++] = e;
    }

    *count = n;
    return true;
}

static bool emit_note(PCtx *ctx, u8 note, u16 frames) {
    if (pool_used >= V68_SOUND_POOL) {
        ctx->overflow = true;
        return false;
    }

    i32 idx = pool_used++;
    pool[idx].note = note;
    pool[idx].frames = frames;
    ctx->last_event = idx;
    return true;
}

static bool emit_rest(PCtx *ctx, u16 frames) {
    if (ctx->last_event >= 0 && pool[ctx->last_event].note == 0) {
        pool[ctx->last_event].frames += frames;
        return true;
    }

    if (pool_used >= V68_SOUND_POOL) {
        ctx->overflow = true;
        return false;
    }

    i32 idx = pool_used++;
    pool[idx].note = 0;
    pool[idx].frames = frames;
    ctx->last_event = idx;
    return true;
}

static bool process_leaf(PCtx *ctx, const char *tok, const char *end, u16 frames) {
    i32 len = (i32)(end - tok);

    if (len == 1 && tok[0] == '~')
        return emit_rest(ctx, frames);

    if (len == 1 && tok[0] == '-') {
        if (ctx->last_event < 0) {
            ctx->bad_tok = tok;
            ctx->bad_len = len;
            return false;
        }
        pool[ctx->last_event].frames += frames;
        return true;
    }

    if (ctx->perc) {
        i32 slot = (len == 1) ? perc_slot(tok[0]) : -1;
        if (slot < 0) {
            ctx->bad_tok = tok;
            ctx->bad_len = len;
            return false;
        }
        return emit_note(ctx, (u8)slot, frames);
    }

    i32 pc0 = (len >= 1) ? pitch_class(tok[0]) : -1;
    if (pc0 < 0) {
        ctx->bad_tok = tok;
        ctx->bad_len = len;
        return false;
    }

    i32 idx = 1;
    i32 accidental = 0;
    if (idx < len && tok[idx] == '#') {
        accidental = 1;
        idx++;
    } else if (idx < len && tok[idx] == 'b') {
        accidental = -1;
        idx++;
    }

    if (idx >= len || tok[idx] < '0' || tok[idx] > '9' || idx + 1 != len) {
        ctx->bad_tok = tok;
        ctx->bad_len = len;
        return false;
    }

    i32 octave = tok[idx] - '0';
    i32 semitone = pc0 + accidental;

    if (semitone < 0) {
        semitone += 12;
        octave--;
    } else if (semitone > 11) {
        semitone -= 12;
        octave++;
    }

    i32 absolute = octave * 12 + semitone + ctx->transpose;
    if (absolute < 0 || absolute > 95) {
        ctx->bad_tok = tok;
        ctx->bad_len = len;
        return false;
    }

    if (ctx->ch >= 8 && ctx->ch <= 10) {
        i32 pc = absolute % 12;
        i32 oct = absolute / 12;
        u32 period = v68_psg_period[pc];

        if (oct < 4) period <<= (4 - oct);
        else period >>= (oct - 4);

        if (period > 1023) {
            ctx->bad_tok = tok;
            ctx->bad_len = len;
            return false;
        }
    }

    return emit_note(ctx, (u8)(2 + absolute), frames);
}

static bool process_group(PCtx *ctx, const Elem *elems, i32 n, u16 frames) {
    if (n == 0) {
        ctx->empty_group = true;
        return false;
    }

    i32 w = 0;
    for (i32 i = 0; i < n; i++)
        w += elems[i].weight;

    i32 bases[V68_GROUP_MAX];
    i32 base_sum = 0;
    for (i32 i = 0; i < n; i++) {
        bases[i] = (i32)frames * elems[i].weight / w;
        base_sum += bases[i];
    }
    i32 remainder = (i32)frames - base_sum;

    for (i32 i = 0; i < n; i++) {
        u16 span = (u16)(bases[i] + (i < remainder ? 1 : 0));

        if (elems[i].is_bracket) {
            Elem sub[V68_GROUP_MAX];
            i32 subn;
            if (!scan_group(ctx, elems[i].start, elems[i].end, sub, &subn))
                return false;
            if (!process_group(ctx, sub, subn, span))
                return false;
        } else if (!process_leaf(ctx, elems[i].start, elems[i].end, span)) {
            return false;
        }
    }

    return true;
}

static bool parse_bar(PCtx *ctx, const char *bar, u16 bar_frames) {
    const char *end = bar;
    while (*end != '\0')
        end++;

    Elem elems[V68_GROUP_MAX];
    i32 n;
    if (!scan_group(ctx, bar, end, elems, &n))
        return false;
    return process_group(ctx, elems, n, bar_frames);
}

static bool parse_track(u8 sec, u8 trk, const V68Track *t, u16 bar_frames, i32 *first, i32 *count) {
    PCtx ctx = {
        .ch = t->ch,
        .transpose = t->transpose,
        .perc = (t->ch == 11),
        .last_event = -1,
        .overflow = false,
        .empty_group = false,
        .bad_tok = 0,
        .bad_len = 0,
    };

    *first = pool_used;

    for (u8 b = 0; b < t->bar_count; b++) {
        if (!parse_bar(&ctx, t->bars[b], bar_frames)) {
            if (ctx.overflow)
                v68_puts("sound: pool overflow\n");
            else if (ctx.empty_group)
                diag_empty_group(sec, trk, b);
            else
                diag_bad_token(sec, trk, b, ctx.bad_tok, ctx.bad_len);
            return false;
        }
    }

    *count = pool_used - *first;
    return true;
}

static bool parse_section(u8 sec, const V68Section *s) {
    if (s->track_count > V68_SOUND_MAX_ACTIVE) {
        v68_puts("sound: too many tracks\n");
        return false;
    }

    for (u8 t = 1; t < s->track_count; t++) {
        if (s->tracks[t].bar_count != s->tracks[0].bar_count) {
            v68_puts("sound: s");
            putdec(sec);
            v68_puts(": uneven tracks\n");
            return false;
        }
    }

    for (u8 t = 0; t < s->track_count; t++) {
        const V68Track *trk = &s->tracks[t];

        if (trk->ch > 11) {
            v68_puts("sound: bad channel\n");
            return false;
        }

        if (!trk->patch_ptr && trk->patch >= 12) {
            v68_puts("sound: bad patch\n");
            return false;
        }

        if (layout_used >= V68_SOUND_MAX_TRACKS) {
            v68_puts("sound: track layout overflow\n");
            return false;
        }

        i32 first, count;
        if (!parse_track(sec, t, trk, s->bar_frames, &first, &count))
            return false;

        layout[layout_used].first = (u16)first;
        layout[layout_used].count = (u16)count;
        layout_used++;
    }

    return true;
}

void v68_fm_patch(u8 ch, const V68Patch *p) {
    volatile u8 *reg = V68_AUDIO_CH(ch);
    const u8    *src = (const u8 *)p->op;

    for (u8 i = 0; i < 28; i++)
        reg[i] = src[i];

    reg[0x1E] = p->fb_alg;
}

// percussion (ch 11) note -> noise CTRL: bit2 selects white(1)/periodic(0), bits[1:0] the
// rate divisor; k=periodic clk/2048, s=white clk/1024, h=white clk/512 (pool slots 2/3/4)
static u8 perc_ctrl(u8 note) {
    switch (note) {
    case 3:  return 0x05; // s
    case 4:  return 0x04; // h
    default: return 0x02; // k
    }
}

// kit envelope: attenuation ramps from 0 towards silence every frame the note isn't retriggered
static void perc_decay(u8 ch) {
    volatile u8 *atten = &V68_AUDIO_CH(ch)[0x02];
    u8 v = *atten;

    if (v < 15)
        *atten = (v + 2 > 15) ? 15 : (u8)(v + 2);
}

static void dispatch_event(const V68Track *trk, u16 idx) {
    u8 ch = trk->ch;
    u8 note = pool[idx].note;

    if (note == 0) {
        if (ch < 8)
            *V68_AUDIO_KEYON = ch;
        else
            V68_AUDIO_CH(ch)[0x02] = 15;
        return;
    }

    u8 steps = trk->level == 0 ? 0 : (trk->level > 15 ? 0 : 15 - trk->level);
    u32 psg_atten = (u32)V68_PSG_LEVEL_BASE + steps;
    u8 atten = psg_atten > 15 ? 15 : (u8)psg_atten;

    if (ch == 11) {
        u32 pa = (u32)V68_PERC_LEVEL_BASE + steps;

        V68_AUDIO_CH(11)[0x00] = perc_ctrl(note);
        V68_AUDIO_CH(11)[0x02] = pa > 15 ? 15 : (u8)pa;
        return;
    }

    // transpose is already folded into `note` at parse time
    i32 absolute = note - 2;
    i32 pc = absolute % 12;
    i32 oct = absolute / 12;

    if (ch < 8) {
        u16 fnum = v68_fnum[pc];

        V68_AUDIO_CH(ch)[0x1C] = (u8)((oct << 3) | (fnum >> 8));
        V68_AUDIO_CH(ch)[0x1D] = (u8)(fnum & 0xFF);
        *V68_AUDIO_KEYON = 0xF0 | ch;
    } else {
        u32 period = v68_psg_period[pc];
        period = (oct < 4) ? period << (4 - oct) : period >> (oct - 4);

        V68_AUDIO_CH(ch)[0x00] = (u8)(period >> 8);
        V68_AUDIO_CH(ch)[0x01] = (u8)(period & 0xFF);
        V68_AUDIO_CH(ch)[0x02] = atten;
    }
}

static u16 section_layout_base(u8 sec) {
    u16 base = 0;

    for (u8 s = 0; s < sec; s++)
        base += playing->sections[s].track_count;

    return base;
}

static bool section_uses_channel(const V68Section *s, u8 ch) {
    for (u8 t = 0; t < s->track_count; t++)
        if (s->tracks[t].ch == ch)
            return true;

    return false;
}

static void section_enter(u8 sec) {
    const V68Section *s = &playing->sections[sec];
    u16 base = section_layout_base(sec);
    u16 esend = *V68_AUDIO_ESEND;

    if (section_tracks > 0) {
        const V68Section *prev = &playing->sections[section];

        for (u8 t = 0; t < prev->track_count; t++) {
            u8 ch = prev->tracks[t].ch;

            if (section_uses_channel(s, ch))
                continue;

            if (ch < 8)
                *V68_AUDIO_KEYON = ch;
            else
                V68_AUDIO_CH(ch)[0x02] = 15;

            esend &= (u16)~(1 << ch);
        }
    }

    section = sec;
    section_base = base;
    section_tracks = s->track_count;
    section_frames_left = (s->track_count > 0) ? (u32)s->tracks[0].bar_count * s->bar_frames : 0;

    for (u8 t = 0; t < s->track_count; t++) {
        const V68Track *trk = &s->tracks[t];
        const V68Patch *patch = trk->patch_ptr ? trk->patch_ptr : &v68_patches[trk->patch];

        if (trk->ch < 8) {
            v68_fm_patch(trk->ch, patch);

            u8 steps = trk->level == 0 ? 0 : (trk->level > 15 ? 0 : 15 - trk->level);

            if (steps > 0) {
                volatile u8 *pg = V68_AUDIO_CH(trk->ch);
                u8 carriers = alg_carriers[patch->fb_alg & 0x07];

                for (u8 op = 0; op < 4; op++)
                    if (carriers & (1 << op)) {
                        u32 tl = (u32)pg[op * 7 + 1] + (u32)steps * 3;
                        pg[op * 7 + 1] = tl > 127 ? 127 : (u8)tl;
                    }
            }
        }

        if (patch->echo)
            esend |= (u16)(1 << trk->ch);
        else
            esend &= (u16)~(1 << trk->ch);

        state[t].next_idx = layout[base + t].first;
        state[t].frames_left = 0;
    }

    *V68_AUDIO_ESEND = esend;
}

void v68_sound_tick(void) {
    if (!playing)
        return;

    const V68Section *s = &playing->sections[section];

    for (u8 t = 0; t < section_tracks; t++) {
        const V68Track *trk = &s->tracks[t];
        V68TrackState *st = &state[t];
        const V68TrackLayout *tl = &layout[section_base + t];
        bool dispatched = false;

        while (st->frames_left == 0 && st->next_idx < tl->first + tl->count) {
            dispatch_event(trk, st->next_idx);
            st->frames_left = pool[st->next_idx].frames;
            st->next_idx++;
            dispatched = true;
        }

        if (!dispatched && trk->ch == 11)
            perc_decay(trk->ch);

        if (st->frames_left > 0)
            st->frames_left--;
    }

    if (section_frames_left > 0)
        section_frames_left--;

    if (section_frames_left == 0) {
        u8 next = section + 1;

        if (next >= playing->section_count)
            next = playing->loop_section;

        section_enter(next);
    }
}

i32 v68_song_start(const V68Song *song) {
    v68_song_stop();

    if (song->loop_section >= song->section_count) {
        v68_puts("sound: bad loop section\n");
        return -1;
    }

    for (u8 s = 0; s < song->section_count; s++) {
        if (!parse_section(s, &song->sections[s])) {
            sound_clear();
            return -1;
        }
    }

    playing = song;
    section_enter(0);

    return 0;
}

void v68_song_stop(void) {
    if (playing && section_tracks > 0) {
        const V68Section *s = &playing->sections[section];

        for (u8 t = 0; t < s->track_count; t++) {
            u8 ch = s->tracks[t].ch;

            if (ch < 8)
                *V68_AUDIO_KEYON = ch;
            else
                V68_AUDIO_CH(ch)[0x02] = 15;
        }
    }

    sound_clear();
}

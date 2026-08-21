#include "vega68_gfx.h"

static u32 ring;
static u32 ring_mask;
static u16 tail;

static void st32(u32 off, u32 v) {
    *(volatile u32 *)(V68_TPU_RAM + off) = v;
}

static void push(u32 w) {
    st32(ring + (u32)(tail & ring_mask) * 4, w);
    tail++;
}

void v68_3d_init(u32 ring_off, u32 ring_words, u32 color_off, u32 z_off, u16 w, u16 h) {
    V68_TPU_STATE->ring_base = ring_off;
    V68_TPU_STATE->ring_words = ring_words;
    V68_TPU_STATE->color_base = color_off;
    V68_TPU_STATE->z_base = z_off;
    V68_TPU_STATE->width = w;
    V68_TPU_STATE->height = h;

    ring = ring_off;
    ring_mask = ring_words - 1;
    tail = *V68_TPU_HEAD;
}

void v68_3d_fill(u16 x0, u16 y0, u16 x1, u16 y1, u8 flags, u8 color, u16 z) {
    push((u32)V68_TPU_OP_FILL << 24 | flags);
    push((u32)x0 << 16 | y0);
    push((u32)x1 << 16 | y1);
    push(color);
    push(z);
}

void v68_3d_tri(u32 w0, u32 w1, u32 cmap_off, u32 blend_off, const V68Vert *a,
                const V68Vert *b, const V68Vert *c) {
    const V68Vert *vs[3] = { a, b, c };

    push(w0);
    push(w1);
    push(cmap_off);
    push(blend_off);

    for (i32 i = 0; i < 3; i++) {
        push((u32)vs[i]->x);
        push((u32)vs[i]->y);
        push((u32)vs[i]->z);
        push((u32)vs[i]->uq);
        push((u32)vs[i]->vq);
        push((u32)vs[i]->q);
        push((u32)vs[i]->shade);
    }
}

void v68_3d_submit(void) {
    *V68_TPU_TAIL = tail;
}

void v68_3d_wait(void) {
    while (*V68_TPU_STATUS & V68_TPU_BUSY) {}
}

void v68_3d_fb(u32 fb_off) {
    *V68_FB_BASE = fb_off;
}

void v68_3d_mode(i32 hires, i32 tpu_plane) {
    *V68_VDP_MODE = (hires ? V68_MODE_HIRES : 0) | (tpu_plane ? V68_MODE_TPU_PLANE : 0);
}

void v68_2d_sprite(i32 i, i32 x, i32 y, u16 ctrl, u16 attr) {
    volatile V68Sprite *s = &V68_SPRITES[i];

    s->x = (i16)x;
    s->y = (i16)y;
    s->ctrl = ctrl;
    s->attr = attr;
}

void v68_2d_scroll(i32 plane, u16 h, u16 v) {
    V68_SCROLL->plane[plane].h = h;
    V68_SCROLL->plane[plane].v = v;
}

void v68_2d_palette(i32 i, u32 rgb) {
    V68_PALETTE[i] = rgb;
}

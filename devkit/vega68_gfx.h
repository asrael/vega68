#ifndef V68_GFX_H
#define V68_GFX_H

#include "vega68_hw.h"

// 16.16 throughout: uq = u*q, vq = v*q and q = 1/w are premultiplied here,
// shade selects the colormap row (integer part 0..63).
typedef struct { i32 x, y, z, uq, vq, q, shade; } V68Vert;

// TRI w1: [31:12] texture offset >> 3, [11:8] log2w, [7:4] log2h, [3:0] mips
static inline u32 v68_3d_tex(u32 tex_off, u32 log2w, u32 log2h, u32 levels) {
    return (tex_off >> 3) << 12 | (log2w & 0xF) << 8 | (log2h & 0xF) << 4 | (levels & 0xF);
}

// TRI w0: opcode, s4.4 LOD bias, V68_TRI_* flags
static inline u32 v68_3d_flags(i32 lod_bias, u32 flags) {
    return (u32)V68_TPU_OP_TRI << 24 | ((u32)lod_bias & 0xFF) << 8 | (flags & 0xFF);
}

void v68_3d_init(u32 ring_off, u32 ring_words, u32 color_off, u32 z_off, u16 w, u16 h);
void v68_3d_mode(i32 hires, i32 tpu_plane);
void v68_3d_fb(u32 fb_off);
void v68_3d_tri(u32 w0, u32 w1, u32 cmap_off, u32 blend_off, const V68Vert *a,
                const V68Vert *b, const V68Vert *c);
void v68_3d_fill(u16 x0, u16 y0, u16 x1, u16 y1, u8 flags, u8 color, u16 z);
void v68_3d_submit(void);
void v68_3d_wait(void);

void v68_2d_sprite(i32 i, i32 x, i32 y, u16 ctrl, u16 attr);
void v68_2d_scroll(i32 plane, u16 h, u16 v);
void v68_2d_palette(i32 i, u32 rgb);

#endif

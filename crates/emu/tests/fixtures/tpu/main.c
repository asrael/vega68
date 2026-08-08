#include "vega68_gfx.h"

// TPU RAM layout: state block at 0, then the ring, a 64-row colormap, a
// two-level 8x8 texture, the 64K blend table and the render targets.
#define RING_OFF   0x000100
#define RING_WORDS 256
#define CMAP_OFF   0x001000
#define TEX_OFF    0x012000
#define BLEND_OFF  0x020000
#define COLOR_OFF  0x040000
#define Z_OFF      0x050000

#define W 320
#define H 180

#define FX(n) ((i32)((n) << 16))

// bias +7.9375 (s4.4): forces the minifying LOD past level 0 on a triangle
// whose uv step would otherwise sit well under one texel per pixel.
#define BIAS_MAX 127

static const V68Vert textured[3] = {
    { FX(40), FX(20), FX(0x1000), FX(0), FX(0), FX(1), FX(0) },
    { FX(200), FX(30), FX(0x1000), FX(8), FX(0), FX(1), FX(32) },
    { FX(60), FX(140), FX(0x1000), FX(0), FX(8), FX(1), FX(63) },
};

// nearer in z than the textured one, so it wins the depth test where they meet
static const V68Vert blended[3] = {
    { FX(100), FX(60), FX(0x0800), FX(0), FX(0), FX(1), FX(16) },
    { FX(260), FX(80), FX(0x0800), FX(4), FX(0), FX(1), FX(48) },
    { FX(120), FX(160), FX(0x0800), FX(0), FX(4), FX(1), FX(32) },
};

static void puthex32(u32 v) {
    static const char digits[] = "0123456789abcdef";

    for (i32 i = 28; i >= 0; i -= 4)
        *V68_DEBUG_PUTC = digits[(v >> i) & 0xF];
}

static void build_tables(void) {
    volatile u8 *tex = V68_TPU_RAM + TEX_OFF;
    volatile u8 *cmap = V68_TPU_RAM + CMAP_OFF;
    volatile u8 *blend = V68_TPU_RAM + BLEND_OFF;

    for (u32 y = 0; y < 8; y++)
        for (u32 x = 0; x < 8; x++)
            tex[y * 8 + x] = (u8)(16 + ((x + y) & 7));

    // level 1 is 4x4 and tagged out of a different band, so which mip a
    // fragment sampled is readable straight off the picture
    for (u32 y = 0; y < 4; y++)
        for (u32 x = 0; x < 4; x++)
            tex[64 + y * 4 + x] = (u8)(40 + ((x + y) & 3));

    // the shade shifts the index, so lighting is visible in the output
    for (u32 r = 0; r < 64; r++)
        for (u32 t = 0; t < 256; t++)
            cmap[r * 256 + t] = (u8)(t + (r >> 4));

    // both operands matter, and the 128 base keeps every blended pixel out of
    // the index bands the two source triangles paint with
    for (u32 d = 0; d < 256; d++)
        for (u32 s = 0; s < 256; s++)
            blend[d * 256 + s] = (u8)(128 + ((d + s) >> 2));

    for (i32 i = 0; i < V68_PALETTE_SIZE; i++)
        v68_2d_palette(i, (u32)((i * 7) & 0xFF) << 16 | (u32)((i * 13) & 0xFF) << 8 |
                              (u32)((i * 29) & 0xFF));
}

void main(void) {
    u32 tex = v68_3d_tex(TEX_OFF, 3, 3, 2);

    build_tables();

    v68_3d_init(RING_OFF, RING_WORDS, COLOR_OFF, Z_OFF, W, H);
    v68_3d_fill(0, 0, W, H, V68_FILL_COLOR | V68_FILL_Z, 1, 0xFFFF);
    v68_3d_tri(v68_3d_flags(0, 0), tex, CMAP_OFF, 0, &textured[0], &textured[1],
               &textured[2]);
    v68_3d_tri(v68_3d_flags(BIAS_MAX, V68_TRI_BLEND), tex, CMAP_OFF, BLEND_OFF, &blended[0],
               &blended[1], &blended[2]);

    v68_irq_init();
    v68_vblank_enable();

    // submit at the top of vblank: the fragment counter zeroes at every frame
    // start, so the read below has to happen in the frame that drew them
    v68_wait_vblank();
    v68_3d_submit();

    if (*V68_TPU_STATUS & V68_TPU_BUSY)
        v68_puts("busy\n");

    puthex32((u32)*V68_TPU_PIXELS_HI << 16 | *V68_TPU_PIXELS_LO);
    *V68_DEBUG_PUTC = '\n';

    v68_3d_fb(COLOR_OFF);
    v68_3d_mode(0, 1);
    v68_wait_vblank();

    v68_puts("ok\n");
    *V68_DEBUG_PUTC = 0x04;

    while (true) {}
}

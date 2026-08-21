#include "gfx.h"
#include "math.h"

#define RING_OFF   0x000100
#define RING_WORDS 2048
#define CMAP_OFF   0x004000
#define TEX_OFF    0x008000
#define FB0_OFF    0x100000
#define FB1_OFF    0x140000
#define Z_OFF      0x180000

#define W 640
#define H 360

#define ROOM   448
#define ROOM_Y 128
#define TEXW   (ROOM / 4)
#define TEXH   (ROOM_Y / 4)
#define NEAR      (16 << 16)
#define FOCAL     320
#define SPEED     (10 << 16)
#define SENS      ((3 << 16) / 8)
#define PITCH_MAX (140 << 16)
#define CLAMP     ((ROOM - 56) << 16)
#define FOG_START 224
#define FOG_END   1200
#define FOG_MUL   ((63 << 16) / (FOG_END - FOG_START))

#define FX(n) ((i32)((n) << 16))

typedef struct { i32 x, y, z, u, v; } ViewV;

static const i16 faces[6][4][5] = {
    { { -ROOM, -ROOM_Y, -ROOM, 0, 0 }, { ROOM, -ROOM_Y, -ROOM, TEXW, 0 },
      { ROOM, -ROOM_Y, ROOM, TEXW, TEXW }, { -ROOM, -ROOM_Y, ROOM, 0, TEXW } },
    { { -ROOM, ROOM_Y, -ROOM, 0, 0 }, { ROOM, ROOM_Y, -ROOM, TEXW, 0 },
      { ROOM, ROOM_Y, ROOM, TEXW, TEXW }, { -ROOM, ROOM_Y, ROOM, 0, TEXW } },
    { { -ROOM, -ROOM_Y, ROOM, 0, TEXH }, { ROOM, -ROOM_Y, ROOM, TEXW, TEXH },
      { ROOM, ROOM_Y, ROOM, TEXW, 0 }, { -ROOM, ROOM_Y, ROOM, 0, 0 } },
    { { -ROOM, -ROOM_Y, -ROOM, 0, TEXH }, { ROOM, -ROOM_Y, -ROOM, TEXW, TEXH },
      { ROOM, ROOM_Y, -ROOM, TEXW, 0 }, { -ROOM, ROOM_Y, -ROOM, 0, 0 } },
    { { ROOM, -ROOM_Y, -ROOM, 0, TEXH }, { ROOM, -ROOM_Y, ROOM, TEXW, TEXH },
      { ROOM, ROOM_Y, ROOM, TEXW, 0 }, { ROOM, ROOM_Y, -ROOM, 0, 0 } },
    { { -ROOM, -ROOM_Y, -ROOM, 0, TEXH }, { -ROOM, -ROOM_Y, ROOM, TEXW, TEXH },
      { -ROOM, ROOM_Y, ROOM, TEXW, 0 }, { -ROOM, ROOM_Y, -ROOM, 0, 0 } },
};

static i32 cam_x = 0;
static i32 cam_z = 0;
static u32 front = 0;
static i32 printed = 0;
static u32 yaw16 = 0;
static i32 pitch16 = 0;

static void build_tables(void) {
    volatile u8 *tex = V68_TPU_RAM + TEX_OFF;
    volatile u8 *cmap = V68_TPU_RAM + CMAP_OFF;
    u32 off = 0;

    for (u32 y = 0; y < 64; y++)
        for (u32 x = 0; x < 64; x++) {
            u32 mx = (x + ((y >> 4) & 1 ? 8 : 0)) & 31;
            u8 t = ((y & 15) < 2 || mx < 2) ? 220 : (u8)(140 + ((x ^ y) & 7) * 6);

            tex[y * 64 + x] = t;
        }

    for (u32 lvl = 1; lvl < 4; lvl++) {
        u32 w = 64u >> lvl;
        u32 prev = off;

        off += (w * 2) * (w * 2);

        for (u32 y = 0; y < w; y++)
            for (u32 x = 0; x < w; x++) {
                u32 a = prev + (y * 2) * (w * 2) + x * 2;
                u32 s = (u32)tex[a] + tex[a + 1] + tex[a + w * 2] + tex[a + w * 2 + 1];

                tex[off + y * w + x] = (u8)(s >> 2);
            }
    }

    for (u32 r = 0; r < 64; r++)
        for (u32 t = 0; t < 256; t++)
            cmap[r * 256 + t] = (u8)((t * (63 - r)) / 63);

    for (i32 i = 0; i < V68_PALETTE_SIZE; i++)
        v68_2d_palette(i, (u32)i << 16 | (u32)i << 8 | (u32)((i * 3) / 4 + 32));
}

static i32 clamp_slope(i32 r) {
    if (r > FX(48))
        return FX(48);

    if (r < -FX(48))
        return -FX(48);

    return r;
}

static void puthex32(u32 v) {
    static const char digits[] = "0123456789abcdef";

    for (i32 i = 28; i >= 0; i -= 4)
        *V68_DEBUG_PUTC = digits[(v >> i) & 0xF];
}

static V68Vert project(const ViewV *p) {
    V68Vert o;
    i32 xr = clamp_slope(v68_divs((i64)p->x << 16, p->z));
    i32 yr = clamp_slope(v68_divs((i64)p->y << 16, p->z));
    u32 q = (u32)v68_divs((i64)1 << 32, p->z);
    i32 shade = (i32)(((i64)(p->z - FX(FOG_START)) * FOG_MUL) >> 16);

    o.x = FX(W / 2) + xr * FOCAL;
    o.y = FX(H / 2) - yr * FOCAL;
    o.z = (i32)(((i64)p->z * 64) >> 16 << 16);
    o.uq = (i32)(((i64)p->u * q) >> 16);
    o.vq = (i32)(((i64)p->v * q) >> 16);
    o.q = (i32)q;
    o.shade = shade < 0 ? 0 : (shade > FX(63) ? FX(63) : shade);

    return o;
}

static ViewV vlerp(const ViewV *a, const ViewV *b) {
    i32 t = v68_divs((i64)(NEAR - a->z) << 16, b->z - a->z);
    ViewV o;

    o.x = a->x + (i32)(((i64)(b->x - a->x) * t) >> 16);
    o.y = a->y + (i32)(((i64)(b->y - a->y) * t) >> 16);
    o.z = NEAR;
    o.u = a->u + (i32)(((i64)(b->u - a->u) * t) >> 16);
    o.v = a->v + (i32)(((i64)(b->v - a->v) * t) >> 16);

    return o;
}

static void submit_tri(u32 tex, const ViewV *v0, const ViewV *v1, const ViewV *v2) {
    const ViewV *in[3] = { v0, v1, v2 };
    ViewV poly[4];
    V68Vert s[4];
    i32 n = 0;

    for (i32 i = 0; i < 3; i++) {
        const ViewV *a = in[i];
        const ViewV *b = in[(i + 1) % 3];

        if (a->z >= NEAR)
            poly[n++] = *a;

        if ((a->z >= NEAR) != (b->z >= NEAR))
            poly[n++] = a->z < b->z ? vlerp(a, b) : vlerp(b, a);
    }

    if (n < 3)
        return;

    for (i32 i = 0; i < n; i++)
        s[i] = project(&poly[i]);

    v68_3d_tri(v68_3d_flags(0, 0), tex, CMAP_OFF, 0, &s[0], &s[1], &s[2]);

    if (n == 4)
        v68_3d_tri(v68_3d_flags(0, 0), tex, CMAP_OFF, 0, &s[0], &s[2], &s[3]);
}

static void update(void) {
    u16 pad = *V68_PAD_1;
    i32 mdx = (i16)*V68_MOUSE_X;
    i32 mdy = (i16)*V68_MOUSE_Y;
    i32 fwd_x;
    i32 fwd_z;

    yaw16 += (u32)(mdx * SENS);
    pitch16 -= mdy * SENS;

    if (pitch16 > PITCH_MAX)
        pitch16 = PITCH_MAX;

    if (pitch16 < -PITCH_MAX)
        pitch16 = -PITCH_MAX;

    fwd_x = (i32)(((i64)v68_fsin(yaw16 >> 16) * SPEED) >> 16);
    fwd_z = (i32)(((i64)v68_fcos(yaw16 >> 16) * SPEED) >> 16);

    if (pad & V68_PAD_UP) {
        cam_x += fwd_x;
        cam_z += fwd_z;
    }

    if (pad & V68_PAD_DOWN) {
        cam_x -= fwd_x;
        cam_z -= fwd_z;
    }

    if (pad & V68_PAD_RIGHT) {
        cam_x += fwd_z;
        cam_z -= fwd_x;
    }

    if (pad & V68_PAD_LEFT) {
        cam_x -= fwd_z;
        cam_z += fwd_x;
    }

    if (cam_x > CLAMP)
        cam_x = CLAMP;

    if (cam_x < -CLAMP)
        cam_x = -CLAMP;

    if (cam_z > CLAMP)
        cam_z = CLAMP;

    if (cam_z < -CLAMP)
        cam_z = -CLAMP;
}

static void draw(u32 tex, u32 fb) {
    i32 c = v68_fcos(yaw16 >> 16);
    i32 s = v68_fsin(yaw16 >> 16);
    i32 cp = v68_fcos((u32)(pitch16 >> 16));
    i32 sp = v68_fsin((u32)(pitch16 >> 16));

    V68_TPU_STATE->color_base = fb;

    v68_3d_fill(0, 0, W, H, V68_FILL_COLOR | V68_FILL_Z, 0, 0xFFFF);

    for (i32 f = 0; f < 6; f++) {
        ViewV v[4];

        for (i32 i = 0; i < 4; i++) {
            const i16 *p = faces[f][i];
            i32 dx = (p[0] << 16) - cam_x;
            i32 dz = (p[2] << 16) - cam_z;

            i32 yv = FX(p[1]);
            i32 zv = (i32)(((i64)dx * s + (i64)dz * c) >> 16);

            v[i].x = (i32)(((i64)dx * c - (i64)dz * s) >> 16);
            v[i].y = (i32)(((i64)yv * cp - (i64)zv * sp) >> 16);
            v[i].z = (i32)(((i64)yv * sp + (i64)zv * cp) >> 16);
            v[i].u = FX(p[3]);
            v[i].v = FX(p[4]);
        }

        submit_tri(tex, &v[0], &v[1], &v[2]);
        submit_tri(tex, &v[0], &v[2], &v[3]);
    }

    v68_3d_submit();
}

void main(void) {
    u32 tex = v68_3d_tex(TEX_OFF, 6, 6, 4);

    build_tables();
    v68_3d_init(RING_OFF, RING_WORDS, FB0_OFF, Z_OFF, W, H);
    v68_mode(1, 1);
    v68_irq_init();
    v68_vblank_enable();
    v68_wait_vblank();

    while (true) {
        u32 back = front ? FB0_OFF : FB1_OFF;

        update();
        draw(tex, back);

        v68_fb(back);
        front = !front;

        if (printed < 4) {
            printed++;
            v68_puts(printed == 1 ? "frag " : "line ");
            puthex32(printed == 1
                         ? (u32)*V68_TPU_PIXELS_HI << 16 | *V68_TPU_PIXELS_LO
                         : (u32)(*V68_VDP_STATUS & V68_LINE_MASK));
            *V68_DEBUG_PUTC = '\n';
        }

        v68_wait_vblank();
    }
}

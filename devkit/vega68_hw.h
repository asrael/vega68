#ifndef V68_HW_H
#define V68_HW_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef int8_t i8;
typedef int16_t i16;
typedef int32_t i32;
typedef int64_t i64;
typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;
typedef size_t usize;
typedef ptrdiff_t isize;

#define V68_VRAM_SIZE         0x80000
#define V68_TILE_COUNT        4096
#define V68_TILEMAP_COLS      128
#define V68_TILEMAP_ROWS      128
#define V68_TILEMAP_CELLS     (V68_TILEMAP_COLS * V68_TILEMAP_ROWS)
#define V68_TILEMAP_PLANES    4
#define V68_SPRITE_COUNT      128
#define V68_PALETTE_SIZE      256
#define V68_BRIGHTNESS_LEVELS 256
#define V68_TPU_RAM_SIZE      0x400000

typedef struct { i16 x, y; u16 ctrl, attr; } V68Sprite;
typedef struct { u8 px[8][8]; } V68Tile;
typedef struct { u16 cell[V68_TILEMAP_ROWS][V68_TILEMAP_COLS]; } V68Tilemap;

typedef struct {
    struct { u16 h, v; } plane[4];
    u16 line_h[4][180];
    u16 col_v[4][40];
} V68Scroll;

typedef struct {
    u32 ring_base;
    u32 ring_words;
    u32 color_base;
    u32 z_base;
    u16 width;
    u16 height;
} V68TpuState;

#define V68_VRAM      ((volatile u8 *)0x03000000)
#define V68_TILES     ((volatile V68Tile *)0x03000000)
#define V68_TILEMAPS  ((volatile V68Tilemap *)0x03040000)
#define V68_SPRITES   ((volatile V68Sprite *)0x03060000)
#define V68_SCROLL    ((volatile V68Scroll *)0x03061000)
#define V68_VDP_MODE  ((volatile u16 *)0x03061800)
#define V68_FB_BASE   ((volatile u32 *)0x03061804)
#define V68_PALETTE   ((volatile u32 *)0x03080000)
#define V68_TPU_RAM   ((volatile u8 *)0x04000000)
#define V68_TPU_STATE ((volatile V68TpuState *)V68_TPU_RAM)

#define V68_VDP_STATUS    ((volatile u16 *)0xFF000000)
#define V68_IRQ_ENABLE    ((volatile u16 *)0xFF000004)
#define V68_IRQ_ACK       ((volatile u16 *)0xFF000008)
#define V68_LINE_COMPARE  ((volatile u16 *)0xFF00000C)
#define V68_BRIGHTNESS    ((volatile u16 *)0xFF000010)
#define V68_LINE_INTERVAL ((volatile u16 *)0xFF000014)
#define V68_MOUSE_X       ((volatile u16 *)0xFF000108)
#define V68_MOUSE_Y       ((volatile u16 *)0xFF00010C)
#define V68_PAD_1         ((volatile u16 *)0xFF000100)
#define V68_PAD_2         ((volatile u16 *)0xFF000104)
#define V68_DEBUG_PUTC    ((volatile u16 *)0xFF000200)
#define V68_RESET_REASON  ((volatile u16 *)0xFF000300)

#define V68_AUDIO_CH(n)  ((volatile u8 *)(0xFF000400 + (n) * 0x40))
#define V68_AUDIO_KEYON  ((volatile u8 *)0xFF000800)
#define V68_AUDIO_LFO    ((volatile u8 *)0xFF000801)
#define V68_AUDIO_STATUS ((volatile u16 *)0xFF000802)
#define V68_AUDIO_ESEND  ((volatile u16 *)0xFF000810)
#define V68_AUDIO_EDELAY ((volatile u8 *)0xFF000812)
#define V68_AUDIO_EFB    ((volatile u8 *)0xFF000813)
#define V68_AUDIO_EVOL_L ((volatile u8 *)0xFF000814)
#define V68_AUDIO_EVOL_R ((volatile u8 *)0xFF000815)
#define V68_AUDIO_EFIR   ((volatile u8 *)0xFF000816)

#define V68_TPU_TAIL  ((volatile u16 *)0xFF000A00)
#define V68_TPU_STATUS    ((volatile u16 *)0xFF000A04)
#define V68_TPU_HEAD      ((volatile u16 *)0xFF000A08)
#define V68_TPU_PIXELS_LO ((volatile u16 *)0xFF000A0C)
#define V68_TPU_PIXELS_HI ((volatile u16 *)0xFF000A10)

#define V68_VBLANK    0x8000
#define V68_LINE_MASK 0x00FF

#define V68_IRQ_VBLANK 0x0001
#define V68_IRQ_LINE   0x0002

#define V68_RESET_COLD   0
#define V68_RESET_WARM   1
#define V68_RESET_RELOAD 2

#define V68_MODE_HIRES     0x0001
#define V68_MODE_TPU_PLANE 0x0002

#define V68_PAD_UP     0x0001
#define V68_PAD_DOWN   0x0002
#define V68_PAD_LEFT   0x0004
#define V68_PAD_RIGHT  0x0008
#define V68_PAD_A      0x0010
#define V68_PAD_B      0x0020
#define V68_PAD_X      0x0040
#define V68_PAD_Y      0x0080
#define V68_PAD_START  0x0100
#define V68_PAD_SELECT 0x0200
#define V68_PAD_L      0x0400
#define V68_PAD_R      0x0800

#define V68_TPU_BUSY 0x8000

#define V68_TPU_OP_TRI     0x01
#define V68_TPU_OP_FILL    0x02
#define V68_TPU_TRI_WORDS  25
#define V68_TPU_FILL_WORDS 5

#define V68_TRI_BLEND      0x01
#define V68_TRI_ZGREATER   0x02
#define V68_TRI_ZTEST_OFF  0x04
#define V68_TRI_ZWRITE_OFF 0x08

#define V68_FILL_COLOR 0x01
#define V68_FILL_Z     0x02

#ifdef __clang__
#ifdef V68_LSP
#define V68_INTERRUPT __attribute__((unused))
#else
#error "clang cannot build vega68 interrupt handlers (no interrupt_handler attribute)"
#endif
#else
#define V68_INTERRUPT __attribute__((interrupt_handler, unused))
#endif

static inline volatile u32 *v68_vec(u32 a) {
    volatile u32 *p;
    __asm__("" : "=r"(p) : "0"(a));
    return p;
}

#define V68_VEC_LINE   (*v68_vec(0x70))
#define V68_VEC_VBLANK (*v68_vec(0x78))

void __attribute__((noreturn)) v68_reset(void);

__attribute__((weak)) volatile bool v68_frame_ready = false;
__attribute__((weak)) void v68_hblank_hook(u16 line);
__attribute__((weak)) void v68_vblank_hook(void);
__attribute__((weak)) void v68_sound_tick(void);

static inline void v68_irq_init(void) {
    __asm__ volatile("move.w #0x2000, %%sr" ::: "cc");
}

static inline void v68_puts(const char *s) {
    while (s && *s)
        *V68_DEBUG_PUTC = *s++;
}

V68_INTERRUPT static void v68_hblank_isr(void) {
    if (v68_hblank_hook)
        v68_hblank_hook(*V68_VDP_STATUS & V68_LINE_MASK);

    *V68_IRQ_ACK = V68_IRQ_LINE;
}

static inline void v68_hblank_enable(u16 first, u16 every) {
    V68_VEC_LINE       = (u32)v68_hblank_isr;
    *V68_LINE_COMPARE  = first;
    *V68_LINE_INTERVAL = every;
    *V68_IRQ_ENABLE   |= V68_IRQ_LINE;
}

static inline void v68_wait_vblank(void) {
    while (!v68_frame_ready) {}
    v68_frame_ready = false;

    if (v68_sound_tick)
        v68_sound_tick();
}

V68_INTERRUPT static void v68_vblank_isr(void) {
    if (v68_vblank_hook)
        v68_vblank_hook();

    v68_frame_ready = true;
    *V68_IRQ_ACK = V68_IRQ_VBLANK;
}

static inline void v68_vblank_enable(void) {
    V68_VEC_VBLANK = (u32)v68_vblank_isr;
    *V68_IRQ_ENABLE |= V68_IRQ_VBLANK;
}

#endif

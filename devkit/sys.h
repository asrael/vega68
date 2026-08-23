#ifndef V68_SYS_H
#define V68_SYS_H

#include "types.h"

#define V68_VDP_STATUS    ((volatile u16 *)0xFF000000)
#define V68_IRQ_ENABLE    ((volatile u16 *)0xFF000004)
#define V68_IRQ_ACK       ((volatile u16 *)0xFF000008)
#define V68_LINE_COMPARE  ((volatile u16 *)0xFF00000C)
#define V68_LINE_INTERVAL ((volatile u16 *)0xFF000014)
#define V68_MOUSE_X       ((volatile u16 *)0xFF000108)
#define V68_MOUSE_Y       ((volatile u16 *)0xFF00010C)
#define V68_MOUSE_BTN     ((volatile u16 *)0xFF000110)
#define V68_PAD_1         ((volatile u16 *)0xFF000100)
#define V68_PAD_2         ((volatile u16 *)0xFF000104)
#define V68_DEBUG_PUTC    ((volatile u16 *)0xFF000200)
#define V68_RESET_REASON  ((volatile u16 *)0xFF000300)

#define V68_VBLANK    0x8000
#define V68_LINE_MASK 0x00FF

#define V68_IRQ_VBLANK 0x0001
#define V68_IRQ_LINE   0x0002

#define V68_RESET_COLD   0
#define V68_RESET_WARM   1
#define V68_RESET_RELOAD 2

#define V68_MOUSE_L 0x0001
#define V68_MOUSE_R 0x0002

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

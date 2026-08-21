#include "bios.h"
#include "sys.h"

#define V68_CART  ((volatile u8 *)0x01000000)
#define V68_MAGIC 0x56363800

void __attribute__((noreturn)) v68_reset(void) {
    *V68_IRQ_ENABLE = 0;
    *V68_IRQ_ACK = V68_IRQ_VBLANK | V68_IRQ_LINE;

    V68_VEC_VBLANK = (u32)v68_rte_stub;
    V68_VEC_LINE = (u32)v68_rte_stub;

    __asm__ volatile("move.w #0x2700, %%sr" ::: "cc");

    *V68_RESET_REASON = V68_RESET_WARM;

    __asm__ volatile("move.l #__stack_top, %sp\n\tjmp _start");
    __builtin_unreachable();
}

void main(void) {
    const volatile u32 *header = (const volatile u32 *)V68_CART;

    if (header[0] != V68_MAGIC || header[1] != 0) v68_monitor("vega68: no cart\n");

    u32 entry = header[2];

    if (!entry) v68_monitor("vega68: no cart\n");

    ((void (*)(void))entry)();

    *V68_DEBUG_PUTC = 0x04;
    v68_monitor("vega68: cart returned\n");
}

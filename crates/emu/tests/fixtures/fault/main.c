#include "vega68_hw.h"

void v68_vblank_hook(void) {
    *V68_DEBUG_PUTC = '!';
}

void main(void) {
    v68_irq_init();
    v68_vblank_enable();
    v68_puts("pre\n");

    __asm__ volatile(".word 0x4afc");

    v68_puts("post\n");
}

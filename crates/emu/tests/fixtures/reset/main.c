#include "vega68_hw.h"

V68_INTERRUPT static void on_line(void) {
    *V68_IRQ_ACK = V68_IRQ_LINE;
}

void main(void) {
    u8 n = V68_VRAM[0];

    V68_VRAM[0] = n + 1;
    v68_puts("boot\n");

    if (n < 2) {
        v68_init();
        V68_VEC_LINE = (u32)on_line;
        v68_reset();
    }

    v68_puts("ok\n");
    *V68_DEBUG_PUTC = 0x04;
}

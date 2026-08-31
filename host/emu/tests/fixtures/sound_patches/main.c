#include "afx.h"

static void putdec(u32 v) {
    if (v >= 10)
        *V68_DEBUG_PUTC = (char)('0' + v / 10);
    *V68_DEBUG_PUTC = (char)('0' + v % 10);
}

static void puthex(u8 v) {
    static const char digits[] = "0123456789abcdef";

    *V68_DEBUG_PUTC = digits[v >> 4];
    *V68_DEBUG_PUTC = digits[v & 0x0F];
}

void main(void) {
    v68_irq_init();
    v68_vblank_enable();

    for (u8 i = 0; i < 12; i++) {
        const V68_Patch *p = &v68_patches[i];

        v68_fm_patch(0, p);
        V68_AUDIO_CH(0)[0x1C] = (4 << 3) | (1083 >> 8);
        V68_AUDIO_CH(0)[0x1D] = 1083 & 0xFF;
        *V68_AUDIO_KEYON = 0xF0;

        bool on = false;
        for (u8 f = 0; f < 8; f++) {
            v68_wait_vblank();
            if (*V68_AUDIO_STATUS & 0x0001)
                on = true;
        }

        v68_puts("p");
        putdec(i);
        *V68_DEBUG_PUTC = ' ';
        puthex(p->op[0][0]);
        *V68_DEBUG_PUTC = ' ';
        puthex(p->op[1][0]);
        *V68_DEBUG_PUTC = ' ';
        puthex(p->op[2][0]);
        *V68_DEBUG_PUTC = ' ';
        puthex(p->op[3][0]);
        *V68_DEBUG_PUTC = ' ';
        puthex(p->fb_alg);
        if (on)
            v68_puts(" on");
        v68_puts("\n");

        *V68_AUDIO_KEYON = 0x00;
        for (u8 f = 0; f < 30; f++)
            v68_wait_vblank();
    }

    v68_puts("ok\n");
    *V68_DEBUG_PUTC = 0x04;
    while (true) {}
}

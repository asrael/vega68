#include "vega68_hw.h"

void main(void) {
    volatile u8 *ch = V68_AUDIO_CH(0);

    ch[0x00] = 0x01;
    ch[0x02] = 0x1F;
    ch[0x08] = 127;
    ch[0x0F] = 127;
    ch[0x16] = 127;
    ch[0x1E] = 0x07;
    ch[0x1C] = (4 << 3) | (1083 >> 8);
    ch[0x1D] = 1083 & 0xFF;

    volatile u8 *sq = V68_AUDIO_CH(8);

    sq[0x01] = 0xFE;
    sq[0x02] = 4;

    *V68_AUDIO_KEYON = 0xF0;

    for (u32 tries = 0; tries < 10000; tries++) {
        if ((*V68_AUDIO_STATUS & 0x0101) == 0x0101)
            break;
    }

    if ((*V68_AUDIO_STATUS & 0x0101) == 0x0101)
        v68_puts("ok\n");

    *V68_DEBUG_PUTC = 0x04;

    while (true) {}
}

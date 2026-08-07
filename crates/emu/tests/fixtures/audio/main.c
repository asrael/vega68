#include "vega68_hw.h"

// one sine carrier at A440 (alg 7, ops 2-4 muted) + one square
void main(void) {
    volatile u8 *ch = V68_AUDIO_CH(0);

    ch[0x00] = 0x01; // op 1: MUL 1
    ch[0x02] = 0x1F; // AR max
    ch[0x08] = 127;  // op 2 TL: silent
    ch[0x0F] = 127;  // op 3
    ch[0x16] = 127;  // op 4
    ch[0x1E] = 0x07; // alg 7
    ch[0x1C] = (4 << 3) | (1083 >> 8);
    ch[0x1D] = 1083 & 0xFF;

    volatile u8 *sq = V68_AUDIO_CH(8);

    sq[0x01] = 0xFE; // period 254
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

#include "v68.h"

#define FIRE_COUNT 4
#define FIRST_LINE 5
#define LAST_LINE  6

void v68_hblank_hook(u16 line) {
    if (V68_VRAM[FIRE_COUNT] == 0)
        V68_VRAM[FIRST_LINE] = (u8)line;

    V68_VRAM[LAST_LINE] = (u8)line;
    V68_VRAM[FIRE_COUNT]++;
}

void main(void) {
    v68_irq_init();
    v68_hblank_enable(40, 2);
    v68_vblank_enable();

    v68_wait_vblank();
    V68_VRAM[FIRE_COUNT] = 0;
    V68_VRAM[FIRST_LINE] = 0;
    V68_VRAM[LAST_LINE] = 0;
    v68_wait_vblank();
    *V68_IRQ_ENABLE = V68_IRQ_VBLANK;

    v68_hblank_enable(250, 2);
    v68_wait_vblank();

    v68_puts("ok\n");
    *V68_DEBUG_PUTC = 0x04;
}

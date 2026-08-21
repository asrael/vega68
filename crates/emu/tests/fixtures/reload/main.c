#include "v68.h"

static u32 sentinel __attribute__((section(".noinit")));

void main(void) {
    if (*V68_RESET_REASON == V68_RESET_COLD) {
        sentinel = 0xC0FFEE;
        v68_puts("cold\n");
    } else {
        v68_puts(sentinel == 0xC0FFEE ? "reload ok\n" : "reload bad\n");
    }

    *V68_DEBUG_PUTC = 0x04;
}

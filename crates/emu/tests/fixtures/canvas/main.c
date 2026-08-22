#include "v68.h"

void main(void) {
    v68_palette(1, 0x00FF0000);
    v68_palette(2, 0x0000FF00);
    v68_palette(3, 0x000000FF);
    v68_palette(4, 0x00FFFF00);

    v68_canvas(0, 1);

    *v68_pixel(0, 0) = 1;
    *v68_pixel(319, 179) = 2;

    for (i32 i = 0; i < 32; i++) {
        *v68_pixel(4 + i, 4 + i) = 3;
    }

    v68_fill(100, 50, 30, 20, 4);

    if (*v68_pixel(0, 0) != 1 || *v68_pixel(319, 179) != 2 ||
        *v68_pixel(20, 20) != 3 || *v68_pixel(1, 0) != 0 ||
        *v68_pixel(100, 50) != 4 || *v68_pixel(129, 69) != 4) {
        v68_puts("bad\n");
    } else {
        v68_puts("ok\n");
    }

    *V68_DEBUG_PUTC = 0x04;
}

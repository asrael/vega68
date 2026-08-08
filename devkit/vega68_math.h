#ifndef V68_MATH_H
#define V68_MATH_H

#include "vega68_hw.h"

static inline i32 v68_divs(i64 n, i32 d) {
    i32 hi = (i32)(n >> 32);
    i32 lo = (i32)n;

    __asm__("divs.l %2,%0:%1" : "+d"(hi), "+d"(lo) : "d"(d) : "cc");

    return lo;
}

// a in 0..1023 angle units, 16.16 out
static inline i32 v68_fsin(u32 a) {
    i32 half = (i32)(a & 511);
    i32 y = half * (512 - half);

    return (a & 512) ? -y : y;
}

static inline i32 v68_fcos(u32 a) {
    return v68_fsin(a + 256);
}

#endif

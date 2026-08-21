#include "bios.h"
#include "gfx.h"

#define DEBOUNCE 20000
#define WINDOW   16

static const char hex[] = "0123456789abcdef";

static u32 monitor_stack[128];
static const char *monitor_msg;
static u16 pad_prev;
static const u32 presets[] = {0x02000000, V68_CART_RAM, (u32)V68_VRAM, 0x01000000};

static void puthex(u32 v) {
    for (i32 i = 28; i >= 0; i -= 4)
        *V68_DEBUG_PUTC = hex[(v >> i) & 0xF];
}

static void dump16(u32 addr) {
    *V68_DEBUG_PUTC = '\n';
    puthex(addr);
    *V68_DEBUG_PUTC = ':';

    for (i32 i = 0; i < WINDOW; i++) {
        u8 b = *(const volatile u8 *)(addr + (u32)i);

        *V68_DEBUG_PUTC = ' ';
        *V68_DEBUG_PUTC = hex[b >> 4];
        *V68_DEBUG_PUTC = hex[b & 0xF];
    }
}

static u32 edge(void) {
    u16 now = *V68_PAD_1;
    u16 pressed = now & ~pad_prev;

    pad_prev = now;

    for (volatile i32 i = 0; i < DEBOUNCE; i++) {}

    return pressed;
}

static void __attribute__((noreturn)) monitor_loop(void) {
    u32 cursor = presets[0];
    u32 preset = 0;

    pad_prev = *V68_PAD_1;
    v68_puts(monitor_msg);

    while (true) {
        u32 pressed = edge();

        if (pressed & V68_PAD_UP) dump16(cursor -= WINDOW);
        if (pressed & V68_PAD_DOWN) dump16(cursor += WINDOW);
        if (pressed & V68_PAD_LEFT) dump16(cursor -= WINDOW * WINDOW);
        if (pressed & V68_PAD_RIGHT) dump16(cursor += WINDOW * WINDOW);

        if (pressed & (V68_PAD_L | V68_PAD_R)) {
            preset = (preset + 1) % V68_LEN(presets);
            dump16(cursor = presets[preset]);
        }

        if (pressed & V68_PAD_A)
            for (i32 i = 0; i < WINDOW; i++)
                dump16(cursor + (u32)i * WINDOW);

        if (pressed & V68_PAD_START) v68_reset();
    }
}

u32 v68_fault_regs[17];
u32 *const v68_monitor_sp = monitor_stack + V68_LEN(monitor_stack);

void v68_fault_dump(void) {
    const volatile u16 *frame = (const volatile u16 *)v68_fault_regs[16];

    v68_puts("\nfault sr=");
    puthex(frame[0]);
    v68_puts(" pc=");
    puthex((u32)frame[1] << 16 | frame[2]);
    v68_puts(" fv=");
    puthex(frame[3]);

    for (usize i = 0; i < V68_LEN(v68_fault_regs) - 1; i++) {
        v68_puts(i % 4 == 0 ? "\n" : " ");
        puthex(v68_fault_regs[i]);
    }

    *V68_DEBUG_PUTC = '\n';
    v68_monitor(0);
}

void v68_monitor(const char *msg) {
    monitor_msg = msg;

    __asm__ volatile("move.w #0x2700, %%sr\n\tmove.l %0, %%sp\n\tjsr (%1)" ::"a"(v68_monitor_sp),
                     "a"(monitor_loop)
                     : "cc", "memory");

    __builtin_unreachable();
}

const hw = @import("hw");
const memmap = hw.memmap;

comptime {
    _ = @import("vectors.zig");
}

const USER_STACK_TOP: u32 = memmap.WORK_RAM_BASE + 0x0020_0000;

fn dropToUser(entry: u32) noreturn {
    asm volatile (
        \\ move.l %[ustk], %%a0
        \\ .short 0x4E60
        \\ move.l %[entry], -(%%sp)
        \\ move.w #0, -(%%sp)
        \\ rte
        :
        : [ustk] "r" (USER_STACK_TOP),
          [entry] "r" (entry),
        : .{ .memory = true });
    unreachable;
}

pub export fn _start() callconv(.c) noreturn {
    const hdr: *const hw.CartHeader = @ptrFromInt(memmap.CART_BASE);
    if (hdr.magic != hw.CART_MAGIC) while (true) {};
    if (hdr.abi_version != hw.syscall.ABI_VERSION) while (true) {};
    dropToUser(hdr.entry);
}

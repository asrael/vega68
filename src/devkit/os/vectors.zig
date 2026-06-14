//! bios vector table: reset ssp, reset pc, trap #0

const os = @import("os.zig");
const syscall = @import("syscall.zig");
const memmap = @import("hw").memmap;

const VectorTable = extern struct {
    initial_sp: u32, // 0x00 (supervisor stack top)
    reset_pc: *const fn () callconv(.c) noreturn, // 0x04
    _pad: [30]u32, // vectors 2..31 (30 × 4B = 0x78); next field at 0x08+0x78 = 0x80
    trap0: *const fn () callconv(.naked) void, // 0x80 = vector 32 (TRAP #0)
};

export const boot_vectors linksection(".vectors") = VectorTable{
    .initial_sp = memmap.WORK_RAM_BASE + 0x0010_0000, // $80100000 kernel ssp
    .reset_pc = &os._start,
    ._pad = @splat(0),
    .trap0 = &syscall.handler,
};

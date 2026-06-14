const std = @import("std");
const hw = @import("hw");

pub fn handler() callconv(.naked) void {
    asm volatile (std.fmt.comptimePrint(
            \\ cmpi.l #{[write]d}, %%d0      // SYS_write?
            \\ beq .Lwrite
            \\ cmpi.l #{[exit]d}, %%d0       // SYS_exit?
            \\ beq .Lexit
            \\ cmpi.l #{[version]d}, %%d0    // SYS_version?
            \\ beq .Lversion
            \\ move.l #0x{[badcall]x}, %%d0  // unknown call -> error
            \\ rte
            \\.Lversion:
            \\ move.l #{[abiver]d}, %%d0      // D0 = abi_version
            \\ rte
            \\.Lwrite:
            \\ cmpi.l #{[fd]d}, %%d1         // fd == console?
            \\ bne .Lbadfd
            \\ move.l #0, %%d0               // D0 = bytes-written counter
            \\ move.l #0x{[console]x}, %%a1  // console_reg
            \\.Lwloop:
            \\ cmpi.l #0, %%d2
            \\ beq .Ldone
            \\ move.b (%%a0)+, (%%a1)
            \\ .short 0x5280                 // addq.l #1,%d0 (LLVM m68k asm rejects addq.l)
            \\ .short 0x5382                 // subq.l #1,%d2 (   "        "        subq.l)
            \\ bra .Lwloop
            \\.Ldone:
            \\ rte                           // D0 = bytes written
            \\.Lbadfd:
            \\ move.l #0x{[badfd]x}, %%d0     // unknown fd -> error
            \\ rte
            \\.Lexit:
            \\ move.l #0x{[exitreg]x}, %%a1   // exit_reg
            \\ move.l %%d1, (%%a1)
            \\ rte
        , .{
            .write = @intFromEnum(hw.Syscall.write),
            .exit = @intFromEnum(hw.Syscall.exit),
            .version = @intFromEnum(hw.Syscall.version),
            .abiver = hw.syscall.ABI_VERSION,
            .fd = hw.syscall.FD_CONSOLE,
            .badcall = hw.syscall.Err.badcall,
            .badfd = hw.syscall.Err.badfd,
            .console = hw.memmap.CONSOLE_REG,
            .exitreg = hw.memmap.EXIT_REG,
        }) ::: .{ .memory = true });
}

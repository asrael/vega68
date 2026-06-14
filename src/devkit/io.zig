const abi = @import("abi.zig");

fn syscall(number: abi.Syscall, arg1: u32, arg2: u32, ptr: u32) u32 {
    return asm volatile ("trap #0"
        : [ret] "={d0}" (-> u32),
        : [num] "{d0}" (@intFromEnum(number)),
          [a1] "{d1}" (arg1),
          [a2] "{d2}" (arg2),
          [p] "{a0}" (ptr),
        : .{ .a1 = true, .memory = true });
}

pub fn exit(status: u32) noreturn {
    _ = syscall(.exit, status, 0, 0);
    unreachable;
}

pub fn version() u16 {
    return @truncate(syscall(.version, 0, 0, 0));
}

pub fn write(fd: u32, bytes: []const u8) usize {
    return syscall(.write, fd, bytes.len, @intFromPtr(bytes.ptr));
}

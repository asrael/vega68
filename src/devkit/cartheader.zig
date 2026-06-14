const hw = @import("hw");

extern fn _start() callconv(.c) noreturn;

const Emit = extern struct {
    magic: u32,
    entry: *const fn () callconv(.c) noreturn,
    size: u32,
    abi_version: u16,
    title: [32]u8,
    reserved: [18]u8,
};

fn title(comptime s: []const u8) [32]u8 {
    var t: [32]u8 = @splat(0);
    @memcpy(t[0..s.len], s);
    return t;
}

export const cart_header linksection(".cartheader") = Emit{
    .magic = hw.CART_MAGIC,
    .entry = &_start,
    .size = 0,
    .abi_version = hw.syscall.ABI_VERSION,
    .title = title("hello"),
    .reserved = @splat(0),
};

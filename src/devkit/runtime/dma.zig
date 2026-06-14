const abi = @import("../abi.zig");

pub const Target = enum { bg, fg, sprites };

pub fn loadTiles(src: []const u8, target: Target) void {
    _ = src;
    _ = target;
}

pub fn flush() void {
    _ = abi.PPU;
}

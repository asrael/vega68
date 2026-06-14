const abi = @import("../abi.zig");

pub fn commit() void {
    _ = abi.PPU;
}

pub fn setX(layer: u8, x: i16) void {
    _ = layer;
    _ = x;
}

pub fn setY(layer: u8, y: i16) void {
    _ = layer;
    _ = y;
}

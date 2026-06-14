const abi = @import("../abi.zig");

pub var shadow: [128]abi.Sprite = @splat(.{});

pub fn commit() void {
    for (shadow, 0..) |s, i| abi.SPRITES[i] = s;
}

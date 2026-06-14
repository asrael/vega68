const std = @import("std");

const W: usize = 320;
const H: usize = 240;

const bars = [8]u32{ 0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF, 0x000000 };

pub fn fill(fb: []u32, frame: u32) void {
    std.debug.assert(fb.len >= W * H);

    var y: usize = 0;
    while (y < H) : (y += 1) {
        var x: usize = 0;
        while (x < W) : (x += 1) fb[y * W + x] = bars[(x / 40) % 8];
    }

    var x: usize = 0;
    while (x < W) : (x += 1) {
        fb[x] = 0xFFFFFF; // top row
        fb[(H - 1) * W + x] = 0xFFFFFF; // bottom row
    }
    y = 0;
    while (y < H) : (y += 1) {
        fb[y * W] = 0xFFFFFF; // left column
        fb[y * W + (W - 1)] = 0xFFFFFF; // right column
    }

    fillRect(fb, 0, 0, 0xFF0000); // top-left red
    fillRect(fb, W - 8, 0, 0x00FF00); // top-right green
    fillRect(fb, 0, H - 8, 0x0000FF); // bottom-left blue
    fillRect(fb, W - 8, H - 8, 0xFFFFFF); // bottom-right white

    const sx: usize = @intCast(frame % W);
    y = 0;
    while (y < H) : (y += 1) fb[y * W + sx] ^= 0xFFFFFF;
}

fn fillRect(fb: []u32, x0: usize, y0: usize, color: u32) void {
    var y = y0;
    while (y < y0 + 8) : (y += 1) {
        var x = x0;
        while (x < x0 + 8) : (x += 1) fb[y * W + x] = color;
    }
}

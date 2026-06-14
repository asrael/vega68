//! hardware sprites
//! 128 on-screen, variable 8×8–32×32, drawn in idx order

const palette = @import("palette.zig");

pub const Sprite = @import("hw").Sprite;

pub fn drawScanline(oam: []const Sprite, cram: []const u16, line: u16, out: []u32) void {
    const li: i32 = @intCast(line);
    const width: i32 = @intCast(out.len);

    for (oam) |s| {
        if (s.enable == 0) continue;
        const top: i32 = @intCast(s.y);
        const height = (@as(i32, s.h) + 1) * 8;
        if (li < top or li >= top + height) continue;

        const color = palette.toHost(cram[@as(usize, s.palette) * 16 + 1]);
        const left: i32 = @intCast(s.x);
        const right = left + (@as(i32, s.w) + 1) * 8;
        var px = if (left < 0) 0 else left;
        while (px < right and px < width) : (px += 1) out[@intCast(px)] = color;
    }
}

//! pixel processing unit
//!
//! - 2 scrolling BG layers, 128 variable-size sprites, 4bpp tiles, 256-color cram
//! - tile vram size: 64 kb

const hw = @import("hw");
const memmap = hw.memmap;

pub const tiles = @import("tiles.zig");
pub const sprites = @import("sprites.zig");
pub const background = @import("background.zig");
pub const palette = @import("palette.zig");

pub const PPU = struct {
    cram: [256]u16 = @splat(0),
    layers: [2]background.Layer = @splat(.{}),
    oam: [128]sprites.Sprite = @splat(.{}),
    regs: hw.PPURegs = .{},
    tile_vram: [memmap.TILE_VRAM_SIZE]u8 = @splat(0),

    pub fn renderScanline(self: *PPU, line: u16, out: []u32) void {
        const backdrop = palette.toHost(self.cram[0]);
        for (out) |*px| px.* = backdrop;
        sprites.drawScanline(&self.oam, &self.cram, line, out);
    }
};

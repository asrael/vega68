//! background planes
//!  2 layers, each a 64×32-tile map (512×256 px), scrollable; priority per-layer

const tiles = @import("tiles.zig");

pub const Layer = struct {
    enabled: bool = false,
    hscroll_table: u24 = 0,
    scroll_x: u16 = 0,
    scroll_y: u16 = 0,
};

/// Render layer `layer` for scanline `line` into the scanline buffer.
pub fn drawScanline(layer: *const Layer, vram: []const u8, line: u16, out: []u32) void {
    _ = layer;
    _ = vram;
    _ = line;
    _ = out;

    @panic("TODO: fetch tilemap row, apply scroll + per-line H-scroll, blit");
}

comptime {
    _ = tiles.TilemapEntry;
}

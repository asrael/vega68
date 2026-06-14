//! tile patterns + tilemaps (patterns 8×8 px, 4bpp (16 colors), 32 bytes each)

/// One tilemap cell (16-bit).
pub const TilemapEntry = @import("hw").TilemapEntry;

/// Decode one 8×8 tile from VRAM into 64 palette indices (0–15 each).
/// 8bpp upgrade swaps the body here only.
pub fn decodeTile(vram: []const u8, index: u16) [64]u8 {
    _ = vram;
    _ = index;

    @panic("TODO: unpack 4bpp -> 64 indices");
}

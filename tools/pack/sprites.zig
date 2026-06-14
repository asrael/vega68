//! emit sprite sheets and attribute-table-ready metadata from an indexed export

pub fn packSprites(allocator: anytype, pixels: []const u8, width: u32, height: u32) ![]u8 {
    _ = allocator;
    _ = pixels;
    _ = width;
    _ = height;

    @panic("TODO: pack sprite cells (8×8–32×32) into 4bpp tiles");
}

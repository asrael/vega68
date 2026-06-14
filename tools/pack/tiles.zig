//! pack decoded image data into 4bpp 8×8 tiles + tilemap entries

pub fn packTiles(allocator: anytype, pixels: []const u8, width: u32, height: u32) ![]u8 {
    _ = allocator;
    _ = pixels;
    _ = width;
    _ = height;

    @panic("TODO: emit 32-byte 4bpp tiles + dedupe identical tiles");
}

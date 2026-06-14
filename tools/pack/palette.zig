//! emit a cram blob (256 × 15-bit RGB) from an indexed palette

/// Convert host 8-bit-per-channel RGB triples into packed 15-bit CRAM entries.
pub fn packCRAM(allocator: anytype, rgb: []const [3]u8) ![]u16 {
    _ = allocator;
    _ = rgb;

    @panic("TODO: quantize 8→5 bits/channel, lay out 16 banks × 16 colors");
}

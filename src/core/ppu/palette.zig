//! color ram
//!  master space 15-bit RGB (32,768 colors)
//!  256 entries = 16 sub-palettes × 16 colors
//!  idx 0 of each sub-palette is transparent
//!
//!  cram entry (16-bit)
//!  [15] reserved (future shadow/highlight) [14:10] R * [9:5] G * [4:0] B

pub const Color = @import("hw").Color;

/// Expand one 15-bit CRAM entry to a host 0x00RRGGBB pixel.
/// 5→8 bits per channel.
pub fn toHost(entry: u16) u32 {
    const e: Color = @bitCast(entry);
    const r: u32 = expand5(e.r);
    const g: u32 = expand5(e.g);
    const b: u32 = expand5(e.b);

    return (r << 16) | (g << 8) | b;
}

/// 5-bit → 8-bit channel expansion (bit-replication keeps white at full scale).
fn expand5(c: u5) u32 {
    return (c << 3) | (c >> 2);
}

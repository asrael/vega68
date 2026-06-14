//! vega68-pack (build-time asset converter)
//!
//!  - one 16-color bank per object
//!  - emits packed 4bpp tiles, tilemaps, sprite sheets, and color ram
//!  - ingests an ase/png export (indexed mode, native 8×8 tilemap layers)
//!  - validates the per-cell palette constraint

const std = @import("std");

pub const hw = @import("hw");
pub const palette = @import("palette.zig");
pub const sprites = @import("sprites.zig");
pub const tiles = @import("tiles.zig");
pub const validate = @import("validate.zig");

pub fn main() void {
    std.debug.print(
        \\vega68-pack
        \\
        \\usage: vega68-pack <input.png|.aseprite> <out-dir>
        \\  emits: tiles.bin, tilemap.bin, sprites.bin, cram.bin
    , .{});
}

test {
    std.testing.refAllDecls(@This());
}

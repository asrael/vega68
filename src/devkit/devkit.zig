//! vega68 devkit

comptime {
    _ = @import("crt0.zig");
    _ = @import("cartheader.zig");
    _ = @import("runtime/mem.zig");
}

pub const abi = @import("abi.zig");
pub const console = @import("console.zig");
pub const dma = @import("runtime/dma.zig");
pub const input = @import("runtime/input.zig");
pub const io = @import("io.zig");
pub const math = @import("runtime/math/math.zig");
pub const music = @import("runtime/music.zig");
pub const palette = @import("runtime/palette.zig");
pub const scroll = @import("runtime/scroll.zig");
pub const sprites = @import("runtime/sprites.zig");
pub const sync = @import("runtime/sync.zig");
pub const video = @import("runtime/video.zig");

pub const fixed = @import("fixed.zig");

pub fn clamp(v: i32, lo: i32, hi: i32) i32 {
    return if (v < lo) lo else if (v > hi) hi else v;
}

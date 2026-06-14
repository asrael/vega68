const std = @import("std");

const ANGLE_BITS: u5 = 10;
const ANGLE_MASK: u32 = @intCast(TABLE_LEN - 1);
const FRAC_BITS: u5 = 16;
const QUARTER: i32 = @intCast(TABLE_LEN >> 2);
const TABLE_LEN: usize = 1 << ANGLE_BITS;

pub const Fixed = i32;
pub const FULL_CIRCLE: i32 = @intCast(TABLE_LEN);
pub const ONE: Fixed = 1 << FRAC_BITS;

const SINE_TABLE: [TABLE_LEN]Fixed = blk: {
    @setEvalBranchQuota(TABLE_LEN * 16);
    var t: [TABLE_LEN]Fixed = undefined;
    var i: usize = 0;

    while (i < TABLE_LEN) : (i += 1) {
        const radians = @as(f64, @floatFromInt(i)) * (2.0 * std.math.pi / @as(f64, @floatFromInt(TABLE_LEN)));
        t[i] = @intFromFloat(@round(@sin(radians) * @as(f64, @floatFromInt(ONE))));
    }
    break :blk t;
};

fn isqrt(n: u64) u64 {
    var rem: u64 = n;
    var root: u64 = 0;
    var bit: u64 = @as(u64, 1) << 62;
    while (bit > rem) bit >>= 2;
    while (bit != 0) {
        if (rem >= root + bit) {
            rem -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    return root;
}

pub fn fromInt(n: i32) Fixed {
    return n << FRAC_BITS;
}

pub fn toInt(x: Fixed) i32 {
    return x >> FRAC_BITS;
}

pub fn cos(angle: i32) Fixed {
    return sin(angle + QUARTER);
}

pub fn div(a: Fixed, b: Fixed) Fixed {
    return @intCast(@divTrunc(@as(i64, a) << FRAC_BITS, @as(i64, b)));
}

pub fn lerp(a: Fixed, b: Fixed, t: Fixed) Fixed {
    return a + mul(b - a, t);
}

pub fn mul(a: Fixed, b: Fixed) Fixed {
    return @intCast((@as(i64, a) * @as(i64, b)) >> FRAC_BITS);
}

pub fn sin(angle: i32) Fixed {
    return SINE_TABLE[@as(usize, @as(u32, @bitCast(angle)) & ANGLE_MASK)];
}

pub fn sqrt(x: Fixed) Fixed {
    if (x <= 0) return 0;
    return @intCast(isqrt(@as(u64, @intCast(x)) << FRAC_BITS));
}

test "arithmetic" {
    try std.testing.expectEqual(@as(i32, 5), toInt(fromInt(5)));
    try std.testing.expectEqual(fromInt(12), mul(fromInt(3), fromInt(4)));
    try std.testing.expectEqual(fromInt(3), div(fromInt(12), fromInt(4)));

    try std.testing.expectEqual(ONE >> 1, lerp(0, ONE, ONE >> 1)); // halfway = 0.5
    try std.testing.expectEqual(ONE >> 2, mul(ONE >> 1, ONE >> 1)); // 0.5 * 0.5 = 0.25
}

test "cardinal angles and wrapping" {
    // cardinal angles
    try std.testing.expectEqual(@as(Fixed, 0), sin(0));
    try std.testing.expectEqual(ONE, sin(QUARTER));
    try std.testing.expectEqual(ONE, cos(0));
    try std.testing.expectEqual(-ONE, sin(QUARTER * 3));

    // wrapping
    try std.testing.expectEqual(sin(0), sin(FULL_CIRCLE));
    try std.testing.expectEqual(sin(5), sin(5 - FULL_CIRCLE));
}

test "perfect squares" {
    try std.testing.expectEqual(fromInt(4), sqrt(fromInt(16)));
    try std.testing.expectEqual(fromInt(12), sqrt(fromInt(144)));
    try std.testing.expectEqual(@as(Fixed, 0), sqrt(fromInt(-9)));
}

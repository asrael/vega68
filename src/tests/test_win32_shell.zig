const std = @import("std");
const core = @import("core");
const shell = @import("shell");

test "scale.fit: exact 3x fit, no bars" {
    const r = shell.scale.fit(960, 720, 320, 240);
    try std.testing.expectEqual(@as(i32, 0), r.x);
    try std.testing.expectEqual(@as(i32, 0), r.y);
    try std.testing.expectEqual(@as(i32, 960), r.w);
    try std.testing.expectEqual(@as(i32, 720), r.h);
}

test "scale.fit: wide client pillarboxes (side bars)" {
    const r = shell.scale.fit(1920, 720, 320, 240);
    try std.testing.expectEqual(@as(i32, 960), r.w);
    try std.testing.expectEqual(@as(i32, 720), r.h);
    try std.testing.expectEqual(@as(i32, 480), r.x);
    try std.testing.expectEqual(@as(i32, 0), r.y);
}

test "scale.fit: tall client letterboxes (top/bottom bars)" {
    const r = shell.scale.fit(960, 1080, 320, 240);
    try std.testing.expectEqual(@as(i32, 960), r.w);
    try std.testing.expectEqual(@as(i32, 720), r.h);
    try std.testing.expectEqual(@as(i32, 0), r.x);
    try std.testing.expectEqual(@as(i32, 180), r.y);
}

test "scale.fit: sub-native client clamps to 1x (clipped, top-left biased)" {
    const r = shell.scale.fit(160, 120, 320, 240);
    try std.testing.expectEqual(@as(i32, 320), r.w);
    try std.testing.expectEqual(@as(i32, 240), r.h);
    try std.testing.expectEqual(@as(i32, -80), r.x);
    try std.testing.expectEqual(@as(i32, -60), r.y);
}

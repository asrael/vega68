//! enforces the per-8×8-cell 16-color rule

const std = @import("std");

pub const Violation = struct {
    banks_used: u32,
    cell_x: u32,
    cell_y: u32,
    object: []const u8,
};

/// Check that every 8×8 cell draws from a single 16-color bank.
/// empty = valid
pub fn checkCells(allocator: anytype, pixels: []const u8, width: u32, height: u32) ![]Violation {
    _ = allocator;
    _ = pixels;
    _ = width;
    _ = height;

    @panic("TODO: scan cells, count distinct banks, collect violations");
}

/// Render a violation as a one-line message.
pub fn format(v: Violation, writer: anytype) !void {
    try writer.print(
        "object '{s}' cell ({d},{d}) spans {d} palette banks",
        .{ v.object, v.cell_x, v.cell_y, v.banks_used },
    );
}

comptime {
    _ = std.fmt;
}

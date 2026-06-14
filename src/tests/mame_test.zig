//! SingleStepTests/m68000 (MAME microcoded) validation vectors

const std = @import("std");
const core = @import("core");
const h = @import("cpu_vectors.zig");

const Value = std.json.Value;

test "harness: loadState applies initial state (regs, sr, pc, prefetch, ram)" {
    var cpu: core.CPU = .{};
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();

    const json =
        \\[{ "initial": {
        \\  "d0": 286331153, "d1": 0, "d2": 0, "d3": 0, "d4": 0, "d5": 0, "d6": 0, "d7": 0,
        \\  "a0": 8192, "a1": 0, "a2": 0, "a3": 0, "a4": 0, "a5": 0, "a6": 0,
        \\  "usp": 12288, "ssp": 0, "sr": 0, "pc": 12,
        \\  "prefetch": [20081, 20081], "ram": [[4096, 171]]
        \\}}]
    ;
    const parsed = try std.json.parseFromSlice(Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const case = parsed.value.array.items[0].object;
    const initial = case.get("initial").?.object;

    h.resetMachine(&cpu, &mem);
    h.loadState(&cpu, &mem, &initial, h.Profile.musashi);

    try std.testing.expectEqual(@as(u32, 0x1111_1111), cpu.d[0]);
    try std.testing.expectEqual(@as(u32, 0x0000_2000), cpu.a[0]);
    try std.testing.expectEqual(@as(u32, 0x0000_3000), cpu.a[7]); // S=0, A7 = USP
    try std.testing.expectEqual(@as(u16, 0x0000), @as(u16, @bitCast(cpu.sr)));
    try std.testing.expectEqual(@as(u32, 0x0000_000C), cpu.pc);

    try std.testing.expectEqual(@as(u16, 0x4E71), mem.read16(0x0000_000C));
    try std.testing.expectEqual(@as(u16, 0x4E71), mem.read16(0x0000_000E));
    try std.testing.expectEqual(@as(u8, 0xAB), mem.read8(0x0000_1000));
}

test "mame vectors" {
    try h.runSet68000("vectors/mame/", h.Profile.mame);
}

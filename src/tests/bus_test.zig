//! console-bus region decode and machine-boot basics

const std = @import("std");
const core = @import("core");

// fn pointers can't close over state; module-level buffer captures writes
var cap_buf: [256]u8 = undefined;
var cap_len: usize = 0;
var cap_fd: u32 = 0;
fn capture(fd: u32, bytes: []const u8) void {
    cap_fd = fd;
    for (bytes) |byte| {
        if (cap_len < cap_buf.len) {
            cap_buf[cap_len] = byte;
            cap_len += 1;
        }
    }
}

fn newBus(rom: []const u8) !*core.Bus {
    const bus = try std.testing.allocator.create(core.Bus);
    bus.* = .{ .rom = rom };
    return bus;
}

test "bus: big-endian ROM reads" {
    const rom = [_]u8{ 0x12, 0x34, 0x56, 0x78 };
    const bus = try newBus(&rom);
    defer std.testing.allocator.destroy(bus);
    try std.testing.expectEqual(@as(u16, 0x1234), bus.read16(0));
    try std.testing.expectEqual(@as(u32, 0x12345678), bus.read32(0));
    try std.testing.expectEqual(@as(u8, 0x56), bus.read8(2));
}

test "bus: write32 to exit_reg latches value and flag" {
    const bus = try newBus(&.{});
    defer std.testing.allocator.destroy(bus);
    try std.testing.expect(!bus.exited);
    bus.write32(core.memmap.EXIT_REG, 0x00C0FFEE);
    try std.testing.expect(bus.exited);
    try std.testing.expectEqual(@as(u32, 0x00C0FFEE), bus.status);
}

test "bus: work RAM round-trips a long, big-endian" {
    const bus = try newBus(&.{});
    defer std.testing.allocator.destroy(bus);
    bus.write32(core.memmap.WORK_RAM_BASE, 0xDEADBEEF);
    try std.testing.expectEqual(@as(u32, 0xDEADBEEF), bus.read32(core.memmap.WORK_RAM_BASE));
    try std.testing.expectEqual(@as(u8, 0xDE), bus.read8(core.memmap.WORK_RAM_BASE));
}

test "bus: stub device region reads 0 and ignores writes" {
    const bus = try newBus(&.{});
    defer std.testing.allocator.destroy(bus);
    bus.write16(core.memmap.PPU_REGS_BASE, 0xFFFF); // ignored
    try std.testing.expectEqual(@as(u16, 0), bus.read16(core.memmap.PPU_REGS_BASE));
}

test "cpu: reset loads SP and PC from the vector table (big-endian)" {
    const rom = [_]u8{
        0x80, 0x20, 0x00, 0x00, // [0] initial SP = 0x80200000
        0x00, 0x00, 0x00, 0x08, // [1] reset PC = 0x00000008
    };
    const bus = try newBus(&rom);
    defer std.testing.allocator.destroy(bus);
    var cpu: core.CPU = .{};
    cpu.reset(bus);
    try std.testing.expectEqual(@as(u32, 0x80200000), cpu.a[7]);
    try std.testing.expectEqual(@as(u32, 0x00000008), cpu.pc);
}

test "cpu: an unimplemented opcode raises the illegal-instruction exception (vector 4)" {
    // line-A at 0; vector 4 (offset 0x10) → handler 0x00000100
    var rom = [_]u8{0} ** 0x14;
    rom[0] = 0xA0; // line-A
    rom[1] = 0x00;
    rom[0x12] = 0x01; // vector 4 low word high byte → 0x0100
    const bus = try newBus(&rom);
    defer std.testing.allocator.destroy(bus);
    var cpu: core.CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x0000)); // user
    cpu.ssp = core.memmap.WORK_RAM_BASE + 0x100; // ISP in writable RAM
    cpu.a[7] = 0; // active USP (unused by the frame push)
    cpu.step(bus);
    try std.testing.expect(!cpu.stopped);
    try std.testing.expectEqual(@as(u32, 0x0000_0100), cpu.pc);
    try std.testing.expectEqual(@as(u1, 1), cpu.sr.s);
    try std.testing.expectEqual(core.memmap.WORK_RAM_BASE + 0x100 - 6, cpu.a[7]);
}

test "bus: cartridge rom reads at cart_base, big-endian" {
    const cart = [_]u8{ 0xCA, 0xFE, 0xBA, 0xBE };
    const bus = try newBus(&.{});
    defer std.testing.allocator.destroy(bus);
    bus.cart = &cart;
    try std.testing.expectEqual(@as(u16, 0xCAFE), bus.read16(core.memmap.CART_BASE));
    try std.testing.expectEqual(@as(u32, 0xCAFEBABE), bus.read32(core.memmap.CART_BASE));
    try std.testing.expectEqual(@as(u8, 0xBA), bus.read8(core.memmap.CART_BASE + 2));
}

test "bus: console_reg byte write forwards to the host sink" {
    cap_len = 0;
    const bus = try newBus(&.{});
    defer std.testing.allocator.destroy(bus);
    bus.host_write = &capture;
    bus.write8(core.memmap.CONSOLE_REG, 'A');
    bus.write8(core.memmap.CONSOLE_REG, 'Z');
    try std.testing.expectEqual(@as(u32, 1), cap_fd); // console uses fd 1
    try std.testing.expectEqualStrings("AZ", cap_buf[0..cap_len]);
}

test "cpu: reset enters supervisor (S=1, IPL=7) and loads SP/PC" {
    const rom = [_]u8{
        0x80, 0x10, 0x00, 0x00, // [0] initial SSP = 0x80100000
        0x00, 0x00, 0x00, 0x08, // [1] reset PC = 0x00000008
    };
    const bus = try newBus(&rom);
    defer std.testing.allocator.destroy(bus);
    var cpu: core.CPU = .{};
    cpu.reset(bus);
    try std.testing.expectEqual(@as(u1, 1), cpu.sr.s);
    try std.testing.expectEqual(@as(u3, 7), cpu.sr.ipl); // IPL=7 masks all IRQs
    try std.testing.expectEqual(@as(u32, 0x80100000), cpu.a[7]);
    try std.testing.expectEqual(@as(u32, 0x00000008), cpu.pc);
}

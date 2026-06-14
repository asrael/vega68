//! exception processing: takeException / rte / triggers / address error
const std = @import("std");
const core = @import("core");
const h = @import("cpu_vectors.zig");

const CPU = core.CPU;

test "takeException: group-2 trap from user mode pushes 6-byte frame on ISP, enters supervisor" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();

    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x0015)); // user (S=0), CCR x/z/c set
    cpu.usp = 0x0001_0000;
    cpu.ssp = 0x0002_0000;
    cpu.a[7] = cpu.usp;
    cpu.pc = 0x0000_4000;
    mem.write32(6 * 4, 0x00AB_CDEE); // CHK vector (6) → handler

    cpu.takeException(&mem, 6, cpu.pc);

    // entered supervisor, CCR preserved
    try std.testing.expectEqual(@as(u16, 0x2015), @as(u16, @bitCast(cpu.sr)));
    try std.testing.expectEqual(@as(u32, 0x0002_0000 - 6), cpu.a[7]); // ISP decremented by 6
    try std.testing.expectEqual(@as(u32, 0x0001_0000), cpu.usp);
    // frame: SR @ SSP+0, PC @ SSP+2
    try std.testing.expectEqual(@as(u16, 0x0015), mem.read16(cpu.a[7]));
    try std.testing.expectEqual(@as(u32, 0x0000_4000), mem.read32(cpu.a[7] + 2));
    try std.testing.expectEqual(@as(u32, 0x00AB_CDEE), cpu.pc);
}

test "rte: pops SR+PC, returns to user mode and re-banks A7 to USP" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();

    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000)); // supervisor
    cpu.usp = 0x0001_0000;
    cpu.a[7] = 0x0002_0000; // active ISP
    cpu.ssp = 0;
    // frame on ISP: SR=0x0015 (user), PC=0x0000_4000
    mem.write16(cpu.a[7], 0x0015);
    mem.write32(cpu.a[7] + 2, 0x0000_4000);
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E73); // RTE

    cpu.step(&mem);

    try std.testing.expectEqual(@as(u16, 0x0015), @as(u16, @bitCast(cpu.sr)));
    try std.testing.expectEqual(@as(u32, 0x0000_4000), cpu.pc);
    try std.testing.expectEqual(@as(u32, 0x0001_0000), cpu.a[7]); // active is USP
    try std.testing.expectEqual(@as(u32, 0x0002_0000 + 6), cpu.ssp); // ISP advanced past frame
}

test "rte: executed in user mode raises a privilege violation (vector 8)" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();

    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x0000)); // user
    cpu.usp = 0x0001_0000;
    cpu.ssp = 0x0002_0000;
    cpu.a[7] = cpu.usp;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E73); // RTE
    mem.write32(8 * 4, 0x00CA_FE00); // privilege vector

    cpu.step(&mem);

    try std.testing.expectEqual(@as(u16, 0x2000), @as(u16, @bitCast(cpu.sr)));
    try std.testing.expectEqual(@as(u32, 0x00CA_FE00), cpu.pc);
    // group-1 frame: stacked PC = opcode addr, SR = user SR
    try std.testing.expectEqual(@as(u32, 0x0002_0000 - 6), cpu.a[7]);
    try std.testing.expectEqual(@as(u16, 0x0000), mem.read16(cpu.a[7]));
    try std.testing.expectEqual(@as(u32, 0x0000_9000), mem.read32(cpu.a[7] + 2));
}

test "trigger: unimplemented opcode → illegal (vector 4), group-1 stacked PC = opcode addr" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000)); // supervisor
    cpu.a[7] = 0x0002_0000;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0xA000); // line-A: unimplemented
    mem.write32(4 * 4, 0x00DE_AD00); // illegal vector
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x00DE_AD00), cpu.pc);
    try std.testing.expectEqual(@as(u32, 0x0000_9000), mem.read32(cpu.a[7] + 2));
    try std.testing.expect(!cpu.stopped);
}

test "trigger: TRAP #3 → vector 35, group-2 stacked PC = next instruction" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000));
    cpu.a[7] = 0x0002_0000;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E43); // TRAP #3
    mem.write32(35 * 4, 0x0011_2200);
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0011_2200), cpu.pc);
    try std.testing.expectEqual(@as(u32, 0x0000_9002), mem.read32(cpu.a[7] + 2));
}

test "trigger: TRAPV traps when V set, is a nop when V clear" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2002)); // supervisor, V set
    cpu.a[7] = 0x0002_0000;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E76); // TRAPV
    mem.write32(7 * 4, 0x0033_4400);
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0033_4400), cpu.pc);

    cpu.sr = @bitCast(@as(u16, 0x2000)); // V clear
    cpu.pc = 0x0000_9000;
    cpu.a[7] = 0x0002_0000;
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0000_9002), cpu.pc);
}

test "trigger: DIVU by zero → zero-divide (vector 5), group-2 stacked PC = next instruction" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000));
    cpu.a[7] = 0x0002_0000;
    cpu.d[0] = 0x0001_0000; // dividend
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x80FC); // DIVU #imm, D0
    mem.write16(cpu.pc + 2, 0x0000); // divisor = 0
    mem.write32(5 * 4, 0x0055_6600);
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0055_6600), cpu.pc);
    try std.testing.expectEqual(@as(u32, 0x0000_9004), mem.read32(cpu.a[7] + 2)); // past opcode + imm word
}

test "trigger: CHK out of bounds → vector 6, N=value<0, Z/V/C cleared, stacked SR carries it" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000)); // supervisor, CCR clear
    cpu.a[7] = 0x0002_0000;
    cpu.d[1] = 0x0000_0005; // bound D1.w = 5
    cpu.d[2] = 0x0000_FFF0; // value D2.w = -16, N=1
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4581); // CHK D1, D2  (bound=D1, value=D2)
    mem.write32(6 * 4, 0x0077_8800);
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0077_8800), cpu.pc);
    // stacked SR: S=1 (0x2000) | N (0x08) = 0x2008
    try std.testing.expectEqual(@as(u16, 0x2008), mem.read16(cpu.a[7]));
    try std.testing.expectEqual(@as(u32, 0x0000_9002), mem.read32(cpu.a[7] + 2));
}

test "address error: word read at odd EA pushes the 14-byte group-0 frame (vector 3)" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000)); // supervisor, FC = supervisor data = 5
    cpu.a[7] = 0x0002_0000;
    cpu.a[1] = 0x0010_0001; // odd
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x3011); // MOVE.W (A1), D0  → reads word at odd (A1)
    mem.write32(3 * 4, 0x00AE_0000); // address-error vector
    cpu.step(&mem);

    try std.testing.expectEqual(@as(u32, 0x00AE_0000), cpu.pc);
    try std.testing.expectEqual(@as(u1, 1), cpu.sr.s);
    const ssp = cpu.a[7];
    try std.testing.expectEqual(@as(u32, 0x0002_0000 - 14), ssp); // 14-byte frame
    // Figure 6-7 layout: SSW@+0, access@+2, IR@+6, SR@+8, PC@+10
    // SSW = (0x3011 & 0xFFE0) | read<<4 | FC(5) = 0x3015
    try std.testing.expectEqual(@as(u16, 0x3015), mem.read16(ssp + 0));
    try std.testing.expectEqual(@as(u32, 0x0010_0001), mem.read32(ssp + 2)); // access addr
    try std.testing.expectEqual(@as(u16, 0x3011), mem.read16(ssp + 6)); // IR
    try std.testing.expectEqual(@as(u16, 0x2000), mem.read16(ssp + 8)); // SR
    // PC at fault: MOVE.W (A1),D0 is one word; pc was 0x9002 before operand read
    try std.testing.expectEqual(@as(u32, 0x0000_9002), mem.read32(ssp + 10));
}

test "address error: byte access at an odd address does NOT fault" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000));
    cpu.a[7] = 0x0002_0000;
    cpu.a[1] = 0x0010_0001; // odd
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x1011); // MOVE.B (A1), D0 — byte, no fault at odd
    mem.write8(0x0010_0001, 0x7E);
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0000_9002), cpu.pc);
    try std.testing.expectEqual(@as(u8, 0x7E), @as(u8, @truncate(cpu.d[0])));
}

test "move usp: round-trips An <-> USP bank in supervisor" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x2000)); // supervisor
    cpu.a[0] = 0x0012_3456;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E60); // MOVE A0, USP
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x0012_3456), cpu.usp);

    cpu.usp = 0x00AB_CDEF;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E69); // MOVE USP, A1
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u32, 0x00AB_CDEF), cpu.a[1]);
}

test "move usp: in user mode raises a privilege violation (vector 8)" {
    var mem = h.FlatMemory.init(std.testing.allocator);
    defer mem.deinit();
    var cpu: CPU = .{};
    cpu.sr = @bitCast(@as(u16, 0x0000)); // user
    cpu.usp = 0x0001_0000;
    cpu.ssp = 0x0002_0000;
    cpu.a[7] = cpu.usp;
    cpu.pc = 0x0000_9000;
    mem.write16(cpu.pc, 0x4E60); // MOVE A0, USP (privileged)
    mem.write32(8 * 4, 0x00CA_FE00); // privilege vector
    cpu.step(&mem);
    try std.testing.expectEqual(@as(u16, 0x2000), @as(u16, @bitCast(cpu.sr)));
    try std.testing.expectEqual(@as(u32, 0x00CA_FE00), cpu.pc);
    try std.testing.expectEqual(@as(u32, 0x0002_0000 - 6), cpu.a[7]);
    try std.testing.expectEqual(@as(u32, 0x0000_9000), mem.read32(cpu.a[7] + 2));
}

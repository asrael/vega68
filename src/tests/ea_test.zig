//! unit tests for the effective-address resolver (src/core/cpu/ea.zig)

const std = @import("std");
const core = @import("core");
const ea = core.cpu.ea;
const ram = core.memmap.WORK_RAM_BASE;

test "ea: data_reg_direct reads Dn (word)" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.d[3] = 0x1234_5678;
    const a = ea.EffectiveAddress{ .mode = .data_reg_direct, .reg = 3 };
    try std.testing.expectEqual(@as(u32, 0x5678), ea.read(&sys.cpu, &sys.bus, a, .w));
}

test "ea: data_reg_direct reads Dn (byte and long)" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.d[1] = 0x1234_56AB;
    const a = ea.EffectiveAddress{ .mode = .data_reg_direct, .reg = 1 };
    try std.testing.expectEqual(@as(u32, 0xAB), ea.read(&sys.cpu, &sys.bus, a, .b));
    try std.testing.expectEqual(@as(u32, 0x1234_56AB), ea.read(&sys.cpu, &sys.bus, a, .l));
}

test "ea: addr_reg_direct reads An (word masks low 16, long full)" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[4] = 0xDEAD_BEEF;
    const a = ea.EffectiveAddress{ .mode = .addr_reg_direct, .reg = 4 };
    try std.testing.expectEqual(@as(u32, 0xBEEF), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, 0xDEAD_BEEF), ea.read(&sys.cpu, &sys.bus, a, .l));
}

test "ea: addr_reg_indirect reads (An)" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[2] = ram + 0x100;
    sys.bus.write16(ram + 0x100, 0xABCD);
    const a = ea.EffectiveAddress{ .mode = .addr_reg_indirect, .reg = 2 };
    try std.testing.expectEqual(@as(u32, 0xABCD), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x100), sys.cpu.a[2]); // An unchanged
}

test "ea: postincrement reads (An)+ and advances An by size" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[2] = ram;
    sys.bus.write32(ram, 0xCAFEBABE);
    const a = ea.EffectiveAddress{ .mode = .postincrement, .reg = 2 };
    try std.testing.expectEqual(@as(u32, 0xCAFEBABE), ea.read(&sys.cpu, &sys.bus, a, .l));
    try std.testing.expectEqual(@as(u32, ram + 4), sys.cpu.a[2]);
}

test "ea: postincrement byte on A7 advances by 2" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[7] = ram + 0x200;
    sys.bus.write8(ram + 0x200, 0x42);
    const a = ea.EffectiveAddress{ .mode = .postincrement, .reg = 7 };
    try std.testing.expectEqual(@as(u32, 0x42), ea.read(&sys.cpu, &sys.bus, a, .b));
    try std.testing.expectEqual(@as(u32, ram + 0x202), sys.cpu.a[7]); // A7 byte step = 2
}

test "ea: predecrement reads -(An) decrementing first" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[3] = ram + 0x100;
    sys.bus.write16(ram + 0x100 - 2, 0x1357);
    const a = ea.EffectiveAddress{ .mode = .predecrement, .reg = 3 };
    try std.testing.expectEqual(@as(u32, 0x1357), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x100 - 2), sys.cpu.a[3]);
}

test "ea: predecrement byte on A7 decrements by 2" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[7] = ram + 0x200;
    sys.bus.write8(ram + 0x200 - 2, 0x99);
    const a = ea.EffectiveAddress{ .mode = .predecrement, .reg = 7 };
    try std.testing.expectEqual(@as(u32, 0x99), ea.read(&sys.cpu, &sys.bus, a, .b));
    try std.testing.expectEqual(@as(u32, ram + 0x200 - 2), sys.cpu.a[7]);
}

test "ea: displacement (d16,An) with negative d16, advances PC by 2" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x100;
    sys.cpu.pc = ram + 0x1000;
    sys.bus.write16(ram + 0x1000, 0xFFF0); // -16
    sys.bus.write16(ram + 0x100 - 16, 0x7654);
    const a = ea.EffectiveAddress{ .mode = .displacement, .reg = 1 };
    try std.testing.expectEqual(@as(u32, 0x7654), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x1002), sys.cpu.pc);
}

test "ea: indexed (d8,An,Dn.w) sign-extends word index" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[2] = ram + 0x1000;
    sys.cpu.d[5] = 0x0000_FFFE; // low word = -2 sign-extended
    sys.cpu.pc = ram + 0x2000;
    // ext: bit15=0 (Dn), bits14-12=5 (D5), bit11=0 (word), d8=0x04
    sys.bus.write16(ram + 0x2000, 0b0101_0000_0000_0100);
    sys.bus.write16(ram + 0x1000 + 4 - 2, 0x4321);
    const a = ea.EffectiveAddress{ .mode = .indexed, .reg = 2 };
    try std.testing.expectEqual(@as(u32, 0x4321), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x2002), sys.cpu.pc);
}

test "ea: indexed (d8,An,An.l) uses full long index" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x1000;
    sys.cpu.a[6] = 0x0000_0010; // full long index = 16
    sys.cpu.pc = ram + 0x2000;
    // ext: bit15=1 (An), bits14-12=6 (A6), bit11=1 (long), d8=0x08
    sys.bus.write16(ram + 0x2000, 0b1110_1000_0000_1000);
    sys.bus.write16(ram + 0x1000 + 8 + 16, 0x9876);
    const a = ea.EffectiveAddress{ .mode = .indexed, .reg = 1 };
    try std.testing.expectEqual(@as(u32, 0x9876), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x2002), sys.cpu.pc);
}

test "ea: absolute_word sign-extends to high address" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x3000;
    sys.bus.write16(ram + 0x3000, 0x0010); // addr $10 (outside Work RAM)
    sys.bus.write16(0x10, 0xBEEF);
    const a = ea.EffectiveAddress{ .mode = .absolute_word, .reg = 0 };
    _ = ea.read(&sys.cpu, &sys.bus, a, .w); // assert ext word consumed
    try std.testing.expectEqual(@as(u32, ram + 0x3002), sys.cpu.pc);
}

test "ea: absolute_word negative value sign-extends into top of map" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x3000;
    // 0x8000 sign-extends to 0xFFFF_8000 (outside Work RAM); assert PC advances
    sys.bus.write16(ram + 0x3000, 0x8000);
    const a = ea.EffectiveAddress{ .mode = .absolute_word, .reg = 0 };
    _ = ea.read(&sys.cpu, &sys.bus, a, .w);
    try std.testing.expectEqual(@as(u32, ram + 0x3002), sys.cpu.pc);
}

test "ea: absolute_long reads a 32-bit address from the stream" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x3000;
    sys.bus.write32(ram + 0x3000, ram + 0x5000); // absolute address
    sys.bus.write32(ram + 0x5000, 0x0BAD_F00D);
    const a = ea.EffectiveAddress{ .mode = .absolute_long, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0x0BAD_F00D), ea.read(&sys.cpu, &sys.bus, a, .l));
    try std.testing.expectEqual(@as(u32, ram + 0x3004), sys.cpu.pc);
}

test "ea: pc_displacement base is the extension-word address" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x4000;
    sys.bus.write16(ram + 0x4000, 0x0010); // +16 from base (the ext word addr)
    sys.bus.write16(ram + 0x4000 + 16, 0x2222);
    const a = ea.EffectiveAddress{ .mode = .pc_displacement, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0x2222), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x4002), sys.cpu.pc);
}

test "ea: pc_indexed base is ext-word address plus index" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x4000;
    sys.cpu.d[0] = 0x20; // index = 32
    // ext word: Dn=0 (D0), long, d8=0x04
    sys.bus.write16(ram + 0x4000, 0b0000_1000_0000_0100);
    sys.bus.write16(ram + 0x4000 + 4 + 0x20, 0x3333);
    const a = ea.EffectiveAddress{ .mode = .pc_indexed, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0x3333), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x4002), sys.cpu.pc);
}

test "ea: immediate byte reads low 8 bits, advances PC by 2" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x6000;
    sys.bus.write16(ram + 0x6000, 0x00AB);
    const a = ea.EffectiveAddress{ .mode = .immediate, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0xAB), ea.read(&sys.cpu, &sys.bus, a, .b));
    try std.testing.expectEqual(@as(u32, ram + 0x6002), sys.cpu.pc);
}

test "ea: immediate word advances PC by 2" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x6000;
    sys.bus.write16(ram + 0x6000, 0x1234);
    const a = ea.EffectiveAddress{ .mode = .immediate, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0x1234), ea.read(&sys.cpu, &sys.bus, a, .w));
    try std.testing.expectEqual(@as(u32, ram + 0x6002), sys.cpu.pc);
}

test "ea: immediate long advances PC by 4" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.pc = ram + 0x6000;
    sys.bus.write32(ram + 0x6000, 0x1234_5678);
    const a = ea.EffectiveAddress{ .mode = .immediate, .reg = 0 };
    try std.testing.expectEqual(@as(u32, 0x1234_5678), ea.read(&sys.cpu, &sys.bus, a, .l));
    try std.testing.expectEqual(@as(u32, ram + 0x6004), sys.cpu.pc);
}

test "ea: write data_reg_direct byte preserves upper 24 bits" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.d[2] = 0x1122_3344;
    const a = ea.EffectiveAddress{ .mode = .data_reg_direct, .reg = 2 };
    ea.write(&sys.cpu, &sys.bus, a, .b, 0xFF);
    try std.testing.expectEqual(@as(u32, 0x1122_33FF), sys.cpu.d[2]);
}

test "ea: write data_reg_direct word preserves upper 16 bits" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.d[2] = 0x1122_3344;
    const a = ea.EffectiveAddress{ .mode = .data_reg_direct, .reg = 2 };
    ea.write(&sys.cpu, &sys.bus, a, .w, 0xBEEF);
    try std.testing.expectEqual(@as(u32, 0x1122_BEEF), sys.cpu.d[2]);
}

test "ea: write data_reg_direct long replaces all 32 bits" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.d[2] = 0x1122_3344;
    const a = ea.EffectiveAddress{ .mode = .data_reg_direct, .reg = 2 };
    ea.write(&sys.cpu, &sys.bus, a, .l, 0xAABB_CCDD);
    try std.testing.expectEqual(@as(u32, 0xAABB_CCDD), sys.cpu.d[2]);
}

test "ea: write addr_reg_direct word sign-extends (MOVEA.W)" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[3] = 0x0000_0000;
    const a = ea.EffectiveAddress{ .mode = .addr_reg_direct, .reg = 3 };
    ea.write(&sys.cpu, &sys.bus, a, .w, 0x8000); // sign bit set
    try std.testing.expectEqual(@as(u32, 0xFFFF_8000), sys.cpu.a[3]);
}

test "ea: write addr_reg_direct long stores full 32 bits" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    const a = ea.EffectiveAddress{ .mode = .addr_reg_direct, .reg = 3 };
    ea.write(&sys.cpu, &sys.bus, a, .l, 0x1234_5678);
    try std.testing.expectEqual(@as(u32, 0x1234_5678), sys.cpu.a[3]);
}

test "ea: write addr_reg_indirect stores to memory" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x100;
    const a = ea.EffectiveAddress{ .mode = .addr_reg_indirect, .reg = 1 };
    ea.write(&sys.cpu, &sys.bus, a, .l, 0xFEED_FACE);
    try std.testing.expectEqual(@as(u32, 0xFEED_FACE), sys.bus.read32(ram + 0x100));
}

test "ea: write postincrement stores then advances An" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x100;
    const a = ea.EffectiveAddress{ .mode = .postincrement, .reg = 1 };
    ea.write(&sys.cpu, &sys.bus, a, .w, 0xAA55);
    try std.testing.expectEqual(@as(u32, 0xAA55), sys.bus.read16(ram + 0x100));
    try std.testing.expectEqual(@as(u32, ram + 0x102), sys.cpu.a[1]);
}

test "ea: write predecrement decrements then stores" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x100;
    const a = ea.EffectiveAddress{ .mode = .predecrement, .reg = 1 };
    ea.write(&sys.cpu, &sys.bus, a, .w, 0x5AA5);
    try std.testing.expectEqual(@as(u32, ram + 0x100 - 2), sys.cpu.a[1]);
    try std.testing.expectEqual(@as(u32, 0x5AA5), sys.bus.read16(ram + 0x100 - 2));
}

test "ea: write displacement consumes ext word and stores" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.cpu.a[1] = ram + 0x100;
    sys.cpu.pc = ram + 0x1000;
    sys.bus.write16(ram + 0x1000, 0x0008); // +8
    const a = ea.EffectiveAddress{ .mode = .displacement, .reg = 1 };
    ea.write(&sys.cpu, &sys.bus, a, .w, 0x1234);
    try std.testing.expectEqual(@as(u32, 0x1234), sys.bus.read16(ram + 0x108));
    try std.testing.expectEqual(@as(u32, ram + 0x1002), sys.cpu.pc);
}

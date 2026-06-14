const decode = @import("decode.zig");

pub const AddressingMode = enum {
    data_reg_direct, // Dn
    addr_reg_direct, // An
    addr_reg_indirect, // (An)
    postincrement, // (An)+
    predecrement, // -(An)
    displacement, // (d16,An)
    indexed, // (d8,An,Xn)
    absolute_word, // (xxx).W
    absolute_long, // (xxx).L
    pc_displacement, // (d16,PC)
    pc_indexed, // (d8,PC,Xn)
    immediate, // #imm
};

pub const EffectiveAddress = struct {
    mode: AddressingMode = .data_reg_direct,
    reg: u3 = 0,
};

pub fn decodeEA(mode: u3, reg: u3) EffectiveAddress {
    const m: AddressingMode = switch (mode) {
        0 => .data_reg_direct,
        1 => .addr_reg_direct,
        2 => .addr_reg_indirect,
        3 => .postincrement,
        4 => .predecrement,
        5 => .displacement,
        6 => .indexed,
        7 => switch (reg) {
            0 => .absolute_word,
            1 => .absolute_long,
            2 => .pc_displacement,
            3 => .pc_indexed,
            else => .immediate,
        },
    };
    return .{ .mode = m, .reg = reg };
}

const Line = enum(u4) {
    arith_imm = 0x0, // ORI/ANDI/SUBI/ADDI/EORI/CMPI/BTST (immediate and bit ops)
    move_b = 0x1, // MOVE.B / MOVEA
    move_l = 0x2, // MOVE.L / MOVEA
    move_w = 0x3, // MOVE.W / MOVEA
    single_op = 0x4, // NOT/NEG/CLR/TST/EXT/SWAP (+ JMP/JSR/LEA/PEA/MOVE-SR unimpl)
    addq_subq = 0x5, // ADDQ/SUBQ (Scc/DBcc share, ss==11)
    branch = 0x6, // Bcc / BRA / BSR
    moveq = 0x7, // MOVEQ
    or_div = 0x8, // OR (DIVU/DIVS share opmode 3/7)
    sub = 0x9, // SUB/SUBA/SUBX
    cmp_eor = 0xB, // CMP/CMPA/CMPM (EOR shares opmode 4/5/6)
    and_mul = 0xC, // AND (MULU/MULS share opmode 3/7)
    add = 0xD, // ADD/ADDA/ADDX
    shift = 0xE, // ASL/ASR/LSL/LSR/ROL/ROR/ROXL/ROXR (shift/rotate)
    _,
};

pub fn classify(operation_word: u16) decode.Instruction {
    const w = operation_word;
    return switch (@as(Line, @enumFromInt(@as(u4, @truncate(w >> 12))))) {
        .arith_imm => classifyLine0(w),
        .move_b => classifyMove(w, .b),
        .move_w => classifyMove(w, .w),
        .move_l => classifyMove(w, .l),
        .single_op => classifyLine4(w),
        .addq_subq => classifyLine5(w),
        .branch => classifyLine6(w), // Bcc / BRA / BSR
        .moveq => classifyMoveq(w),
        .or_div => classifyLogic(w, .or_),
        .sub => classifyAddSub(w, .sub),
        .cmp_eor => classifyLineB(w),
        .and_mul => classifyLogic(w, .and_),
        .add => classifyAddSub(w, .add),
        .shift => classifyLineE(w),
        _ => .{ .word = w },
    };
}

fn classifyLine0(w: u16) decode.Instruction {
    if ((w & 0x0100) != 0 and ((w >> 3) & 7) == 0b001) {
        const opmode: u3 = @truncate(w >> 6);
        if (opmode == 0b100 or opmode == 0b101 or opmode == 0b110 or opmode == 0b111) {
            const size: decode.Size = if ((opmode & 1) != 0) .l else .w;
            const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w >> 9) };
            const an = EffectiveAddress{ .mode = .addr_reg_indirect, .reg = @truncate(w) };
            return .{ .word = w, .op = .movep, .size = size, .src = dn, .dst = an };
        }
    }

    if ((w & 0x0100) != 0 or (w & 0x0F00) == 0x0800) {
        const bitop: decode.Op = switch (@as(u2, @truncate(w >> 6))) {
            0b00 => .btst,
            0b01 => .bchg,
            0b10 => .bclr,
            0b11 => .bset,
        };
        const dst = lowEA(w);
        return .{ .word = w, .op = bitop, .dst = dst };
    }

    if (sizeIs11(w)) return .{ .word = w };

    const op: decode.Op = switch (@as(u8, @truncate(w >> 8))) {
        0x06 => .addi, // 0000 0110
        0x04 => .subi, // 0000 0100
        0x0C => .cmpi, // 0000 1100
        0x00 => .ori, // 0000 0000
        0x02 => .andi, // 0000 0010
        0x0A => .eori, // 0000 1010
        else => return .{ .word = w },
    };
    const size = sizeField(@truncate((w >> 6) & 3));
    const dst = lowEA(w);
    const imm = EffectiveAddress{ .mode = .immediate };
    return .{ .word = w, .op = op, .size = size, .src = imm, .dst = dst };
}

fn classifyLine4(w: u16) decode.Instruction {
    if (((w >> 6) & 7) == 0b110) {
        const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w >> 9) };
        return .{ .word = w, .op = .chk, .size = .w, .src = lowEA(w), .dst = dn };
    }
    const op: decode.Op = switch (@as(u4, @truncate(w >> 8))) {
        0x0 => .negx, // 0100 0000 ss EEEEEE — NEGX (ss==11 = MOVEfromSR, filtered below)
        0x2 => .clr,
        0x4 => .neg,
        0x6 => .not,
        0xA => if (sizeIs11(w))
            return .{ .word = w, .op = .tas, .size = .b, .dst = lowEA(w) }
        else
            .tst,
        0x8 => return classifyExtSwap(w),
        0xC => return classifyMovemLoad(w), // MOVEM load (mem→reg): 0100 1100 1 S EEEEEE
        0xE => return classifyLine4E(w), // JMP/JSR/RTS/RTR/NOP/LINK/UNLK (0100 1110 ...)
        else => {
            if (((w >> 6) & 7) == 0b111) {
                const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = @truncate(w >> 9) };
                return .{ .word = w, .op = .lea, .src = lowEA(w), .dst = an };
            }
            return .{ .word = w };
        },
    };

    if (sizeIs11(w)) return .{ .word = w };

    const size = sizeField(@truncate((w >> 6) & 3));
    const dst = lowEA(w);
    return .{ .word = w, .op = op, .size = size, .dst = dst };
}

fn classifyExtSwap(w: u16) decode.Instruction {
    const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
    return switch (@as(u2, @truncate(w >> 6))) {
        0b01 => if (((w >> 3) & 7) == 0)
            .{ .word = w, .op = .swap, .size = .l, .dst = dn }
        else
            .{ .word = w, .op = .pea, .dst = lowEA(w) },
        0b10 => if (((w >> 3) & 7) == 0)
            .{ .word = w, .op = .ext, .size = .w, .dst = dn }
        else
            .{ .word = w, .op = .movem, .size = .w, .dst = lowEA(w) },
        0b11 => if (((w >> 3) & 7) == 0)
            .{ .word = w, .op = .ext, .size = .l, .dst = dn }
        else
            .{ .word = w, .op = .movem, .size = .l, .dst = lowEA(w) },
        0b00 => .{ .word = w, .op = .nbcd, .size = .b, .dst = lowEA(w) },
    };
}

fn classifyMovemLoad(w: u16) decode.Instruction {
    switch (@as(u2, @truncate(w >> 6))) {
        0b00 => return .{ .word = w, .op = .mull, .size = .l, .src = lowEA(w) },
        0b01 => return .{ .word = w, .op = .divl, .size = .l, .src = lowEA(w) },
        else => {},
    }
    const size: decode.Size = if ((w & 0x0040) != 0) .l else .w;
    return .{ .word = w, .op = .movem, .size = size, .dst = lowEA(w) };
}

fn classifyLine4E(w: u16) decode.Instruction {
    switch (@as(u2, @truncate(w >> 6))) {
        0b11 => return .{ .word = w, .op = .jmp, .dst = lowEA(w) },
        0b10 => return .{ .word = w, .op = .jsr, .dst = lowEA(w) },
        else => {},
    }
    if ((w & 0xFFF8) == 0x4E50) {
        const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = @truncate(w) };
        return .{ .word = w, .op = .link, .dst = an };
    }
    if ((w & 0xFFF8) == 0x4E58) {
        const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = @truncate(w) };
        return .{ .word = w, .op = .unlk, .dst = an };
    }
    if ((w & 0xFFF0) == 0x4E40) return .{ .word = w, .op = .trap };
    if ((w & 0xFFF0) == 0x4E60) {
        const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = @truncate(w) };
        return .{ .word = w, .op = .move_usp, .dst = an };
    }
    return switch (w) {
        0x4E73 => .{ .word = w, .op = .rte },
        0x4E75 => .{ .word = w, .op = .rts },
        0x4E76 => .{ .word = w, .op = .trapv },
        0x4E77 => .{ .word = w, .op = .rtr },
        0x4E71 => .{ .word = w, .op = .nop },
        else => .{ .word = w },
    };
}

fn classifyLine6(w: u16) decode.Instruction {
    return .{ .word = w, .op = .bcc };
}

fn classifyMove(w: u16, size: decode.Size) decode.Instruction {
    const src = lowEA(w);
    const dst = decodeEA(@truncate(w >> 6), @truncate(w >> 9));
    return .{ .word = w, .op = .move, .size = size, .src = src, .dst = dst };
}

fn classifyMoveq(w: u16) decode.Instruction {
    if ((w & 0x0100) != 0) return .{ .word = w }; // bit 8 set: not MOVEQ
    return .{ .word = w, .op = .moveq, .size = .l };
}

fn classifyAddSub(w: u16, op: decode.Op) decode.Instruction {
    const reg: u3 = @truncate(w >> 9); // RRR
    const opmode: u3 = @truncate(w >> 6); // MMM
    const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = reg };
    const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = reg };
    const easpec = lowEA(w);

    return switch (opmode) {
        0, 1, 2 => .{ .word = w, .op = op, .size = byteWordLong(opmode), .src = easpec, .dst = dn },
        4, 5, 6 => blk: {
            const eamode: u3 = @truncate(w >> 3); // bits 5-3
            if (eamode == 0b000 or eamode == 0b001) {
                const xop: decode.Op = if (op == .add) .addx else .subx;
                const size = byteWordLong(opmode - 4);
                if (eamode == 0b000) {
                    const dy = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
                    break :blk .{ .word = w, .op = xop, .size = size, .src = dy, .dst = dn };
                }
                const ay = EffectiveAddress{ .mode = .predecrement, .reg = @truncate(w) };
                const ax = EffectiveAddress{ .mode = .predecrement, .reg = reg };
                break :blk .{ .word = w, .op = xop, .size = size, .src = ay, .dst = ax };
            }
            break :blk .{ .word = w, .op = op, .size = byteWordLong(opmode - 4), .src = dn, .dst = easpec };
        },
        3 => .{ .word = w, .op = op, .size = .w, .src = easpec, .dst = an },
        7 => .{ .word = w, .op = op, .size = .l, .src = easpec, .dst = an },
    };
}

fn classifyLine5(w: u16) decode.Instruction {
    if (sizeIs11(w)) {
        if (((w >> 3) & 7) == 0b001) {
            const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
            return .{ .word = w, .op = .dbcc, .dst = dn };
        }
        return .{ .word = w, .op = .scc, .size = .b, .dst = lowEA(w) };
    }
    const op: decode.Op = if ((w & 0x0100) == 0) .addq else .subq;
    const size = sizeField(@truncate((w >> 6) & 3));
    const dst = lowEA(w);
    return .{ .word = w, .op = op, .size = size, .dst = dst };
}

fn classifyLineB(w: u16) decode.Instruction {
    const reg: u3 = @truncate(w >> 9);
    const opmode: u3 = @truncate(w >> 6);
    const easpec = lowEA(w);
    switch (opmode) {
        0, 1, 2 => {
            const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = reg };
            return .{ .word = w, .op = .cmp, .size = byteWordLong(opmode), .src = easpec, .dst = dn };
        },
        3, 7 => {
            const an = EffectiveAddress{ .mode = .addr_reg_direct, .reg = reg };
            const size: decode.Size = if (opmode == 3) .w else .l;
            return .{ .word = w, .op = .cmpa, .size = size, .src = easpec, .dst = an };
        },
        4, 5, 6 => {
            if (((w >> 3) & 7) == 0b001) {
                const ax = EffectiveAddress{ .mode = .postincrement, .reg = reg };
                const ay = EffectiveAddress{ .mode = .postincrement, .reg = @truncate(w) };
                return .{ .word = w, .op = .cmpm, .size = byteWordLong(opmode - 4), .src = ay, .dst = ax };
            }
            const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = reg };
            return .{ .word = w, .op = .eor, .size = byteWordLong(opmode - 4), .src = dn, .dst = easpec };
        },
    }
}

fn classifyLogic(w: u16, op: decode.Op) decode.Instruction {
    const reg: u3 = @truncate(w >> 9); // RRR
    const opmode: u3 = @truncate(w >> 6); // MMM
    const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = reg };
    const easpec = lowEA(w);

    if ((w & 0x0100) != 0 and (w & 0x00F0) == 0) {
        const bcdop: decode.Op = if (op == .and_) .abcd else .sbcd;
        if ((w & 0x0008) == 0) {
            const dy = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
            return .{ .word = w, .op = bcdop, .size = .b, .src = dy, .dst = dn };
        }
        const ay = EffectiveAddress{ .mode = .predecrement, .reg = @truncate(w) };
        const ax = EffectiveAddress{ .mode = .predecrement, .reg = reg };
        return .{ .word = w, .op = bcdop, .size = .b, .src = ay, .dst = ax };
    }

    if (op == .and_ and (w & 0x0100) != 0) {
        const pat: u5 = @truncate(w >> 3);
        const carry = switch (pat) {
            0b01000, 0b01001, 0b10001 => true,
            else => false,
        };
        if (carry) {
            const dx = EffectiveAddress{ .mode = .data_reg_direct, .reg = reg };
            const yy = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
            return .{ .word = w, .op = .exg, .src = dx, .dst = yy };
        }
    }

    return switch (opmode) {
        0, 1, 2 => .{ .word = w, .op = op, .size = byteWordLong(opmode), .src = easpec, .dst = dn },
        4, 5, 6 => .{ .word = w, .op = op, .size = byteWordLong(opmode - 4), .src = dn, .dst = easpec },
        3, 7 => .{
            .word = w,
            .op = if (op == .and_) .mul else .div,
            .size = .w,
            .src = easpec,
            .dst = dn,
        },
    };
}

fn classifyLineE(w: u16) decode.Instruction {
    if (sizeIs11(w)) {
        const dst = lowEA(w);
        return .{ .word = w, .op = .shift, .size = .w, .dst = dst };
    }

    const size = sizeField(@truncate((w >> 6) & 3));
    const dn = EffectiveAddress{ .mode = .data_reg_direct, .reg = @truncate(w) };
    return .{ .word = w, .op = .shift, .size = size, .dst = dn };
}

fn lowEA(w: u16) EffectiveAddress {
    return decodeEA(@truncate(w >> 3), @truncate(w));
}

fn sizeIs11(w: u16) bool {
    return (w & 0x00C0) == 0x00C0;
}

fn byteWordLong(opmode: u3) decode.Size {
    return switch (opmode) {
        0 => .b,
        1 => .w,
        else => .l,
    };
}

fn sizeField(ss: u2) decode.Size {
    return switch (ss) {
        0b00 => .b,
        0b01 => .w,
        else => .l,
    };
}

fn sizeBytes(size: decode.Size) u32 {
    return switch (size) {
        .b => 1,
        .w => 2,
        .l => 4,
    };
}

fn signExtend16(x: u16) u32 {
    return @bitCast(@as(i32, @as(i16, @bitCast(x))));
}

fn signExtend8(x: u8) u32 {
    return @bitCast(@as(i32, @as(i8, @bitCast(x))));
}

fn memRead(cpu: anytype, bus: anytype, addr: u32, size: decode.Size) u32 {
    return switch (size) {
        .b => bus.read8(addr),
        .w => cpu.rd16(bus, addr),
        .l => cpu.rd32(bus, addr),
    };
}

fn memWrite(cpu: anytype, bus: anytype, addr: u32, size: decode.Size, value: u32) void {
    switch (size) {
        .b => bus.write8(addr, @truncate(value)),
        .w => cpu.wr16(bus, addr, @truncate(value)),
        .l => cpu.wr32(bus, addr, value),
    }
}

fn maskSize(value: u32, size: decode.Size) u32 {
    return switch (size) {
        .b => value & 0xFF,
        .w => value & 0xFFFF,
        .l => value,
    };
}

fn writeDataReg(cpu: anytype, reg: u3, size: decode.Size, value: u32) void {
    const old = cpu.d[reg];
    cpu.d[reg] = switch (size) {
        .b => (old & 0xFFFF_FF00) | (value & 0xFF),
        .w => (old & 0xFFFF_0000) | (value & 0xFFFF),
        .l => value,
    };
}

fn writeAddrReg(cpu: anytype, reg: u3, size: decode.Size, value: u32) void {
    cpu.a[reg] = switch (size) {
        .b, .w => signExtend16(@truncate(value)),
        .l => value,
    };
}

fn adjustStep(reg: u3, size: decode.Size) u32 {
    if (size == .b and reg == 7) return 2;
    return sizeBytes(size);
}

fn briefIndex(cpu: anytype, bus: anytype) u32 {
    const ext = bus.read16(cpu.pc);
    cpu.pc +%= 2;

    const is_addr = (ext & 0x8000) != 0;
    const regnum: u3 = @truncate(ext >> 12);
    const long_index = (ext & 0x0800) != 0;
    const d8 = signExtend8(@truncate(ext));
    const raw = if (is_addr) cpu.a[regnum] else cpu.d[regnum];
    const index_val = if (long_index) raw else signExtend16(@truncate(raw));
    return d8 +% index_val;
}

fn computeAddress(cpu: anytype, bus: anytype, addr: EffectiveAddress, size: decode.Size) u32 {
    return switch (addr.mode) {
        .addr_reg_indirect => cpu.a[addr.reg],
        .postincrement => blk: {
            const a = cpu.a[addr.reg];
            cpu.a[addr.reg] +%= adjustStep(addr.reg, size);
            break :blk a;
        },
        .predecrement => blk: {
            cpu.a[addr.reg] -%= adjustStep(addr.reg, size);
            break :blk cpu.a[addr.reg];
        },
        .displacement => blk: {
            const d16 = signExtend16(bus.read16(cpu.pc));
            cpu.pc +%= 2;
            break :blk cpu.a[addr.reg] +% d16;
        },
        .indexed => blk: {
            const idx = briefIndex(cpu, bus);
            break :blk cpu.a[addr.reg] +% idx;
        },
        .absolute_word => blk: {
            const a = signExtend16(bus.read16(cpu.pc));
            cpu.pc +%= 2;
            break :blk a;
        },
        .absolute_long => blk: {
            const a = bus.read32(cpu.pc);
            cpu.pc +%= 4;
            break :blk a;
        },
        .pc_displacement => blk: {
            const base = cpu.pc;
            const d16 = signExtend16(bus.read16(cpu.pc));
            cpu.pc +%= 2;
            break :blk base +% d16;
        },
        .pc_indexed => blk: {
            const base = cpu.pc;
            const idx = briefIndex(cpu, bus);
            break :blk base +% idx;
        },
        .data_reg_direct, .addr_reg_direct, .immediate => unreachable,
    };
}

pub fn read(cpu: anytype, bus: anytype, addr: EffectiveAddress, size: decode.Size) u32 {
    switch (addr.mode) {
        .data_reg_direct => return maskSize(cpu.d[addr.reg], size),
        .addr_reg_direct => return maskSize(cpu.a[addr.reg], size),
        .immediate => switch (size) {
            .b, .w => {
                const v = cpu.rd16(bus, cpu.pc);
                cpu.pc +%= 2;
                return maskSize(v, size);
            },
            .l => {
                const v = cpu.rd32(bus, cpu.pc);
                cpu.pc +%= 4;
                return v;
            },
        },
        else => {
            const a = computeAddress(cpu, bus, addr, size);
            return memRead(cpu, bus, a, size);
        },
    }
}

pub const Resolved = union(enum) {
    data_reg: u3,
    addr_reg: u3,
    mem: u32,
};

pub fn resolve(cpu: anytype, bus: anytype, addr: EffectiveAddress, size: decode.Size) Resolved {
    return switch (addr.mode) {
        .data_reg_direct => .{ .data_reg = addr.reg },
        .addr_reg_direct => .{ .addr_reg = addr.reg },
        .immediate, .pc_displacement, .pc_indexed => unreachable,
        else => .{ .mem = computeAddress(cpu, bus, addr, size) },
    };
}

pub fn resolveAddress(cpu: anytype, bus: anytype, addr: EffectiveAddress) u32 {
    return switch (addr.mode) {
        .data_reg_direct, .addr_reg_direct, .immediate => unreachable,
        else => computeAddress(cpu, bus, addr, .w),
    };
}

pub fn readResolved(cpu: anytype, bus: anytype, res: Resolved, size: decode.Size) u32 {
    return switch (res) {
        .data_reg => |r| maskSize(cpu.d[r], size),
        .addr_reg => |r| maskSize(cpu.a[r], size),
        .mem => |a| memRead(cpu, bus, a, size),
    };
}

pub fn writeResolved(cpu: anytype, bus: anytype, res: Resolved, size: decode.Size, value: u32) void {
    switch (res) {
        .data_reg => |r| writeDataReg(cpu, r, size, value),
        .addr_reg => |r| writeAddrReg(cpu, r, size, value),
        .mem => |a| memWrite(cpu, bus, a, size, value),
    }
}

pub fn write(cpu: anytype, bus: anytype, addr: EffectiveAddress, size: decode.Size, value: u32) void {
    switch (addr.mode) {
        .data_reg_direct => writeDataReg(cpu, addr.reg, size, value),
        .addr_reg_direct => writeAddrReg(cpu, addr.reg, size, value),
        .immediate, .pc_displacement, .pc_indexed => unreachable,
        else => {
            const a = computeAddress(cpu, bus, addr, size);
            memWrite(cpu, bus, a, size, value);
        },
    }
}

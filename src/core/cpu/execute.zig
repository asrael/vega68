const decode = @import("decode.zig");
const ea = @import("ea.zig");
const flags = @import("flags.zig");

fn rmwAddSub(cpu: anytype, b: anytype, res: ea.Resolved, src: u32, size: decode.Size, is_add: bool) void {
    const dst = ea.readResolved(cpu, b, res, size);
    if (is_add) {
        ea.writeResolved(cpu, b, res, size, dst +% src);
        flags.setFlagsAdd(cpu, dst, src, size);
    } else {
        ea.writeResolved(cpu, b, res, size, dst -% src);
        flags.setFlagsSub(cpu, dst, src, size);
    }
}

fn rmwLogic(cpu: anytype, b: anytype, res: ea.Resolved, result: u32, size: decode.Size) void {
    ea.writeResolved(cpu, b, res, size, result);
    flags.setNZ_clearVC(cpu, result, size);
}

fn signExtend8(x: u8) u32 {
    return @bitCast(@as(i32, @as(i8, @bitCast(x))));
}

fn signExtend16(x: u16) u32 {
    return @bitCast(@as(i32, @as(i16, @bitCast(x))));
}

fn signExtendToLong(value: u32, size: decode.Size) u32 {
    return switch (size) {
        .b => @bitCast(@as(i32, @as(i8, @bitCast(@as(u8, @truncate(value)))))),
        .w => @bitCast(@as(i32, @as(i16, @bitCast(@as(u16, @truncate(value)))))),
        .l => value,
    };
}

pub fn execute(cpu: anytype, b: anytype, insn: decode.Instruction) void {
    switch (insn.op) {
        .move => {
            const v = ea.read(cpu, b, insn.src, insn.size);
            ea.write(cpu, b, insn.dst, insn.size, v);
            if (insn.dst.mode != .addr_reg_direct) flags.setNZ_clearVC(cpu, v, insn.size);
        },
        .moveq => {
            const dn: u3 = @truncate(insn.word >> 9);
            const v = signExtend8(@truncate(insn.word));
            cpu.d[dn] = v;
            flags.setNZ_clearVC(cpu, v, .l);
        },
        .add => {
            if (insn.dst.mode == .addr_reg_direct) {
                const src = signExtendToLong(ea.read(cpu, b, insn.src, insn.size), insn.size);
                cpu.a[insn.dst.reg] +%= src;
                return;
            }

            const src = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);

            rmwAddSub(cpu, b, res, src, insn.size, true);
        },
        .sub => {
            if (insn.dst.mode == .addr_reg_direct) {
                const src = signExtendToLong(ea.read(cpu, b, insn.src, insn.size), insn.size);
                cpu.a[insn.dst.reg] -%= src;
                return;
            }

            const src = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);

            rmwAddSub(cpu, b, res, src, insn.size, false);
        },
        .addx, .subx => {
            const src = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const dst = ea.readResolved(cpu, b, res, insn.size);

            const r = if (insn.op == .addx)
                flags.setFlagsAddX(cpu, dst, src, insn.size)
            else
                flags.setFlagsSubX(cpu, dst, src, insn.size);

            ea.writeResolved(cpu, b, res, insn.size, r);
        },
        .negx => {
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const v = ea.readResolved(cpu, b, res, insn.size);
            const r = flags.setFlagsSubX(cpu, 0, v, insn.size);

            ea.writeResolved(cpu, b, res, insn.size, r);
        },
        .addq, .subq => {
            const q3: u3 = @truncate(insn.word >> 9);
            const q: u32 = if (q3 == 0) 8 else q3;

            if (insn.dst.mode == .addr_reg_direct) {
                const r = insn.dst.reg;
                cpu.a[r] = if (insn.op == .addq) cpu.a[r] +% q else cpu.a[r] -% q;
                return;
            }

            const res = ea.resolve(cpu, b, insn.dst, insn.size);

            rmwAddSub(cpu, b, res, q, insn.size, insn.op == .addq);
        },
        .addi, .subi => {
            const imm = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);

            rmwAddSub(cpu, b, res, imm, insn.size, insn.op == .addi);
        },
        .cmp => {
            const a = ea.read(cpu, b, insn.dst, insn.size); // Dn (minuend)
            const src = ea.read(cpu, b, insn.src, insn.size); // <ea> (subtrahend)

            flags.setFlagsCmp(cpu, a, src, insn.size);
        },
        .cmpa => {
            const a = cpu.a[insn.dst.reg];
            const src = signExtendToLong(ea.read(cpu, b, insn.src, insn.size), insn.size);

            flags.setFlagsCmp(cpu, a, src, .l);
        },
        .cmpm => {
            const src = ea.read(cpu, b, insn.src, insn.size);
            const a = ea.read(cpu, b, insn.dst, insn.size);

            flags.setFlagsCmp(cpu, a, src, insn.size);
        },
        .cmpi => {
            const imm = ea.read(cpu, b, insn.src, insn.size);
            const dst = ea.read(cpu, b, insn.dst, insn.size);

            flags.setFlagsCmp(cpu, dst, imm, insn.size);
        },
        .and_, .or_ => {
            const src = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const dst = ea.readResolved(cpu, b, res, insn.size);
            const r = if (insn.op == .and_) dst & src else dst | src;

            rmwLogic(cpu, b, res, r, insn.size);
        },
        .eor => {
            const src = ea.read(cpu, b, insn.src, insn.size); // Dn
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const dst = ea.readResolved(cpu, b, res, insn.size);

            rmwLogic(cpu, b, res, dst ^ src, insn.size);
        },
        .andi, .ori, .eori => {
            const imm = ea.read(cpu, b, insn.src, insn.size);
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const dst = ea.readResolved(cpu, b, res, insn.size);
            const r = switch (insn.op) {
                .andi => dst & imm,
                .ori => dst | imm,
                else => dst ^ imm, // eori
            };

            rmwLogic(cpu, b, res, r, insn.size);
        },
        .btst, .bchg, .bclr, .bset => {
            const bitnum_raw: u32 = if ((insn.word & 0x0100) != 0)
                cpu.d[@as(u3, @truncate(insn.word >> 9))]
            else blk: {
                const ext = b.read16(cpu.pc);
                cpu.pc +%= 2;
                break :blk ext & 0xFF;
            };

            const is_reg = insn.dst.mode == .data_reg_direct;
            const size: decode.Size = if (is_reg) .l else .b;
            const bit: u5 = @truncate(bitnum_raw % (if (is_reg) @as(u32, 32) else 8));
            const mask = @as(u32, 1) << bit;

            if (insn.op == .btst) {
                const val = ea.read(cpu, b, insn.dst, size);
                cpu.sr.z = @intFromBool((val & mask) == 0);
            } else {
                const res = ea.resolve(cpu, b, insn.dst, size);
                const val = ea.readResolved(cpu, b, res, size);
                cpu.sr.z = @intFromBool((val & mask) == 0);
                ea.writeResolved(cpu, b, res, size, switch (insn.op) {
                    .bchg => val ^ mask,
                    .bclr => val & ~mask,
                    .bset => val | mask,
                    else => unreachable,
                });
            }
        },
        .shift => executeShift(cpu, b, insn),
        .not => {
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const v = ea.readResolved(cpu, b, res, insn.size);

            rmwLogic(cpu, b, res, ~v, insn.size);
        },
        .neg => {
            const res = ea.resolve(cpu, b, insn.dst, insn.size);
            const v = ea.readResolved(cpu, b, res, insn.size);

            ea.writeResolved(cpu, b, res, insn.size, 0 -% v);
            flags.setFlagsSub(cpu, 0, v, insn.size);
        },
        .clr => {
            const res = ea.resolve(cpu, b, insn.dst, insn.size);

            // m68k CLR does a dummy read before writing 0
            _ = ea.readResolved(cpu, b, res, insn.size);
            ea.writeResolved(cpu, b, res, insn.size, 0);

            cpu.sr.z = 1;
            cpu.sr.n = 0;
            cpu.sr.v = 0;
            cpu.sr.c = 0;
        },
        .tst => {
            const v = ea.read(cpu, b, insn.dst, insn.size);

            flags.setNZ_clearVC(cpu, v, insn.size);
        },
        .ext => {
            const dn = insn.dst.reg;
            const r: u32 = switch (insn.size) {
                .w => (cpu.d[dn] & 0xFFFF_0000) |
                    (@as(u32, @bitCast(@as(i32, @as(i8, @bitCast(@as(u8, @truncate(cpu.d[dn]))))))) & 0xFFFF),
                else => @bitCast(@as(i32, @as(i16, @bitCast(@as(u16, @truncate(cpu.d[dn])))))),
            };

            cpu.d[dn] = r;
            flags.setNZ_clearVC(cpu, r, insn.size);
        },
        .swap => {
            const dn = insn.dst.reg;
            const r = (cpu.d[dn] << 16) | (cpu.d[dn] >> 16);

            cpu.d[dn] = r;
            flags.setNZ_clearVC(cpu, r, .l);
        },
        .bcc => {
            const cc: u4 = @truncate(insn.word >> 8);
            const base = cpu.pc;
            const disp8: u8 = @truncate(insn.word);
            const disp: u32 = if (disp8 == 0) blk: {
                const w = b.read16(cpu.pc);
                cpu.pc +%= 2;
                break :blk signExtend16(w);
            } else signExtend8(disp8);
            const target = base +% disp;

            if (cc == 1) {
                const ret = cpu.pc;
                cpu.a[7] -%= 4;
                cpu.wr32(b, cpu.a[7], ret);
                cpu.pc = target;
            } else if (cc == 0 or flags.testCondition(cc, cpu.sr)) {
                cpu.pc = target;
            }
        },
        .jmp => {
            cpu.pc = ea.resolveAddress(cpu, b, insn.dst);
        },
        .jsr => {
            const target = ea.resolveAddress(cpu, b, insn.dst);
            const ret = cpu.pc;

            cpu.a[7] -%= 4;
            cpu.wr32(b, cpu.a[7], ret);
            cpu.pc = target;
        },
        .rts => {
            cpu.pc = cpu.rd32(b, cpu.a[7]);
            cpu.a[7] +%= 4;
        },
        .rtr => {
            const ccr = cpu.rd16(b, cpu.a[7]);
            cpu.a[7] +%= 2;
            cpu.sr.c = @truncate(ccr);
            cpu.sr.v = @truncate(ccr >> 1);
            cpu.sr.z = @truncate(ccr >> 2);
            cpu.sr.n = @truncate(ccr >> 3);
            cpu.sr.x = @truncate(ccr >> 4);
            cpu.pc = cpu.rd32(b, cpu.a[7]);
            cpu.a[7] +%= 4;
        },
        .move_usp => {
            if (cpu.sr.s == 0) {
                cpu.takeException(b, 8, cpu.pc -% 2);
                return;
            }

            const an = insn.dst.reg;

            if ((insn.word & 0x0008) == 0) {
                cpu.usp = cpu.a[an];
            } else {
                cpu.a[an] = cpu.usp;
            }
        },
        .rte => {
            if (cpu.sr.s == 0) {
                cpu.takeException(b, 8, cpu.pc -% 2);
                return;
            }

            const new_sr = cpu.rd16(b, cpu.a[7]);

            cpu.a[7] +%= 2;

            const new_pc = cpu.rd32(b, cpu.a[7]);

            cpu.a[7] +%= 4;
            cpu.setSR(new_sr);
            cpu.pc = new_pc;
        },
        .nop => {},
        .scc => {
            const cc: u4 = @truncate(insn.word >> 8);
            const res = ea.resolve(cpu, b, insn.dst, .b);
            const v: u32 = if (flags.testCondition(cc, cpu.sr)) 0xFF else 0x00;

            ea.writeResolved(cpu, b, res, .b, v);
        },
        .dbcc => {
            const cc: u4 = @truncate(insn.word >> 8);
            const dn: u3 = @truncate(insn.word);
            const base = cpu.pc;
            const disp = signExtend16(b.read16(cpu.pc));
            cpu.pc +%= 2;

            if (flags.testCondition(cc, cpu.sr)) return;

            const counter: u16 = @truncate(cpu.d[dn]);
            const next = counter -% 1;
            cpu.d[dn] = (cpu.d[dn] & 0xFFFF_0000) | next;
            if (next != 0xFFFF) cpu.pc = base +% disp;
        },
        .lea => {
            cpu.a[insn.dst.reg] = ea.resolveAddress(cpu, b, insn.src);
        },
        .pea => {
            const addr = ea.resolveAddress(cpu, b, insn.dst);
            cpu.a[7] -%= 4;
            cpu.wr32(b, cpu.a[7], addr);
        },
        .link => {
            const disp = signExtend16(b.read16(cpu.pc));

            cpu.pc +%= 2;

            const an = insn.dst.reg;

            cpu.a[7] -%= 4;
            cpu.wr32(b, cpu.a[7], cpu.a[an]);
            cpu.a[an] = cpu.a[7];
            cpu.a[7] +%= disp;
        },
        .unlk => {
            const an = insn.dst.reg;

            cpu.a[7] = cpu.a[an];

            const popped = cpu.rd32(b, cpu.a[7]);

            cpu.a[7] +%= 4;
            cpu.a[an] = popped;
        },
        .exg => {
            const xx: u3 = @truncate(insn.word >> 9);
            const yy: u3 = @truncate(insn.word);
            const pat: u5 = @truncate(insn.word >> 3);

            switch (pat) {
                0b01000 => {
                    const t = cpu.d[xx];
                    cpu.d[xx] = cpu.d[yy];
                    cpu.d[yy] = t;
                },
                0b01001 => {
                    const t = cpu.a[xx];
                    cpu.a[xx] = cpu.a[yy];
                    cpu.a[yy] = t;
                },
                else => {
                    const t = cpu.d[xx];
                    cpu.d[xx] = cpu.a[yy];
                    cpu.a[yy] = t;
                },
            }
        },
        .mul => {
            const src = ea.read(cpu, b, insn.src, .w);
            const dn = insn.dst.reg;
            const signed = (insn.word & 0x0100) != 0;
            const product: u32 = if (signed) blk: {
                const a: i32 = @as(i16, @bitCast(@as(u16, @truncate(cpu.d[dn]))));
                const m: i32 = @as(i16, @bitCast(@as(u16, @truncate(src))));
                break :blk @bitCast(a *% m);
            } else (cpu.d[dn] & 0xFFFF) *% (src & 0xFFFF);

            cpu.d[dn] = product;
            flags.setNZ_clearVC(cpu, product, .l);
        },
        .div => {
            const divisor16 = ea.read(cpu, b, insn.src, .w);
            const dn = insn.dst.reg;
            const signed = (insn.word & 0x0100) != 0;

            if ((divisor16 & 0xFFFF) == 0) {
                cpu.takeException(b, 5, cpu.pc);
                return;
            }

            if (signed) {
                const dividend: i32 = @bitCast(cpu.d[dn]);
                const divisor: i32 = @as(i16, @bitCast(@as(u16, @truncate(divisor16))));
                const quotient = @divTrunc(dividend, divisor);

                if (quotient > 32767 or quotient < -32768) {
                    cpu.sr.v = 1;
                    cpu.sr.c = 0;
                    return;
                }

                const remainder = @rem(dividend, divisor);
                const q16: u32 = @as(u16, @bitCast(@as(i16, @truncate(quotient))));
                const r16: u32 = @as(u16, @bitCast(@as(i16, @truncate(remainder))));
                cpu.d[dn] = (r16 << 16) | q16;
                cpu.sr.n = @intFromBool((q16 >> 15) & 1 != 0);
                cpu.sr.z = @intFromBool(q16 == 0);
                cpu.sr.v = 0;
                cpu.sr.c = 0;
            } else {
                const dividend: u32 = cpu.d[dn];
                const divisor: u32 = divisor16 & 0xFFFF;
                const quotient = dividend / divisor;

                if (quotient > 0xFFFF) {
                    cpu.sr.v = 1;
                    cpu.sr.c = 0;
                    return;
                }

                const remainder = dividend % divisor;
                cpu.d[dn] = ((remainder & 0xFFFF) << 16) | (quotient & 0xFFFF);
                cpu.sr.n = @intFromBool((quotient >> 15) & 1 != 0);
                cpu.sr.z = @intFromBool((quotient & 0xFFFF) == 0);
                cpu.sr.v = 0;
                cpu.sr.c = 0;
            }
        },
        .mull => {
            const ext = b.read16(cpu.pc);
            cpu.pc +%= 2;

            const dl: u3 = @truncate(ext >> 12);
            const dh: u3 = @truncate(ext);
            const is64 = (ext & 0x0400) != 0;
            const signed = (ext & 0x0800) != 0;
            const src = ea.read(cpu, b, insn.src, .l);

            if (signed) {
                const a: i64 = @as(i32, @bitCast(cpu.d[dl]));
                const m: i64 = @as(i32, @bitCast(src));
                const product: i64 = a *% m;
                const u: u64 = @bitCast(product);
                const lo: u32 = @truncate(u);
                const hi: u32 = @truncate(u >> 32);

                if (is64) {
                    cpu.d[dl] = lo;
                    cpu.d[dh] = hi;
                    cpu.sr.n = @intFromBool((u >> 63) & 1 != 0);
                    cpu.sr.z = @intFromBool(u == 0);
                    cpu.sr.v = 0;
                    cpu.sr.c = 0;
                } else {
                    const fits = (product == @as(i64, @as(i32, @bitCast(lo))));
                    cpu.d[dl] = lo;
                    cpu.sr.n = @intFromBool((lo >> 31) & 1 != 0);
                    cpu.sr.z = @intFromBool(lo == 0);
                    cpu.sr.v = @intFromBool(!fits);
                    cpu.sr.c = 0;
                }
            } else {
                const a: u64 = cpu.d[dl];
                const m: u64 = src;
                const product: u64 = a *% m;
                const lo: u32 = @truncate(product);
                const hi: u32 = @truncate(product >> 32);

                if (is64) {
                    cpu.d[dl] = lo;
                    cpu.d[dh] = hi;
                    cpu.sr.n = @intFromBool((product >> 63) & 1 != 0);
                    cpu.sr.z = @intFromBool(product == 0);
                    cpu.sr.v = 0;
                    cpu.sr.c = 0;
                } else {
                    cpu.d[dl] = lo;
                    cpu.sr.n = @intFromBool((lo >> 31) & 1 != 0);
                    cpu.sr.z = @intFromBool(lo == 0);
                    cpu.sr.v = @intFromBool(hi != 0);
                    cpu.sr.c = 0;
                }
            }
        },
        .divl => {
            const ext = b.read16(cpu.pc);
            cpu.pc +%= 2;

            const dq: u3 = @truncate(ext >> 12);
            const dr: u3 = @truncate(ext);
            const is64 = (ext & 0x0400) != 0;
            const signed = (ext & 0x0800) != 0;
            const divisor32 = ea.read(cpu, b, insn.src, .l);

            if (divisor32 == 0) return;

            if (signed) {
                const dividend: i64 = if (is64)
                    @bitCast((@as(u64, cpu.d[dr]) << 32) | cpu.d[dq])
                else
                    @as(i32, @bitCast(cpu.d[dq]));
                const divisor: i64 = @as(i32, @bitCast(divisor32));
                const quotient = @divTrunc(dividend, divisor);

                if (quotient > 0x7FFF_FFFF or quotient < -0x8000_0000) {
                    cpu.sr.v = 1;
                    cpu.sr.c = 0;
                    return;
                }

                const remainder = @rem(dividend, divisor);
                const q: u32 = @truncate(@as(u64, @bitCast(quotient)));
                const r: u32 = @truncate(@as(u64, @bitCast(remainder)));
                cpu.d[dr] = r;
                cpu.d[dq] = q;
                cpu.sr.n = @intFromBool((q >> 31) & 1 != 0);
                cpu.sr.z = @intFromBool(q == 0);
                cpu.sr.v = 0;
                cpu.sr.c = 0;
            } else {
                const dividend: u64 = if (is64)
                    (@as(u64, cpu.d[dr]) << 32) | cpu.d[dq]
                else
                    cpu.d[dq];
                const divisor: u64 = divisor32;
                const quotient = dividend / divisor;

                if (quotient > 0xFFFF_FFFF) {
                    cpu.sr.v = 1;
                    cpu.sr.c = 0;
                    return;
                }

                const remainder = dividend % divisor;
                const q: u32 = @truncate(quotient);
                const r: u32 = @truncate(remainder);
                cpu.d[dr] = r;
                cpu.d[dq] = q;
                cpu.sr.n = @intFromBool((q >> 31) & 1 != 0);
                cpu.sr.z = @intFromBool(q == 0);
                cpu.sr.v = 0;
                cpu.sr.c = 0;
            }
        },
        .abcd, .sbcd => {
            const src = ea.read(cpu, b, insn.src, .b);
            const res = ea.resolve(cpu, b, insn.dst, .b);
            const dst = ea.readResolved(cpu, b, res, .b);
            const r = if (insn.op == .abcd)
                flags.bcdAdd(cpu, dst, src)
            else
                flags.bcdSub(cpu, dst, src);

            ea.writeResolved(cpu, b, res, .b, r);
        },
        .nbcd => {
            const res = ea.resolve(cpu, b, insn.dst, .b);
            const v = ea.readResolved(cpu, b, res, .b);
            const r = flags.bcdSub(cpu, 0, v);

            ea.writeResolved(cpu, b, res, .b, r);
        },
        .tas => {
            const res = ea.resolve(cpu, b, insn.dst, .b);
            const v = ea.readResolved(cpu, b, res, .b);
            cpu.sr.n = @intFromBool((v & 0x80) != 0);
            cpu.sr.z = @intFromBool((v & 0xFF) == 0);
            cpu.sr.v = 0;
            cpu.sr.c = 0;

            ea.writeResolved(cpu, b, res, .b, v | 0x80);
        },
        .chk => {
            const dn: u3 = @truncate(insn.word >> 9);
            const bound = ea.read(cpu, b, insn.src, .w);
            const value: i16 = @bitCast(@as(u16, @truncate(cpu.d[dn])));
            const limit: i16 = @bitCast(@as(u16, @truncate(bound)));

            if (value < 0 or value > limit) {
                cpu.sr.n = @intFromBool(value < 0);
                cpu.sr.z = 0;
                cpu.sr.v = 0;
                cpu.sr.c = 0;
                cpu.takeException(b, 6, cpu.pc);
                return;
            }

            cpu.sr.v = 0;
            cpu.sr.c = 0;
        },
        .movep => {
            const dn: u3 = @truncate(insn.word >> 9);
            const an: u3 = @truncate(insn.word);
            const disp = signExtend16(b.read16(cpu.pc));
            const addr = cpu.a[an] +% disp;
            cpu.pc +%= 2;

            const nbytes: u32 = if (insn.size == .l) 4 else 2;
            const to_mem = (insn.word & 0x0080) != 0;
            if (to_mem) {
                const shift_base: u5 = if (insn.size == .l) 24 else 8;
                var i: u32 = 0;
                while (i < nbytes) : (i += 1) {
                    const sh: u5 = @truncate(shift_base - 8 * i);
                    b.write8(addr +% (2 * i), @truncate(cpu.d[dn] >> sh));
                }
            } else {
                var acc: u32 = 0;
                var i: u32 = 0;
                while (i < nbytes) : (i += 1) {
                    acc = (acc << 8) | b.read8(addr +% (2 * i));
                }

                cpu.d[dn] = if (insn.size == .l)
                    acc
                else
                    (cpu.d[dn] & 0xFFFF_0000) | (acc & 0xFFFF);
            }
        },
        .movem => executeMovem(cpu, b, insn),
        .trap => {
            const v: u4 = @truncate(insn.word);

            cpu.takeException(b, 32 + @as(u32, v), cpu.pc);
        },
        .trapv => {
            if (cpu.sr.v != 0) cpu.takeException(b, 7, cpu.pc);
        },
        .unimplemented => {
            cpu.takeException(b, 4, cpu.pc -% 2);
        },
    }
}

fn executeMovem(cpu: anytype, b: anytype, insn: decode.Instruction) void {
    const an = insn.dst.reg;
    const ea_mode = insn.dst.mode;
    const w = insn.word;
    const is_load = (w & 0x0400) != 0;
    const size = insn.size;
    const step: u32 = if (size == .l) 4 else 2;
    const mask = b.read16(cpu.pc);
    cpu.pc +%= 2;

    if (!is_load and ea_mode == .predecrement) {
        var addr = cpu.a[an];
        var i: u4 = 0;
        while (true) : (i += 1) {
            if ((mask >> i) & 1 != 0) {
                const regsel: u4 = 15 - i;
                const val = regValue(cpu, regsel);

                addr -%= step;
                memWrite(cpu, b, addr, size, val);
            }

            if (i == 15) break;
        }

        cpu.a[an] = addr;
        return;
    }

    if (is_load and ea_mode == .postincrement) {
        var addr = cpu.a[an];
        var i: u4 = 0;
        while (true) : (i += 1) {
            if ((mask >> i) & 1 != 0) {
                const v = memRead(cpu, b, addr, size);

                addr +%= step;
                loadReg(cpu, i, size, v);
            }

            if (i == 15) break;
        }

        cpu.a[an] = addr;
        return;
    }

    var addr = ea.resolveAddress(cpu, b, insn.dst);
    var i: u4 = 0;
    while (true) : (i += 1) {
        if ((mask >> i) & 1 != 0) {
            if (is_load) {
                loadReg(cpu, i, size, memRead(cpu, b, addr, size));
            } else {
                memWrite(cpu, b, addr, size, regValue(cpu, i));
            }
            addr +%= step;
        }
        if (i == 15) break;
    }
}

fn regValue(cpu: anytype, n: u4) u32 {
    return if (n < 8) cpu.d[@as(u3, @truncate(n))] else cpu.a[@as(u3, @truncate(n - 8))];
}

fn loadReg(cpu: anytype, n: u4, size: decode.Size, value: u32) void {
    const v: u32 = if (size == .w) signExtend16(@truncate(value)) else value;

    if (n < 8) {
        cpu.d[@as(u3, @truncate(n))] = v;
    } else {
        cpu.a[@as(u3, @truncate(n - 8))] = v;
    }
}

fn memRead(cpu: anytype, b: anytype, addr: u32, size: decode.Size) u32 {
    return switch (size) {
        .w => cpu.rd16(b, addr),
        .l => cpu.rd32(b, addr),
        .b => unreachable,
    };
}

fn memWrite(cpu: anytype, b: anytype, addr: u32, size: decode.Size, value: u32) void {
    switch (size) {
        .w => cpu.wr16(b, addr, @truncate(value)),
        .l => cpu.wr32(b, addr, value),
        .b => unreachable,
    }
}

const ShiftResult = struct {
    value: u32,
    c: bool,
    v: bool,
    x: ?bool,
};

const ShiftType = enum { as, ls, rox, ro };

fn shiftType(tt: u2) ShiftType {
    return switch (tt) {
        0b00 => .as,
        0b01 => .ls,
        0b10 => .rox,
        0b11 => .ro,
    };
}

fn shiftCompute(
    ty: ShiftType,
    left: bool,
    count: u32,
    value: u32,
    n: u6,
    x_in: bool,
) ShiftResult {
    const msb: u5 = @truncate(n - 1);
    const full_mask: u32 = if (n == 32) 0xFFFF_FFFF else (@as(u32, 1) << @truncate(n)) - 1;

    if (count == 0) {
        const c0 = if (ty == .rox) x_in else false;
        return .{ .value = value & full_mask, .c = c0, .v = false, .x = null };
    }

    var v: u32 = value & full_mask;
    var carry: bool = false;
    var x: bool = x_in;
    var overflow: bool = false;
    const orig_sign = (v >> msb) & 1;

    if (ty == .as or ty == .ls) {
        carry = if (count > n)
            (ty == .as and !left and orig_sign != 0)
        else if (left)
            (v >> @as(u5, @truncate(n - count))) & 1 != 0
        else
            (v >> @as(u5, @truncate(count - 1))) & 1 != 0;
        x = carry;
    }

    var i: u32 = 0;
    while (i < count) : (i += 1) {
        switch (ty) {
            .as, .ls => {
                if (left) {
                    v = (v << 1) & full_mask;
                } else if (ty == .as) {
                    const sign = (v >> msb) & 1;
                    v = (v >> 1) | (sign << msb);
                } else {
                    v = v >> 1;
                }
            },
            .ro => {
                if (left) {
                    carry = ((v >> msb) & 1) != 0;
                    v = ((v << 1) | @intFromBool(carry)) & full_mask;
                } else {
                    carry = (v & 1) != 0;
                    v = (v >> 1) | (@as(u32, @intFromBool(carry)) << msb);
                }
            },
            .rox => {
                if (left) {
                    carry = ((v >> msb) & 1) != 0;
                    v = ((v << 1) | @intFromBool(x)) & full_mask;
                } else {
                    carry = (v & 1) != 0;
                    v = (v >> 1) | (@as(u32, @intFromBool(x)) << msb);
                }
                x = carry;
            },
        }
        if (ty == .as and left and ((v >> msb) & 1) != orig_sign) overflow = true;
    }

    const final_v = (ty == .as and left and overflow);
    return switch (ty) {
        .ro => .{ .value = v, .c = carry, .v = final_v, .x = null },
        .rox => .{ .value = v, .c = x, .v = final_v, .x = x },
        else => .{ .value = v, .c = carry, .v = final_v, .x = x },
    };
}

fn executeShift(cpu: anytype, b: anytype, insn: decode.Instruction) void {
    const w = insn.word;
    const left = (w & 0x0100) != 0;
    const memory_form = (w & 0x00C0) == 0x00C0;

    const ty: ShiftType = if (memory_form)
        shiftType(@truncate(w >> 9))
    else
        shiftType(@truncate(w >> 3));

    const count: u32 = if (memory_form)
        1
    else if ((w & 0x0020) != 0)
        cpu.d[@as(u3, @truncate(w >> 9))] % 64
    else blk: {
        const ccc: u3 = @truncate(w >> 9);
        break :blk if (ccc == 0) @as(u32, 8) else ccc;
    };

    const n = flags.bitWidth(insn.size);
    const x_in = cpu.sr.x != 0;
    const res = ea.resolve(cpu, b, insn.dst, insn.size);
    const val = ea.readResolved(cpu, b, res, insn.size);
    const r = shiftCompute(ty, left, count, val, n, x_in);

    ea.writeResolved(cpu, b, res, insn.size, r.value);

    cpu.sr.n = @intFromBool(flags.signBit(r.value, insn.size));
    cpu.sr.z = @intFromBool(flags.maskToSize(r.value, insn.size) == 0);
    cpu.sr.v = @intFromBool(r.v);
    cpu.sr.c = @intFromBool(r.c);

    if (r.x) |x| cpu.sr.x = @intFromBool(x);
}

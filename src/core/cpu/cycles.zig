const decode = @import("decode.zig");
const ea = @import("ea.zig");

pub const CYCLES_PER_FRAME: u32 = 166_667; // 10 mhz / 60 hz

fn eaCalcTime(mode: ea.AddressingMode) u32 {
    return switch (mode) {
        .data_reg_direct, .addr_reg_direct, .immediate => 0,
        .addr_reg_indirect, .postincrement => 4,
        .predecrement => 6,
        .displacement, .absolute_word, .pc_displacement => 8,
        .indexed, .pc_indexed => 10,
        .absolute_long => 12,
    };
}

fn opBase(op: decode.Op) u32 {
    return switch (op) {
        .move, .moveq => 4,
        .add, .sub => 4,
        .addx, .subx, .negx => 4,
        .abcd, .sbcd, .nbcd => 6,
        .addq, .subq, .addi, .subi => 4,
        .cmp, .cmpa, .cmpm, .cmpi => 4,
        .and_, .or_, .eor => 4,
        .andi, .ori, .eori => 4,
        .btst, .bchg, .bclr, .bset => 4,
        .shift => 6,
        .not, .neg, .clr, .tst => 4,
        .tas => 4,
        .chk => 10,
        .movep => 16,
        .ext, .swap => 4,
        .movem => 12,
        .bcc => 10,
        .jmp => 8,
        .jsr => 16,
        .rts, .rtr => 16,
        .nop => 4,
        .scc => 4,
        .dbcc => 10,
        .lea => 4,
        .pea => 12,
        .link => 16,
        .unlk => 12,
        .exg => 6,
        .mul => 70,
        .div => 140,
        .mull => 44,
        .divl => 90,
        .unimplemented => 4,
    };
}

pub fn cost(insn: decode.Instruction) u32 {
    return opBase(insn.op) + eaCalcTime(insn.src.mode) + eaCalcTime(insn.dst.mode);
}

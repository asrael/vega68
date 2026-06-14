const ea = @import("ea.zig");

pub const Instruction = struct {
    src: ea.EffectiveAddress = .{},
    dst: ea.EffectiveAddress = .{},

    op: Op = .unimplemented,
    size: Size = .w,
    word: u16,
};

pub const Op = enum {
    move, // also movea (size + dst mode disambiguate)
    moveq, // 0111 ddd0 iiiiiiii: sign-extended 8-bit immediate to Dn
    add, // also adda (dst .addr_reg_direct ⇒ no flags); line D
    sub, // also suba (dst .addr_reg_direct ⇒ no flags); line 9
    addq, // 0101 ddd0 ssEEEEEE: add quick immediate (An dst ⇒ no flags, full-32)
    subq, // 0101 ddd1 ssEEEEEE: sub quick immediate (An dst ⇒ no flags, full-32)
    addi, // 0000 0110 ssEEEEEE + imm: add immediate
    subi, // 0000 0100 ssEEEEEE + imm: sub immediate
    cmp, // 1011 RRR 0/1/2 EEEEEE: Dn - <ea>, sets NZVC (X preserved); line B
    cmpa, // 1011 RRR 3/7 EEEEEE: An - <ea>, full-32 (word src sign-extends), sets NZVC
    cmpm, // 1011 xxx 1 ss 001 yyy: (Ax)+ - (Ay)+, postincrement both, sets NZVC
    cmpi, // 0000 1100 ss EEEEEE + imm: <ea> - imm, sets NZVC (X preserved); line 0
    and_, // 1100 RRR 0-2/4-6 EEEEEE: Dn & <ea>; logic flags (NZ, V/C cleared); line C
    or_, // 1000 RRR 0-2/4-6 EEEEEE: Dn | <ea>; logic flags; line 8
    eor, // 1011 RRR 4-6 EEEEEE (EA≠001): Dn ^ <ea> -> <ea>; logic flags; line B
    andi, // 0000 0010 ss EEEEEE + imm: <ea> & imm -> <ea>; logic flags; line 0
    ori, // 0000 0000 ss EEEEEE + imm: <ea> | imm -> <ea>; logic flags; line 0
    eori, // 0000 1010 ss EEEEEE + imm: <ea> ^ imm -> <ea>; logic flags; line 0
    btst, // 0000 100000 EEEEEE +imm (static) / 0000 RRR 100 EEEEEE dynamic test bit (Z only)
    bchg, // ...01... test + toggle bit (Z only)
    bclr, // ...10... test + clear bit (Z only)
    bset, // ...11... test + set bit (Z only)
    shift, // line E: ASL/ASR/LSL/LSR/ROL/ROR/ROXL/ROXR (execute re-derives type/dir/count)
    not, // 0100 0110 ss EEEEEE: ~<ea> -> <ea>; logic flags (NZ, V/C cleared, X kept)
    neg, // 0100 0100 ss EEEEEE: 0 - <ea> -> <ea>; sets X/N/Z/V/C as "0 minus operand"
    clr, // 0100 0010 ss EEEEEE: 0 -> <ea>; Z=1 N=0 V=0 C=0 (X kept); dummy read first
    tst, // 0100 1010 ss EEEEEE: test <ea> vs 0; logic flags, no writeback (line 4)
    ext, // 0100 100 0/1 1 000 Dn: sign-extend byte→word (EXT.w) or word→long (EXT.l)
    swap, // 0100 1000 01 000 Dn: swap the upper/lower 16-bit halves of Dn (long flags)
    bcc, // 0110 CCCC dddddddd: Bcc/BRA (cc 0) / BSR (cc 1); execute re-derives cc + disp
    jmp, // 0100 1110 11 EEEEEE: pc = effective address (control modes) (no stack/flags)
    jsr, // 0100 1110 10 EEEEEE: push return addr (long), pc = effective address
    rts, // 0100 1110 0111 0101: pop long → pc
    rtr, // 0100 1110 0111 0111: pop word → CCR, pop long → pc
    rte, // 0100 1110 0111 0011: privileged; pop SR (word) + PC (long) from ISP
    nop, // 0100 1110 0111 0001: no operation
    scc, // 0101 CCCC 11 EEEEEE: dst byte = 0xFF if cc else 0x00 (mode ≠ 001)
    dbcc, // 0101 CCCC 11001 rrr + disp: decrement-and-branch (execute re-derives cc/disp)
    lea, // 0100 AAA 111 EEEEEE: An = effective ADDRESS (control modes) (no flags)
    pea, // 0100 1000 01 EEEEEE: push effective ADDRESS (long) (no flags)
    link, // 0100 1110 0101 0 AAA + disp — push An, An = SP, SP += signExt(disp) (no flags)
    unlk, // 0100 1110 0101 1 AAA: SP = An, then pop long → An (no flags)
    move_usp, // 0100 1110 0110 d AAA: MOVE An<->USP (privileged); bit 3 = direction
    exg, // 1100 XXX 1 ppppp YYY: swap a register pair (Dx,Dy / Ax,Ay / Dx,Ay) (no flags)
    mul, // 1100 RRR 011/111 EEEEEE: MULU/MULS 16×16→32 to Dn (word 8 of word picks signedness)
    div, // 1000 RRR 011/111 EEEEEE: DIVU/DIVS 32÷16 to Dn (word bit 8 picks signedness)
    mull, // 0100 1100 00 EEEEEE + ext: MULU.L/MULS.L 32×32→32/64 (68020); ext picks Dl/Dh/size/sign
    divl, // 0100 1100 01 EEEEEE + ext: DIVU.L/DIVS.L 32/64÷32 (68020); ext picks Dq/Dr/size/sign
    addx, // 1101 XXX 1 ss 00 M YYY: Dx = Dx + Dy + X (M=0) / -(Ax) += -(Ay) + X (M=1) (sticky Z)
    subx, // 1001 XXX 1 ss 00 M YYY: Dx = Dx - Dy - X (M=0) / -(Ax) -= -(Ay) + X (M=1) (sticky Z)
    negx, // 0100 0000 ss EEEEEE: 0 - <ea> - X -> <ea> (sticky Z, X/N/V/C as borrow)
    movem, // 0100 1 D 001 S EEEEEE + mask — move register set ↔ memory (no flags)
    abcd, // 1100 XXX 1 0000 M YYY: BCD add with X: Dx+Dy+X (M=0) / -(Ax)+-(Ay)+X (M=1)
    sbcd, // 1000 XXX 1 0000 M YYY: BCD subtract with X: Dx-Dy-X / -(Ax)--(Ay)-X
    nbcd, // 0100 1000 00 EEEEEE: BCD negate: 0 - <ea> - X -> <ea> (byte RMW)
    tas, // 0100 1010 11 EEEEEE: test byte (N/Z from original), then set bit 7 (byte RMW)
    chk, // 0100 DDD 110 EEEEEE: bound-check Dn.w against <ea>.w; out-of-bounds traps (vector 6)
    movep, // 0000 DDD 1 OO 001 AAA + disp: alternating-byte transfer An↔Dn (no flags)
    trap, // 0100 1110 0100 vvvv: TRAP #v (vector in low nibble of insn.word)
    trapv, // 0100 1110 0111 0110: trap on overflow (vector 7) when V set
    unimplemented,
};

pub const Size = enum { b, w, l };

pub fn decode(operation_word: u16) Instruction {
    return ea.classify(operation_word);
}

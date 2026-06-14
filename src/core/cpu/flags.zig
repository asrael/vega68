const decode = @import("decode.zig");
const m68k = @import("m68k.zig");

const Size = decode.Size;

fn addOverflow(am: u32, bm: u32, r: u32, n: u6) bool {
    const hi: u5 = @truncate(n - 1);
    return ((~(am ^ bm) & (am ^ r)) >> hi) & 1 != 0;
}

fn subOverflow(am: u32, bm: u32, r: u32, n: u6) bool {
    const hi: u5 = @truncate(n - 1);
    return (((am ^ bm) & (am ^ r)) >> hi) & 1 != 0;
}

fn subNZVC(cpu: anytype, a: u32, b: u32, size: Size) bool {
    const n = bitWidth(size);
    const am = maskToSize(a, size);
    const bm = maskToSize(b, size);
    const r: u32 = maskToSize(am -% bm, size);
    const borrow = am < bm;
    const overflow = subOverflow(am, bm, r, n);
    cpu.sr.n = @intFromBool(signBit(r, size));
    cpu.sr.z = @intFromBool(r == 0);
    cpu.sr.v = @intFromBool(overflow);
    cpu.sr.c = @intFromBool(borrow);
    return borrow;
}

pub fn bcdAdd(cpu: anytype, dst: u32, src: u32) u32 {
    const x_in: u32 = cpu.sr.x;
    const a: u32 = dst & 0xFF;
    const b: u32 = src & 0xFF;
    const lo: u32 = (a & 0x0F) + (b & 0x0F) + x_in;
    const corf: u32 = if (lo > 9) 6 else 0;
    const nocorf: u32 = lo + (a & 0xF0) + (b & 0xF0);
    var result: u32 = nocorf + corf;
    const carry = result > 0x99;
    if (carry) result += 0x60;
    cpu.sr.c = @intFromBool(carry);
    cpu.sr.x = @intFromBool(carry);
    cpu.sr.n = @intFromBool((result & 0x80) != 0);
    cpu.sr.v = @intFromBool((~nocorf & result & 0x80) != 0);
    if ((result & 0xFF) != 0) cpu.sr.z = 0;
    return result & 0xFF;
}

pub fn bcdSub(cpu: anytype, dst: u32, src: u32) u32 {
    const x_in: i32 = cpu.sr.x;
    const a: i32 = @intCast(dst & 0xFF);
    const b: i32 = @intCast(src & 0xFF);
    const lo: i32 = (a & 0x0F) - (b & 0x0F) - x_in;
    const corf: i32 = if (lo < 0) 6 else 0;
    const nocorf: i32 = (a & 0xF0) - (b & 0xF0) + lo;
    var result: i32 = nocorf - corf;
    const borrow = result < 0;
    if (borrow) result -= 0x60;
    const uno: u32 = @bitCast(nocorf);
    const ur: u32 = @bitCast(result);
    cpu.sr.c = @intFromBool(borrow);
    cpu.sr.x = @intFromBool(borrow);
    cpu.sr.n = @intFromBool((ur & 0x80) != 0);
    cpu.sr.v = @intFromBool((uno & ~ur & 0x80) != 0);
    if ((ur & 0xFF) != 0) cpu.sr.z = 0;
    return ur & 0xFF;
}

pub fn bitWidth(size: Size) u6 {
    return switch (size) {
        .b => 8,
        .w => 16,
        .l => 32,
    };
}

pub fn maskToSize(value: u32, size: Size) u32 {
    return switch (size) {
        .b => value & 0xFF,
        .w => value & 0xFFFF,
        .l => value,
    };
}

pub fn setFlagsAdd(cpu: anytype, a: u32, b: u32, size: Size) void {
    const n = bitWidth(size);
    const am = maskToSize(a, size);
    const bm = maskToSize(b, size);
    const wide = @as(u64, am) + @as(u64, bm);
    const r: u32 = maskToSize(@truncate(wide), size);
    const carry = (wide >> n) & 1 != 0;
    const overflow = addOverflow(am, bm, r, n);
    cpu.sr.n = @intFromBool(signBit(r, size));
    cpu.sr.z = @intFromBool(r == 0);
    cpu.sr.v = @intFromBool(overflow);
    cpu.sr.c = @intFromBool(carry);
    cpu.sr.x = @intFromBool(carry);
}

pub fn setNZ_clearVC(cpu: anytype, value: u32, size: Size) void {
    cpu.sr.n = @intFromBool(signBit(value, size));
    cpu.sr.z = @intFromBool(maskToSize(value, size) == 0);
    cpu.sr.v = 0;
    cpu.sr.c = 0;
}

pub fn setFlagsSub(cpu: anytype, a: u32, b: u32, size: Size) void {
    const borrow = subNZVC(cpu, a, b, size);
    cpu.sr.x = @intFromBool(borrow);
}

pub fn setFlagsCmp(cpu: anytype, a: u32, b: u32, size: Size) void {
    _ = subNZVC(cpu, a, b, size);
}

pub fn setFlagsAddX(cpu: anytype, a: u32, b: u32, size: Size) u32 {
    const x_in: u32 = cpu.sr.x;
    const n = bitWidth(size);
    const am = maskToSize(a, size);
    const bm = maskToSize(b, size);
    const wide = @as(u64, am) + @as(u64, bm) + @as(u64, x_in);
    const r: u32 = maskToSize(@truncate(wide), size);
    const carry = (wide >> n) & 1 != 0;
    const overflow = addOverflow(am, bm, r, n);
    cpu.sr.n = @intFromBool(signBit(r, size));
    if (r != 0) cpu.sr.z = 0;
    cpu.sr.v = @intFromBool(overflow);
    cpu.sr.c = @intFromBool(carry);
    cpu.sr.x = @intFromBool(carry);
    return r;
}

pub fn setFlagsSubX(cpu: anytype, a: u32, b: u32, size: Size) u32 {
    const x_in: u32 = cpu.sr.x;
    const n = bitWidth(size);
    const am = maskToSize(a, size);
    const bm = maskToSize(b, size);
    const r: u32 = maskToSize(am -% bm -% x_in, size);
    const borrow = @as(u64, am) < @as(u64, bm) + @as(u64, x_in);
    const overflow = subOverflow(am, bm, r, n);
    cpu.sr.n = @intFromBool(signBit(r, size));
    if (r != 0) cpu.sr.z = 0;
    cpu.sr.v = @intFromBool(overflow);
    cpu.sr.c = @intFromBool(borrow);
    cpu.sr.x = @intFromBool(borrow);
    return r;
}

pub fn signBit(value: u32, size: Size) bool {
    return switch (size) {
        .b => (value >> 7) & 1 != 0,
        .w => (value >> 15) & 1 != 0,
        .l => (value >> 31) & 1 != 0,
    };
}

pub fn testCondition(cc: u4, sr: m68k.StatusRegister) bool {
    const c = sr.c != 0;
    const v = sr.v != 0;
    const z = sr.z != 0;
    const n = sr.n != 0;
    return switch (cc) {
        0x0 => true, // T
        0x1 => false, // F
        0x2 => !c and !z, // HI
        0x3 => c or z, // LS
        0x4 => !c, // CC / HS
        0x5 => c, // CS / LO
        0x6 => !z, // NE
        0x7 => z, // EQ
        0x8 => !v, // VC
        0x9 => v, // VS
        0xA => !n, // PL
        0xB => n, // MI
        0xC => n == v, // GE
        0xD => n != v, // LT
        0xE => !z and (n == v), // GT
        0xF => z or (n != v), // LE
    };
}

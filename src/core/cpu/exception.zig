pub const Fault = struct {
    addr: u32, // access address (odd)
    ir: u16, // opcode word being processed
    rw: u1, // read=1, write=0
    fc: u3, // function code (supervisor data=5, user data=1, program=6/2)
    sr: u16, // SR snapshot at fault time (the aborted instruction may clobber CCR before draining)
};

pub const Vector = enum(u32) {
    bus_error = 2,
    address_error = 3,
    illegal = 4,
    zero_divide = 5,
    chk = 6,
    trapv = 7,
    privilege = 8,
};

pub fn ssw(f: Fault) u16 {
    return (f.ir & 0xFFE0) | (@as(u16, f.rw) << 4) | @as(u16, f.fc);
}

pub fn trapVector(n: u4) u32 {
    return 32 + @as(u32, n);
}

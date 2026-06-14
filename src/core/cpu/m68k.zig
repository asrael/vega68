//! motorola 68000 like cpu

const bus_mod = @import("../bus.zig");

pub const cycles = @import("cycles.zig");
pub const decode = @import("decode.zig");
pub const ea = @import("ea.zig");
pub const exception = @import("exception.zig");
pub const execute = @import("execute.zig");
pub const flags = @import("flags.zig");

pub const StatusRegister = packed struct(u16) {
    c: u1 = 0,
    v: u1 = 0,
    z: u1 = 0,
    n: u1 = 0,
    x: u1 = 0,
    _lo: u3 = 0,
    ipl: u3 = 0,
    _mid: u2 = 0,
    s: u1 = 0, // supervisor
    _hi: u2 = 0,
};

pub const CPU = struct {
    d: [8]u32 = @splat(0), // data registers D0–D7
    a: [8]u32 = @splat(0), // address registers A0–A7 (A7 = active SP)
    pc: u32 = 0,
    sr: StatusRegister = .{},
    ssp: u32 = 0, // supervisor stack pointer
    usp: u32 = 0, // user stack pointer
    stopped: bool = false,
    ir: u16 = 0, // opcode currently being processed
    fault: ?exception.Fault = null,

    fn raiseAddressFault(self: *CPU, addr: u32, rw: u1) void {
        if (self.fault != null) return;
        const fc: u3 = if (self.sr.s == 1) 5 else 1; // supervisor data=5 user data=1
        self.fault = .{ .addr = addr, .ir = self.ir, .rw = rw, .fc = fc, .sr = @bitCast(self.sr) };
    }

    pub fn rd16(self: *CPU, bus: anytype, addr: u32) u16 {
        if (addr & 1 != 0) self.raiseAddressFault(addr, 1);
        return bus.read16(addr);
    }

    pub fn rd32(self: *CPU, bus: anytype, addr: u32) u32 {
        if (addr & 1 != 0) self.raiseAddressFault(addr, 1);
        return bus.read32(addr);
    }

    pub fn reset(self: *CPU, b: *bus_mod.Bus) void {
        self.sr = .{ .s = 1, .ipl = 7 };
        self.a[7] = b.read32(0x000000);
        self.pc = b.read32(0x000004);
    }

    pub fn setSR(self: *CPU, value: u16) void {
        const new: StatusRegister = @bitCast(value);
        if (new.s != self.sr.s) {
            if (self.sr.s == 1) {
                self.ssp = self.a[7];
                self.a[7] = self.usp;
            } else {
                self.usp = self.a[7];
                self.a[7] = self.ssp;
            }
        }
        self.sr = new;
    }

    pub fn step(self: *CPU, bus: anytype) void {
        if (self.pc & 1 != 0) {
            const fc: u3 = if (self.sr.s == 1) 6 else 2; // supervisor program=6, user program=2
            self.fault = .{ .addr = self.pc, .ir = self.ir, .rw = 1, .fc = fc, .sr = @bitCast(self.sr) };
            self.takeAddressError(bus, self.pc);
            return;
        }

        const op_word = bus.read16(self.pc);
        self.ir = op_word;
        self.pc +%= 2;
        self.fault = null;
        const insn = decode.decode(op_word);
        execute.execute(self, bus, insn);
        if (self.fault != null) self.takeAddressError(bus, self.pc);
    }

    pub fn takeAddressError(self: *CPU, bus: anytype, stacked_pc: u32) void {
        const f = self.fault.?;
        const saved_sr: u16 = f.sr;
        var sys: u16 = saved_sr;
        sys |= 0x2000;
        sys &= ~@as(u16, 0x8000);
        self.setSR(sys);
        self.a[7] -%= 4;
        bus.write32(self.a[7], stacked_pc);
        self.a[7] -%= 2;
        bus.write16(self.a[7], saved_sr);
        self.a[7] -%= 2;
        bus.write16(self.a[7], f.ir);
        self.a[7] -%= 4;
        bus.write32(self.a[7], f.addr);
        self.a[7] -%= 2;
        bus.write16(self.a[7], exception.ssw(f));
        self.pc = bus.read32(@intFromEnum(exception.Vector.address_error) * 4);
    }

    pub fn takeException(self: *CPU, bus: anytype, vector: u32, stacked_pc: u32) void {
        const saved_sr: u16 = @bitCast(self.sr);
        var sys: u16 = saved_sr;
        sys |= 0x2000;
        sys &= ~@as(u16, 0x8000);
        self.setSR(sys);
        self.a[7] -%= 4;
        bus.write32(self.a[7], stacked_pc);
        self.a[7] -%= 2;
        bus.write16(self.a[7], saved_sr);
        self.pc = bus.read32(vector * 4);
    }

    pub fn wr16(self: *CPU, bus: anytype, addr: u32, val: u16) void {
        if (addr & 1 != 0) self.raiseAddressFault(addr, 0);
        bus.write16(addr, val);
    }

    pub fn wr32(self: *CPU, bus: anytype, addr: u32, val: u32) void {
        if (addr & 1 != 0) self.raiseAddressFault(addr, 0);
        bus.write32(addr, val);
    }
};

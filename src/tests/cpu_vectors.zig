//! each vector file is a JSON array of cases of the shape:
//!   { "name", "initial": {...}, "final": {...}, "length" }
//! where `initial`/`final` carry
//!   d0..d7, a0..a6, usp, ssp, sr, pc, prefetch[2], ram[[addr,byte]...]

const std = @import("std");
const core = @import("core");

const Value = std.json.Value;
const ObjectMap = std.json.ObjectMap;

pub const FlatMemory = struct {
    cells: std.AutoHashMap(u32, u8),

    const addr_mask: u32 = 0x00FF_FFFF;

    pub fn init(alloc: std.mem.Allocator) FlatMemory {
        return .{ .cells = std.AutoHashMap(u32, u8).init(alloc) };
    }
    pub fn deinit(self: *FlatMemory) void {
        self.cells.deinit();
    }
    pub fn clear(self: *FlatMemory) void {
        self.cells.clearRetainingCapacity();
    }

    pub fn read8(self: *FlatMemory, addr: u32) u8 {
        return self.cells.get(addr & addr_mask) orelse 0;
    }
    pub fn read16(self: *FlatMemory, addr: u32) u16 {
        return (@as(u16, self.read8(addr)) << 8) | self.read8(addr +% 1);
    }
    pub fn read32(self: *FlatMemory, addr: u32) u32 {
        return (@as(u32, self.read16(addr)) << 16) | self.read16(addr +% 2);
    }
    pub fn write8(self: *FlatMemory, addr: u32, val: u8) void {
        self.cells.put(addr & addr_mask, val) catch @panic("oom");
    }
    pub fn write16(self: *FlatMemory, addr: u32, val: u16) void {
        self.write8(addr, @truncate(val >> 8));
        self.write8(addr +% 1, @truncate(val));
    }
    pub fn write32(self: *FlatMemory, addr: u32, val: u32) void {
        self.write16(addr, @truncate(val >> 16));
        self.write16(addr +% 2, @truncate(val));
    }
};

pub const Profile = struct {
    pc_offset: i32 = 0,

    pub const musashi: Profile = .{ .pc_offset = 0 };
    pub const mame: Profile = .{ .pc_offset = -4 };
};

pub const vectors_68000 = [_][]const u8{
    "move_b",  "move_w",  "move_l",  "moveq",  "movea_w", "movea_l",
    "add_b",   "add_w",   "add_l",   "adda_w", "adda_l",  "sub_b",
    "sub_w",   "sub_l",   "suba_w",  "suba_l", "addx_b",  "addx_w",
    "addx_l",  "subx_b",  "subx_w",  "subx_l", "negx_b",  "negx_w",
    "negx_l",  "cmp_b",   "cmp_w",   "cmp_l",  "cmpa_w",  "cmpa_l",
    "and_b",   "and_w",   "and_l",   "or_b",   "or_w",    "or_l",
    "eor_b",   "eor_w",   "eor_l",   "btst",   "bset",    "bclr",
    "bchg",    "asl_b",   "asl_w",   "asl_l",  "asr_b",   "asr_w",
    "asr_l",   "lsl_b",   "lsl_w",   "lsl_l",  "lsr_b",   "lsr_w",
    "lsr_l",   "rol_b",   "rol_w",   "rol_l",  "ror_b",   "ror_w",
    "ror_l",   "roxl_b",  "roxl_w",  "roxl_l", "roxr_b",  "roxr_w",
    "roxr_l",  "not_b",   "not_w",   "not_l",  "neg_b",   "neg_w",
    "neg_l",   "clr_b",   "clr_w",   "clr_l",  "tst_b",   "tst_w",
    "tst_l",   "ext_w",   "ext_l",   "swap",   "bcc",     "bsr",
    "dbcc",    "scc",     "jmp",     "jsr",    "rts",     "rtr",
    "nop",     "mulu",    "muls",    "lea",    "pea",     "unlk",
    "exg",     "movem_w", "movem_l", "tas",    "chk",     "movep_w",
    "movep_l",
};

/// Truncate a JSON integer to an unsigned int of type T.
fn intAs(comptime T: type, v: Value) T {
    return @truncate(@as(u64, @bitCast(v.integer)));
}

fn field(comptime T: type, obj: *const ObjectMap, name: []const u8) T {
    return intAs(T, obj.get(name).?);
}

/// PC plus the profile's capture offset.
fn pcWithOffset(obj: *const ObjectMap, profile: Profile) u32 {
    return field(u32, obj, "pc") +% @as(u32, @bitCast(profile.pc_offset));
}

/// Decode a `[address, byte]` ram pair.
fn ramCell(cell: Value) struct { addr: u32, byte: u8 } {
    const pair = cell.array;
    return .{ .addr = intAs(u32, pair.items[0]), .byte = intAs(u8, pair.items[1]) };
}

/// The SR supervisor bit selects which stack pointer is live in A7.
fn activeA7(usp: u32, ssp: u32, sr: u16) u32 {
    return if (sr & 0x2000 != 0) ssp else usp;
}

pub fn resetMachine(cpu: *core.CPU, mem: *FlatMemory) void {
    cpu.d = @splat(0);
    cpu.a = @splat(0);
    cpu.pc = 0;
    cpu.sr = .{};
    cpu.usp = 0;
    cpu.ssp = 0;
    cpu.stopped = false;
    cpu.fault = null;
    mem.clear();
}

pub fn loadState(cpu: *core.CPU, mem: *FlatMemory, state: *const ObjectMap, profile: Profile) void {
    inline for (0..8) |i| {
        cpu.d[i] = field(u32, state, std.fmt.comptimePrint("d{d}", .{i}));
    }
    inline for (0..7) |i| {
        cpu.a[i] = field(u32, state, std.fmt.comptimePrint("a{d}", .{i}));
    }

    const sr = field(u16, state, "sr");
    cpu.sr = @bitCast(sr);
    const usp = field(u32, state, "usp");
    const ssp = field(u32, state, "ssp");
    cpu.usp = usp;
    cpu.ssp = ssp;
    cpu.a[7] = activeA7(usp, ssp, sr);
    cpu.pc = pcWithOffset(state, profile);

    const prefetch = state.get("prefetch").?.array;
    mem.write16(cpu.pc, intAs(u16, prefetch.items[0]));
    mem.write16(cpu.pc +% 2, intAs(u16, prefetch.items[1]));

    for (state.get("ram").?.array.items) |cell| {
        const c = ramCell(cell);
        mem.write8(c.addr, c.byte);
    }
}

// width is the hex digit count
fn mismatch(comptime label: []const u8, comptime width: usize, args: anytype, got: u64, want: u64) error{TestExpectedEqual} {
    const fmt = "[{s}] " ++ label ++ ": got {x:0>" ++ std.fmt.comptimePrint("{d}", .{width}) ++ "} want {x:0>" ++ std.fmt.comptimePrint("{d}", .{width}) ++ "}\n";
    std.debug.print(fmt, args ++ .{ got, want });
    return error.TestExpectedEqual;
}

pub fn assertState(cpu: *core.CPU, mem: *FlatMemory, final: *const ObjectMap, name: []const u8, profile: Profile) !void {
    inline for (0..8) |i| {
        const want = field(u32, final, std.fmt.comptimePrint("d{d}", .{i}));
        if (cpu.d[i] != want) return mismatch("d{d}", 8, .{ name, i }, cpu.d[i], want);
    }

    const sr = field(u16, final, "sr");
    inline for (0..7) |i| {
        const want = field(u32, final, std.fmt.comptimePrint("a{d}", .{i}));
        if (cpu.a[i] != want) return mismatch("a{d}", 8, .{ name, i }, cpu.a[i], want);
    }
    // a[7] holds the active bank; the inactive bank is in usp/ssp
    const want_usp = field(u32, final, "usp");
    const want_ssp = field(u32, final, "ssp");
    const got_usp = if (sr & 0x2000 != 0) cpu.usp else cpu.a[7];
    const got_ssp = if (sr & 0x2000 != 0) cpu.a[7] else cpu.ssp;
    if (got_usp != want_usp) return mismatch("usp", 8, .{name}, got_usp, want_usp);
    if (got_ssp != want_ssp) return mismatch("ssp", 8, .{name}, got_ssp, want_ssp);

    const got_sr: u16 = @bitCast(cpu.sr);
    if (got_sr != sr) return mismatch("sr", 4, .{name}, got_sr, sr);

    const pc = pcWithOffset(final, profile);
    if (cpu.pc != pc) return mismatch("pc", 8, .{name}, cpu.pc, pc);

    for (final.get("ram").?.array.items) |cell| {
        const c = ramCell(cell);
        const got = mem.read8(c.addr);
        if (got != c.byte) return mismatch("ram[{x:0>8}]", 2, .{ name, c.addr }, got, c.byte);
    }
}

fn ramLong(state: *const ObjectMap, addr: u32) u32 {
    const ram = state.get("ram").?.array;
    var bytes: [4]u8 = .{ 0, 0, 0, 0 };
    for (0..4) |i| {
        const want = (addr +% @as(u32, @intCast(i))) & 0x00FF_FFFF;
        for (ram.items) |cell| {
            const c = ramCell(cell);
            if ((c.addr & 0x00FF_FFFF) == want) {
                bytes[i] = c.byte;
                break;
            }
        }
    }
    return (@as(u32, bytes[0]) << 24) | (@as(u32, bytes[1]) << 16) |
        (@as(u32, bytes[2]) << 8) | bytes[3];
}

// Skips: address-error-masquerading CHK (odd EA → vector 3, Musashi pc=0),
// and A7-operand CHK encodings where oracles disagree.
// Genuine CHK traps (final pc == vector-6 handler) run and assert the 6-byte frame.
pub fn isChkTrap(initial: *const ObjectMap, final: *const ObjectMap, profile: Profile) bool {
    const op0 = intAs(u16, initial.get("prefetch").?.array.items[0]);
    const is_chk = (op0 & 0xF000) == 0x4000 and ((op0 >> 6) & 7) == 0b110;
    if (!is_chk) return false;
    if (field(u32, initial, "ssp") == field(u32, final, "ssp")) return false; // not trapping

    const chk_handler = ramLong(initial, 0x18);
    const is_genuine = chk_handler != 0 and pcWithOffset(final, profile) == chk_handler;
    if (!is_genuine) return true; // address-error-masquerading / garbage pc=0

    // -(A7)=100_111, (A7)+=011_111: oracles disagree even on genuine trap
    const ea = op0 & 0x3F;
    if (ea == 0x27 or ea == 0x1F) return true;
    return false; // genuine, run it
}

pub fn runFile(comptime path: []const u8) !void {
    return runFileProfile(path, Profile.musashi);
}

pub fn runSet68000(comptime dir: []const u8, profile: Profile) !void {
    inline for (vectors_68000) |f| try runFileProfile(dir ++ f ++ ".json", profile);
}

pub fn runFileProfile(comptime path: []const u8, profile: Profile) !void {
    const json = @embedFile(path);
    const alloc = std.testing.allocator;

    const parsed = try std.json.parseFromSlice(Value, alloc, json, .{});
    defer parsed.deinit();

    const cases = parsed.value.array;

    var cpu: core.CPU = .{};
    var mem = FlatMemory.init(alloc);
    defer mem.deinit();

    for (cases.items) |case_val| {
        const case = case_val.object;
        const name = case.get("name").?.string;
        const initial = case.get("initial").?.object;
        const final = case.get("final").?.object;

        if (isChkTrap(&initial, &final, profile)) continue;

        resetMachine(&cpu, &mem);
        loadState(&cpu, &mem, &initial, profile);
        cpu.step(&mem);
        // Musashi completes odd accesses; MAME's stacked PC is unpredictable — address errors
        // are out of oracle scope here; tested in exception_test.zig.
        // cpu.pc odd: address error fires on the next step (e.g. BSR odd target), diverging from oracle.
        if (cpu.fault != null or cpu.pc & 1 != 0) continue;
        try assertState(&cpu, &mem, &final, name, profile);
    }
}

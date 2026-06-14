//! musashi cpu test-vector generator
//! reads MAME_DIR, writes MUSASHI_DIR (paths are compile-time constants)

const std = @import("std");

const CPU_68000: c_uint = 1;
const CPU_68020: c_uint = 4;
const REG_D0: c_uint = 0; // D0..D7 = 0..7
const REG_A0: c_uint = 8; // A0..A6 = 8..14
const REG_PC: c_uint = 16;
const REG_SR: c_uint = 17;
const REG_USP: c_uint = 19;
const REG_ISP: c_uint = 20;

extern fn m68k_init() void;
extern fn m68k_set_cpu_type(cpu_type: c_uint) void;
extern fn m68k_pulse_reset() void;
extern fn m68k_execute(num_cycles: c_int) c_int;
extern fn m68k_set_reg(reg: c_uint, value: c_uint) void;
extern fn m68k_get_reg(context: ?*anyopaque, reg: c_uint) c_uint;

const MAME_DIR = "src/tests/vectors/mame";
const OUT_DIR = "src/tests/vectors/musashi";

const ADDR_MASK: u32 = 0x00FF_FFFF;
const MEM_SIZE: usize = 1 << 24;
var mem: [MEM_SIZE]u8 = undefined;
var touched: [4096]u32 = undefined;
var touched_n: usize = 0;

fn touch(a: u32) void {
    const m = a & ADDR_MASK;
    for (touched[0..touched_n]) |t| if (t == m) return;
    if (touched_n < touched.len) {
        touched[touched_n] = m;
        touched_n += 1;
    }
}

export fn m68k_read_memory_8(a: c_uint) c_uint {
    return mem[a & ADDR_MASK];
}
export fn m68k_read_memory_16(a: c_uint) c_uint {
    const hi: c_uint = mem[a & ADDR_MASK];
    return (hi << 8) | mem[(a +% 1) & ADDR_MASK];
}
export fn m68k_read_memory_32(a: c_uint) c_uint {
    return (m68k_read_memory_16(a) << 16) | m68k_read_memory_16(a +% 2);
}
export fn m68k_write_memory_8(a: c_uint, v: c_uint) void {
    mem[a & ADDR_MASK] = @truncate(v);
    touch(a);
}
export fn m68k_write_memory_16(a: c_uint, v: c_uint) void {
    mem[a & ADDR_MASK] = @truncate(v >> 8);
    touch(a);
    mem[(a +% 1) & ADDR_MASK] = @truncate(v);
    touch(a +% 1);
}
export fn m68k_write_memory_32(a: c_uint, v: c_uint) void {
    m68k_write_memory_16(a, v >> 16);
    m68k_write_memory_16(a +% 2, v);
}
export fn m68k_read_disassembler_16(a: c_uint) c_uint {
    return m68k_read_memory_16(a);
}
export fn m68k_read_disassembler_32(a: c_uint) c_uint {
    return m68k_read_memory_32(a);
}

fn poke16(addr: u32, val: u16) void {
    mem[addr & ADDR_MASK] = @truncate(val >> 8);
    mem[(addr +% 1) & ADDR_MASK] = @truncate(val);
}

const State = struct {
    d: [8]u32,
    a: [7]u32,
    usp: u32,
    ssp: u32,
    sr: u32,
    pc: u32,
};

fn capture() State {
    var s: State = undefined;
    inline for (0..8) |i| s.d[i] = m68k_get_reg(null, REG_D0 + i);
    inline for (0..7) |i| s.a[i] = m68k_get_reg(null, REG_A0 + i);
    s.usp = m68k_get_reg(null, REG_USP);
    s.ssp = m68k_get_reg(null, REG_ISP);
    s.sr = m68k_get_reg(null, REG_SR) & 0xFFFF;
    s.pc = m68k_get_reg(null, REG_PC);
    return s;
}

const OPCODE_PC: u32 = 0x2000;

var rng_state: u32 = 0x1357BD13;
fn rng() u32 {
    var x = rng_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng_state = x;
    return x;
}

fn randOperand() u32 {
    return switch (rng() % 8) {
        0 => rng() % 16,
        1 => 0xFFFFFFFF - (rng() % 16),
        2 => @as(u32, 1) << @intCast(rng() % 32),
        3 => 0x80000000,
        4 => 0x7FFFFFFF,
        else => rng(),
    };
}

fn makeExt(rhi: u32, rlo: u32, signed: bool, is64: bool) u16 {
    const s: u16 = if (signed) 0x0800 else 0;
    const l: u16 = if (is64) 0x0400 else 0;
    return (@as(u16, @intCast(rlo & 7)) << 12) | s | l | @as(u16, @intCast(rhi & 7));
}

fn loadRegs68020(d: [8]u32, a: [7]u32) void {
    m68k_set_reg(REG_SR, 0x2000);
    m68k_set_reg(REG_USP, 0x4000);
    m68k_set_reg(REG_ISP, 0x5000);
    inline for (0..8) |i| m68k_set_reg(REG_D0 + i, d[i]);
    inline for (0..7) |i| m68k_set_reg(REG_A0 + i, a[i]);
    m68k_set_reg(REG_PC, OPCODE_PC);
}

fn runCase(op: u16, ext: u16, d: [8]u32, a: [7]u32, init: *State, fin: *State) ?i32 {
    @memset(mem[0..0x10000], 0);
    poke16(OPCODE_PC, op);
    poke16(OPCODE_PC + 2, ext);
    loadRegs68020(d, a);
    init.* = capture();
    const cycles = m68k_execute(1);
    fin.* = capture();
    if (fin.pc != OPCODE_PC + 4) return null;
    return cycles;
}

fn emitState020(w: anytype, s: *const State, op: u16, ext: u16) !void {
    try w.writeByte('{');
    inline for (0..8) |i| try w.print("\"d{d}\":{d},", .{ i, s.d[i] });
    inline for (0..7) |i| try w.print("\"a{d}\":{d},", .{ i, s.a[i] });
    try w.print("\"usp\":{d},\"ssp\":{d},\"sr\":{d},\"pc\":{d},", .{ s.usp, s.ssp, s.sr, s.pc });
    try w.print("\"prefetch\":[{d},{d}],\"ram\":[]}}", .{ op, ext });
}

fn genFile(w: anytype, mnem: []const u8, opcode: u16, signed: bool, is_div: bool, count: u32) !void {
    try w.writeAll("[\n");
    var emitted: u32 = 0;
    var n: u32 = 0;
    var guard: u32 = 0;
    while (emitted < count and guard < count * 50) {
        guard += 1;
        const is64 = (n & 1) == 1;
        const rlo = rng() % 8;
        var rhi = rng() % 8;
        while (rhi == rlo) rhi = rng() % 8;
        var rsrc = rng() % 8;
        while (rsrc == rlo or rsrc == rhi) rsrc = rng() % 8;
        const op: u16 = opcode | @as(u16, @intCast(rsrc));
        const ext = makeExt(rhi, rlo, signed, is64);

        var d: [8]u32 = @splat(0);
        var a: [7]u32 = @splat(0);
        for (0..7) |i| a[i] = randOperand();
        for (0..8) |i| d[i] = randOperand();
        const src = randOperand();
        d[rsrc] = src;
        d[rlo] = randOperand();
        if (is_div and is64) d[rhi] = randOperand();
        if (is_div and src == 0) continue;

        var init: State = undefined;
        var fin: State = undefined;
        const cycles = runCase(op, ext, d, a, &init, &fin) orelse continue;

        if (emitted != 0) try w.writeAll(",\n");
        var namebuf: [96]u8 = undefined;
        const name = try std.fmt.bufPrint(&namebuf, "{s}.{s} rsrc=D{d} Dl=D{d} Dh=D{d} #{d}", .{
            mnem, if (is64) "64" else "32", rsrc, rlo, rhi, emitted,
        });
        try w.print("  {{ \"name\": \"{s}\", \"initial\": ", .{name});
        try emitState020(w, &init, op, ext);
        try w.writeAll(", \"final\": ");
        try emitState020(w, &fin, op, ext);
        try w.print(", \"length\": {d} }}", .{cycles});
        emitted += 1;
        n += 1;
    }
    try w.writeAll("\n]\n");
}

fn phaseHarness(io: std.Io, alloc: std.mem.Allocator) !void {
    @memset(&mem, 0);
    m68k_set_cpu_type(CPU_68020);
    m68k_pulse_reset();
    _ = m68k_execute(0);

    const files = .{
        .{ "mulu_l.json", "MULU", @as(u16, 0x4C00), false, false },
        .{ "muls_l.json", "MULS", @as(u16, 0x4C00), true, false },
        .{ "divu_l.json", "DIVU", @as(u16, 0x4C40), false, true },
        .{ "divs_l.json", "DIVS", @as(u16, 0x4C40), true, true },
    };
    inline for (files) |f| {
        var aw = std.Io.Writer.Allocating.init(alloc);
        defer aw.deinit();
        try genFile(&aw.writer, f[1], f[2], f[3], f[4], 30);
        try flush(io, alloc, f[0], aw.writer.buffer[0..aw.writer.end]);
    }
}

fn u32v(v: std.json.Value) u32 {
    return @truncate(@as(u64, @bitCast(v.integer)));
}

fn emitRam(w: anytype, cells: []const [2]u32) !void {
    try w.writeByte('[');
    for (cells, 0..) |c, i| {
        if (i != 0) try w.writeByte(',');
        try w.print("[{d},{d}]", .{ c[0], c[1] });
    }
    try w.writeByte(']');
}

fn lessCell(_: void, x: [2]u32, y: [2]u32) bool {
    return x[0] < y[0];
}

fn replayCase(w: anytype, alloc: std.mem.Allocator, case: std.json.Value, first: bool) !void {
    const initial = case.object.get("initial").?.object;
    const d = blk: {
        var arr: [8]u32 = undefined;
        inline for (0..8) |i| arr[i] = u32v(initial.get(std.fmt.comptimePrint("d{d}", .{i})).?);
        break :blk arr;
    };
    const a = blk: {
        var arr: [7]u32 = undefined;
        inline for (0..7) |i| arr[i] = u32v(initial.get(std.fmt.comptimePrint("a{d}", .{i})).?);
        break :blk arr;
    };
    const usp = u32v(initial.get("usp").?);
    const ssp = u32v(initial.get("ssp").?);
    const sr = u32v(initial.get("sr").?);
    const pc = u32v(initial.get("pc").?); // MAME m_au = opcode + 4
    const opaddr = (pc -% 4) & ADDR_MASK;
    const pf = initial.get("prefetch").?.array;
    const p0: u16 = @truncate(@as(u64, @bitCast(pf.items[0].integer)));
    const p1: u16 = @truncate(@as(u64, @bitCast(pf.items[1].integer)));

    for (touched[0..touched_n]) |t| mem[t] = 0;
    touched_n = 0;

    const in_ram = initial.get("ram").?.array;
    for (in_ram.items) |cell| {
        const addr = u32v(cell.array.items[0]) & ADDR_MASK;
        mem[addr] = @truncate(u32v(cell.array.items[1]));
        touch(addr);
    }
    poke16(opaddr, p0);
    touch(opaddr);
    touch(opaddr +% 1);
    poke16(opaddr +% 2, p1);
    touch(opaddr +% 2);
    touch(opaddr +% 3);

    m68k_set_reg(REG_SR, sr);
    m68k_set_reg(REG_USP, usp);
    m68k_set_reg(REG_ISP, ssp);
    inline for (0..8) |i| m68k_set_reg(REG_D0 + i, d[i]);
    inline for (0..7) |i| m68k_set_reg(REG_A0 + i, a[i]);
    m68k_set_reg(REG_PC, opaddr);

    _ = m68k_execute(1);
    const fin = capture();

    var init_ram = try alloc.alloc([2]u32, in_ram.items.len);
    defer alloc.free(init_ram);
    for (in_ram.items, 0..) |cell, i| {
        init_ram[i] = .{ u32v(cell.array.items[0]), u32v(cell.array.items[1]) };
    }
    std.mem.sort([2]u32, init_ram, {}, lessCell);

    var fin_ram = try alloc.alloc([2]u32, touched_n);
    defer alloc.free(fin_ram);
    for (touched[0..touched_n], 0..) |addr, i| fin_ram[i] = .{ addr, mem[addr] };
    std.mem.sort([2]u32, fin_ram, {}, lessCell);

    const length: i64 = if (case.object.get("length")) |l| l.integer else 0;

    if (!first) try w.writeAll(",\n");
    try w.print("  {{\"name\":\"{s}\",\"initial\":{{", .{case.object.get("name").?.string});
    inline for (0..8) |i| try w.print("\"d{d}\":{d},", .{ i, d[i] });
    inline for (0..7) |i| try w.print("\"a{d}\":{d},", .{ i, a[i] });
    try w.print("\"usp\":{d},\"ssp\":{d},\"sr\":{d},\"pc\":{d},\"prefetch\":[{d},{d}],\"ram\":", .{ usp, ssp, sr, opaddr, p0, p1 });
    try emitRam(w, init_ram);
    try w.writeAll("},\"final\":{");
    inline for (0..8) |i| try w.print("\"d{d}\":{d},", .{ i, fin.d[i] });
    inline for (0..7) |i| try w.print("\"a{d}\":{d},", .{ i, fin.a[i] });
    try w.print("\"usp\":{d},\"ssp\":{d},\"sr\":{d},\"pc\":{d},\"ram\":", .{ fin.usp, fin.ssp, fin.sr, fin.pc });
    try emitRam(w, fin_ram);
    try w.print("}},\"length\":{d}}}", .{length});
}

fn phaseCrossref(io: std.Io, alloc: std.mem.Allocator) !void {
    @memset(&mem, 0);
    m68k_set_cpu_type(CPU_68000);
    m68k_pulse_reset();
    _ = m68k_execute(0);

    var names = std.ArrayList([]const u8).empty;
    defer {
        for (names.items) |n| alloc.free(n);
        names.deinit(alloc);
    }
    var dir = try std.Io.Dir.cwd().openDir(io, MAME_DIR, .{ .iterate = true });
    defer dir.close(io);
    var it = dir.iterate();
    while (try it.next(io)) |e| {
        if (e.kind == .file and std.mem.endsWith(u8, e.name, ".json"))
            try names.append(alloc, try alloc.dupe(u8, e.name));
    }
    std.mem.sort([]const u8, names.items, {}, struct {
        fn less(_: void, x: []const u8, y: []const u8) bool {
            return std.mem.lessThan(u8, x, y);
        }
    }.less);

    for (names.items) |fname| {
        const path = try std.fmt.allocPrint(alloc, "{s}/{s}", .{ MAME_DIR, fname });
        defer alloc.free(path);
        const bytes = try std.Io.Dir.cwd().readFileAlloc(io, path, alloc, .unlimited);
        defer alloc.free(bytes);
        const parsed = try std.json.parseFromSlice(std.json.Value, alloc, bytes, .{});
        defer parsed.deinit();

        var aw = std.Io.Writer.Allocating.init(alloc);
        defer aw.deinit();
        const w = &aw.writer;
        try w.writeAll("[\n");
        for (parsed.value.array.items, 0..) |case, i| try replayCase(w, alloc, case, i == 0);
        try w.writeAll("\n]\n");
        try flush(io, alloc, fname, aw.writer.buffer[0..aw.writer.end]);
    }
}

fn flush(io: std.Io, alloc: std.mem.Allocator, name: []const u8, bytes: []const u8) !void {
    const path = try std.fmt.allocPrint(alloc, "{s}/{s}", .{ OUT_DIR, name });
    defer alloc.free(path);
    try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = path, .data = bytes });
    std.debug.print("wrote {s} ({d} bytes)\n", .{ path, bytes.len });
}

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    const alloc = init.gpa;
    m68k_init();
    try phaseHarness(io, alloc);
    try phaseCrossref(io, alloc);
}

const std = @import("std");

const MAX_STEPS_PER_FRAME: u32 = 4_000_000;

pub const hw = @import("hw");
pub const memmap = hw.memmap;
pub const cpu = @import("cpu/m68k.zig");
pub const bus = @import("bus.zig");
pub const ppu = @import("ppu/ppu.zig");
pub const apu = @import("apu/apu.zig");
pub const input = @import("input.zig");
pub const testpattern = @import("testpattern.zig");

pub const Bus = bus.Bus;
pub const CPU = cpu.CPU;
pub const PPU = ppu.PPU;
pub const APU = apu.APU;
pub const Input = input.Input;
pub const PadState = input.PadState;

pub const SCREEN_W = 320;
pub const SCREEN_H = 240;

pub const Shell = struct {
    loadFile: *const fn (path: []const u8, buf: []u8) anyerror!usize,
    pollInput: *const fn (pads: *[2]PadState) void,
    present: *const fn (fb: []const u32, w: u32, h: u32) void,
    renderAudio: *const fn (out: []i16) void,
    shouldQuit: *const fn () bool,
    sleep: *const fn (ms: u32) void,
};

pub const System = struct {
    apu: APU = .{},
    bus: Bus = .{},
    cpu: CPU = .{},
    ppu: PPU = .{},

    framebuffer: [SCREEN_W * SCREEN_H]u32 = @splat(0),
    input: Input = .{},

    pub fn create(allocator: std.mem.Allocator) !*System {
        const self = try allocator.create(System);
        self.* = .{};
        self.bus.ppu = &self.ppu;
        self.bus.input = &self.input;
        return self;
    }

    pub fn destroy(self: *System, allocator: std.mem.Allocator) void {
        allocator.destroy(self);
    }

    pub fn fillTestPattern(self: *System, frame: u32) void {
        testpattern.fill(&self.framebuffer, frame);
    }

    pub fn loadCart(self: *System, cart: []const u8) void {
        self.bus.cart = cart;
    }

    pub fn loadROM(self: *System, rom: []const u8) void {
        self.bus.rom = rom;
    }

    pub fn reset(self: *System) void {
        self.cpu.reset(&self.bus);
    }

    pub fn runFrame(self: *System) void {
        self.bus.vsync = false;
        var i: u32 = 0;
        while (i < MAX_STEPS_PER_FRAME) : (i += 1) {
            if (self.bus.exited or self.cpu.stopped or self.bus.vsync) break;
            self.cpu.step(&self.bus);
        }
        var line: u16 = 0;
        while (line < SCREEN_H) : (line += 1) {
            const row = self.framebuffer[@as(usize, line) * SCREEN_W ..][0..SCREEN_W];
            self.ppu.renderScanline(line, row);
        }
    }

    pub fn runUntilExit(self: *System, max_steps: usize) void {
        self.cpu.reset(&self.bus);
        var i: usize = 0;
        while (i < max_steps) : (i += 1) {
            if (self.bus.exited or self.cpu.stopped) break;
            self.cpu.step(&self.bus);
        }
    }
};

pub fn run(system: *System, shell: Shell) void {
    system.reset();
    while (!shell.shouldQuit()) {
        shell.pollInput(&system.input.pads);
        system.runFrame();
        shell.present(&system.framebuffer, SCREEN_W, SCREEN_H);
        if (system.bus.exited) break;
        shell.sleep(16);
    }
}

test {
    std.testing.refAllDecls(@This());
}

//! vega68 host emulator entry: load a ROM (argv[1], default hello.vro) and run it on the m68k core.

const std = @import("std");
const core = @import("core");
const shell = @import("shell");

fn sink(fd: u32, bytes: []const u8) void {
    _ = fd;
    std.debug.print("{s}", .{bytes});
}

pub fn main(init: std.process.Init) !void {
    var args = try std.process.Args.Iterator.initAllocator(init.minimal.args, init.gpa);
    defer args.deinit();
    _ = args.next(); // program name
    const cart_path = args.next() orelse "zig-out/bin/sprite.vro";

    // vegaOS boot ROM + cartridge
    const vos = try std.Io.Dir.cwd().readFileAlloc(init.io, "zig-out/bin/vos.vro", init.gpa, .unlimited);
    defer init.gpa.free(vos);
    const cart = try std.Io.Dir.cwd().readFileAlloc(init.io, cart_path, init.gpa, .unlimited);
    defer init.gpa.free(cart);

    const system = try core.System.create(init.gpa);
    defer system.destroy(init.gpa);

    system.loadROM(vos); // vegaOS at $0
    system.loadCart(cart); // cartridge at $00400000
    system.bus.host_write = &sink; // console output (text carts) -> stdout
    shell.run(system);
}

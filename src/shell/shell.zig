const core = @import("core");
const std = @import("std");

pub const scale = @import("scale.zig");

const win32 = struct {
    const audio = @import("win32/audio.zig");
    const file = @import("win32/file.zig");
    const input = @import("win32/input.zig");
    const window = @import("win32/window.zig");
};

pub fn interface() core.Shell {
    return .{
        .loadFile = &win32.file.loadFile,
        .pollInput = &win32.input.pollInput,
        .present = &win32.window.present,
        .renderAudio = &win32.audio.renderAudio,
        .shouldQuit = &win32.window.shouldQuit,
        .sleep = &win32.window.sleep,
    };
}

pub fn init() !void {
    try win32.window.create();
}

pub fn deinit() void {
    win32.window.destroy();
}

pub fn run(system: *core.System) void {
    init() catch |err| {
        std.debug.print("vega68: failed to open window: {s}\n", .{@errorName(err)});
        return;
    };
    defer deinit();

    core.run(system, interface());
}

test {
    std.testing.refAllDecls(@This());
}

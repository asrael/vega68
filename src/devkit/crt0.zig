const io = @import("io.zig");

extern fn main() void;

export fn _start() callconv(.c) noreturn {
    main();
    io.exit(0);
}

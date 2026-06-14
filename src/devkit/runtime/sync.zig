const abi = @import("../abi.zig");

pub fn waitForVblank() void {
    abi.VSYNC.* = 1;
}

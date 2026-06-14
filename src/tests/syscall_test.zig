//! Runs the real BIOS + abitest cart on the core; asserts exit 0 + output.
//! Embeds m68k ROMs (needs cross toolchain); gated as `test-abi`.

const std = @import("std");
const core = @import("core");

const vos_rom = @embedFile("vos.vro");
const abitest_cart = @embedFile("abitest.vro");

var cap: [256]u8 = undefined;
var cap_len: usize = 0;
fn sink(fd: u32, bytes: []const u8) void {
    _ = fd;
    for (bytes) |b| {
        if (cap_len < cap.len) {
            cap[cap_len] = b;
            cap_len += 1;
        }
    }
}

test "syscall: BIOS runs abitest cart to exit 0" {
    cap_len = 0;
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.loadROM(vos_rom);
    sys.loadCart(abitest_cart);
    sys.bus.host_write = &sink;
    sys.runUntilExit(2_000_000);

    try std.testing.expect(sys.bus.exited);
    try std.testing.expectEqual(@as(u32, 0), sys.bus.status); // 0 = every on-device check passed
    try std.testing.expectEqualStrings("hello, world\n", cap[0..cap_len]);
}

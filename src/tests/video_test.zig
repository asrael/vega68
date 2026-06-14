//! Boots the real BIOS + sprite cart on the core and asserts the sprite renders
//! and tracks input. Embeds m68k ROMs (needs cross toolchain); gated as `test-video`.

const std = @import("std");
const core = @import("core");

const vos_rom = @embedFile("vos.vro");
const sprite_cart = @embedFile("sprite.vro");

const W = core.SCREEN_W;

fn px(fb: []const u32, x: usize, y: usize) u32 {
    return fb[y * W + x];
}

fn hostColor(rgb: u32) u32 {
    return core.ppu.palette.toHost(@bitCast(core.hw.Color.fromU32(rgb)));
}

test "video: sprite boots, renders, and moves right with input" {
    const sys = try core.System.create(std.testing.allocator);
    defer sys.destroy(std.testing.allocator);
    sys.loadROM(vos_rom);
    sys.loadCart(sprite_cart);
    sys.reset();

    const sprite = hostColor(0xFB4934);
    const backdrop = hostColor(0x101820);

    // Frame 0, no input: 16×16 sprite spawns at (152,112).
    sys.runFrame();
    try std.testing.expect(!sys.bus.exited);
    try std.testing.expectEqual(sprite, px(&sys.framebuffer, 160, 120)); // sprite center
    try std.testing.expectEqual(backdrop, px(&sys.framebuffer, 8, 8)); // far corner

    // Hold RIGHT for 20 frames: +2 px/frame -> x 152..192.
    sys.input.pads[0].buttons = @as(u16, 1) << @intFromEnum(core.hw.Button.right);
    for (0..20) |_| sys.runFrame();

    try std.testing.expectEqual(sprite, px(&sys.framebuffer, 200, 120)); // new center (192+8)
    try std.testing.expectEqual(backdrop, px(&sys.framebuffer, 160, 120)); // vacated spawn column
}

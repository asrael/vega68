//! Win32 input. Keyboard maps to player 1's digital pad. XInput / analog deferred.

const core = @import("core");

const Button = core.input.Button;

extern "user32" fn GetAsyncKeyState(vKey: i32) callconv(.winapi) i16;

const VK_RETURN: i32 = 0x0D;
const VK_LEFT: i32 = 0x25;
const VK_UP: i32 = 0x26;
const VK_RIGHT: i32 = 0x27;
const VK_DOWN: i32 = 0x28;
const VK_X: i32 = 0x58;
const VK_Z: i32 = 0x5A;

fn down(vk: i32) bool {
    return (@as(u16, @bitCast(GetAsyncKeyState(vk))) & 0x8000) != 0;
}

/// Latch the keyboard into player 1's pad; player 2 idle.
pub fn pollInput(pads: *[2]core.PadState) void {
    const map = .{
        .{ VK_UP, Button.up },
        .{ VK_DOWN, Button.down },
        .{ VK_LEFT, Button.left },
        .{ VK_RIGHT, Button.right },
        .{ VK_Z, Button.a },
        .{ VK_X, Button.b },
        .{ VK_RETURN, Button.start },
    };
    var b: u16 = 0;
    inline for (map) |m| {
        if (down(m[0])) b |= @as(u16, 1) << @intFromEnum(m[1]);
    }
    pads[0].buttons = b;
    pads[1] = .{};
}

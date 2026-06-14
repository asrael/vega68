const abi = @import("../abi.zig");

pub const Button = @import("hw").Button;

pub fn pad(player: u1) u16 {
    return abi.PAD[player];
}

pub fn pressed(player: u1, btn: Button) bool {
    return (pad(player) >> @intFromEnum(btn)) & 1 != 0;
}

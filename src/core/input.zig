pub const Button = @import("hw").Button;

pub const PadState = struct {
    buttons: u16 = 0,
    caps: u16 = 0,
    lx: i16 = 0,
    ly: i16 = 0,
    rx: i16 = 0,
    ry: i16 = 0,
    lt: u8 = 0,
    rt: u8 = 0,
};

pub const Input = struct {
    pads: [2]PadState = .{ .{}, .{} },

    pub fn pressed(self: *const Input, player: u1, btn: Button) bool {
        return (self.pads[player].buttons >> @intFromEnum(btn)) & 1 != 0;
    }
};

pub const Color = packed struct(u16) {
    b: u5 = 0,
    g: u5 = 0,
    r: u5 = 0,
    _reserved: u1 = 0,

    pub fn fromU32(rgb: u32) Color {
        return .{
            .r = @intCast((rgb >> 19) & 0x1F),
            .g = @intCast((rgb >> 11) & 0x1F),
            .b = @intCast((rgb >> 3) & 0x1F),
        };
    }
};

pub fn pxl(rgb: u32) u16 {
    return @bitCast(Color.fromU32(rgb));
}

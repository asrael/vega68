const abi = @import("../abi.zig");
const pxl = @import("hw").color.pxl;

pub const GRUVBOX: []const u32 = &.{
    0x1D2021, // bg0_h
    0x282828, // bg0
    0x3C3836, // bg1
    0x504945, // bg2
    0x665C54, // bg3
    0x928374, // gray
    0xA89984, // fg4
    0xBDAE93, // fg3
    0xD5C4A1, // fg2
    0xEBDBB2, // fg1
    0xFBF1C7, // fg0
    0xCC241D, // red0
    0xFB4934, // red1
    0x98971A, // green0
    0xB8BB26, // green1
    0xD79921, // yellow0
    0xFABD2F, // yellow
    0x458588, // blue0
    0x83A598, // blue1
    0xB16286, // purple0
    0xD3869B, // purple1
    0x689D6A, // aqua0
    0x8EC07C, // aqua1
    0xD65D0E, // orange0
    0xFE8019, // orange1
};

pub fn load(src: []const u32) void {
    const n = @min(src.len, 256);
    for (src[0..n], 0..) |rgb, i| abi.CRAM[i] = pxl(rgb);
}

pub fn set(index: u8, rgb: u32) void {
    abi.CRAM[index] = pxl(rgb);
}

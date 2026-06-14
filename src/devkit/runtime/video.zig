const abi = @import("../abi.zig");
const palette = @import("palette.zig");

pub const Config = struct {
    layers: u8 = 2,
    palette: []const u32 = palette.GRUVBOX,
};

pub fn init(cfg: Config) void {
    _ = cfg.layers;
    palette.load(cfg.palette);
}

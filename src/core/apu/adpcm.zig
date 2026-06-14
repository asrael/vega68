pub const Voice = struct {
    cursor: usize = 0,
    sample: []const u8 = &.{},
    playing: bool = false,

    pub fn step(self: *Voice) i16 {
        _ = self;

        @panic("TODO: adpcm nibble to pcm");
    }
};

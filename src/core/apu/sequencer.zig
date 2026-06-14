pub const Sequencer = struct {
    playing: bool = false,
    song: []const u8 = &.{},
    cursor: usize = 0,

    pub fn play(self: *Sequencer, song: []const u8) void {
        _ = self;
        _ = song;

        @panic("TODO: load a song and start playback");
    }

    pub fn tick(self: *Sequencer, apu: anytype) void {
        _ = self;
        _ = apu;

        @panic("TODO: emit this frame's queued register writes");
    }
};

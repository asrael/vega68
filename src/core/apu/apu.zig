const fm = @import("fm.zig");
const adpcm = @import("adpcm.zig");
const sequencer = @import("sequencer.zig");

pub const APU = struct {
    adpcm_voices: [8]adpcm.Voice = @splat(.{}),
    fm_voices: [8]fm.Voice = @splat(.{}),
    seq: sequencer.Sequencer = .{},

    pub fn mix(self: *APU, out: []i16) void {
        _ = self;
        _ = out;

        @panic("TODO: mix fm and adpcm voices into the output buffer");
    }

    pub fn writeReg(self: *APU, reg: u16, val: u16) void {
        _ = self;
        _ = reg;
        _ = val;

        @panic("TODO: route to fm / adpcm / sequencer register files");
    }
};

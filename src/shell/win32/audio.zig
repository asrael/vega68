//! Win32 audio output. Output thread (WASAPI, waveOut fallback) pulls interleaved
//! 16-bit samples from the APU mixer.

/// Render the next audio block into `out`. Interleaved 16-bit samples. Audio thread.
pub fn renderAudio(out: []i16) void {
    _ = out;
    @panic("TODO(milestone 6): WASAPI render thread -> APU.mix");
}

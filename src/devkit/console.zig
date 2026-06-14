//! guest text console
//! append to a buffer and stream to a pluggable sink

const io = @import("io.zig");

var sink: ?Sink = null;
var buffer: [4096]u8 = undefined;
var len: usize = 0;

pub const Sink = *const fn (bytes: []const u8) void;

fn kernelSink(bytes: []const u8) void {
    _ = io.write(1, bytes);
}

pub fn contents() []const u8 {
    return buffer[0..len];
}

pub fn print(s: []const u8) void {
    for (s) |c| {
        if (len < buffer.len) {
            buffer[len] = c;
            len += 1;
        }
    }
    (sink orelse kernelSink)(s);
}

pub fn println(s: []const u8) void {
    print(s + "\n");
}

pub fn setSink(s: Sink) void {
    sink = s;
}

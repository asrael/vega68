//! Win32 file I/O. Loads host files (cartridge ROM image, save data) into a
//! caller-provided buffer.

/// Read the file at `path` into `buf`. Returns bytes read.
pub fn loadFile(path: []const u8, buf: []u8) anyerror!usize {
    _ = path;
    _ = buf;
    @panic("TODO(milestone 1): read file into buf, return byte count");
}

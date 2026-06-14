pub const ABI_VERSION: u16 = 1;
pub const FD_CONSOLE: u32 = 1;
pub const TRAP_VECTOR: u4 = 0;

pub const Err = struct {
    pub const badcall: u32 = @bitCast(@as(i32, -1));
    pub const badfd: u32 = @bitCast(@as(i32, -2));
};

pub const Syscall = enum(u32) {
    exit = 0,
    write = 1,
    version = 2,
    _,
};

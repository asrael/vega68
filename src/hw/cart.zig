pub const CART_MAGIC: u32 = 0x56473638;

comptime {
    if (@sizeOf(CartHeader) != 64) @compileError("CartHeader must be 64 bytes");
}

pub const CartHeader = extern struct {
    magic: u32,
    entry: u32,
    size: u32,
    title: [32]u8,
    abi_version: u16,
    reserved: [18]u8,
};

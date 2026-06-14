const hw = @import("hw");

pub const memmap = hw.memmap;

pub const CRAM: *volatile [256]u16 = @ptrFromInt(memmap.CRAM_BASE);
pub const PAD: *volatile [2]u16 = @ptrFromInt(memmap.SYS_REGS_BASE);
pub const PPU: *volatile PPURegs = @ptrFromInt(memmap.PPU_REGS_BASE);
pub const SPRITES: *volatile [128]Sprite = @ptrFromInt(memmap.SPRITE_ATTR_BASE);
pub const VSYNC: *volatile u16 = @ptrFromInt(memmap.VSYNC_REG);

pub const Color = hw.Color;
pub const Err = hw.syscall.Err;
pub const PPURegs = hw.PPURegs;
pub const Sprite = hw.Sprite;
pub const Syscall = hw.Syscall;
pub const TilemapEntry = hw.TilemapEntry;

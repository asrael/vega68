const memmap = @import("hw").memmap;
const Input = @import("input.zig").Input;
const PPU = @import("ppu/ppu.zig").PPU;

pub const Bus = struct {
    cart: []const u8 = &.{},
    exited: bool = false,
    host_write: ?*const fn (fd: u32, bytes: []const u8) void = null,
    input: ?*Input = null,
    ppu: ?*PPU = null,
    ram: [memmap.WORK_RAM_SIZE]u8 = @splat(0),
    rom: []const u8 = &.{},
    status: u32 = 0,
    vsync: bool = false,

    fn ramIndex(addr: u32) ?usize {
        const base = memmap.WORK_RAM_BASE;
        if (addr >= base and addr < base + memmap.WORK_RAM_SIZE) return addr - base;

        return null;
    }

    fn cartIndex(addr: u32) ?usize {
        const base = memmap.CART_BASE;
        if (addr >= base and addr < base + memmap.CART_SIZE) return addr - base;
        return null;
    }

    fn cramOffset(addr: u32) ?usize {
        const base = memmap.CRAM_BASE;
        if (addr >= base and addr < base + memmap.CRAM_SIZE) return addr - base;
        return null;
    }

    fn oamOffset(addr: u32) ?usize {
        const base = memmap.SPRITE_ATTR_BASE;
        if (addr >= base and addr < base + memmap.SPRITE_ATTR_SIZE) return addr - base;
        return null;
    }

    fn padOffset(addr: u32) ?usize {
        const base = memmap.SYS_REGS_BASE;
        if (addr >= base and addr < base + 4) return addr - base;
        return null;
    }

    pub fn read8(self: *const Bus, addr: u32) u8 {
        if (addr < self.rom.len) return self.rom[addr];
        if (cartIndex(addr)) |i| return if (i < self.cart.len) self.cart[i] else 0;
        if (ramIndex(addr)) |i| return self.ram[i];

        if (self.input) |inp| {
            if (padOffset(addr)) |off| {
                const b = inp.pads[off / 2].buttons;
                return if (off % 2 == 0) @intCast(b >> 8) else @truncate(b);
            }
        }

        return 0;
    }

    pub fn read16(self: *const Bus, addr: u32) u16 {
        return (@as(u16, self.read8(addr)) << 8) | self.read8(addr +% 1);
    }

    pub fn read32(self: *const Bus, addr: u32) u32 {
        return (@as(u32, self.read16(addr)) << 16) | self.read16(addr +% 2);
    }

    pub fn write8(self: *Bus, addr: u32, val: u8) void {
        if (addr == memmap.CONSOLE_REG) {
            if (self.host_write) |sink| sink(1, &.{val});
            return;
        }
        if (addr == memmap.VSYNC_REG or addr == memmap.VSYNC_REG + 1) {
            self.vsync = true;
            return;
        }
        if (self.ppu) |ppu| {
            if (oamOffset(addr)) |off| {
                const shift: u6 = @intCast((7 - off % 8) * 8);
                var v: u64 = @bitCast(ppu.oam[off / 8]);
                v = (v & ~(@as(u64, 0xFF) << shift)) | (@as(u64, val) << shift);
                ppu.oam[off / 8] = @bitCast(v);
                return;
            }
            if (cramOffset(addr)) |off| {
                const e = &ppu.cram[off / 2];
                e.* = if (off % 2 == 0)
                    (e.* & 0x00FF) | (@as(u16, val) << 8)
                else
                    (e.* & 0xFF00) | val;
                return;
            }
        }
        if (ramIndex(addr)) |i| self.ram[i] = val;
    }

    pub fn write16(self: *Bus, addr: u32, val: u16) void {
        self.write8(addr, @truncate(val >> 8));
        self.write8(addr +% 1, @truncate(val));
    }

    pub fn write32(self: *Bus, addr: u32, val: u32) void {
        if (addr == memmap.EXIT_REG) {
            self.exited = true;
            self.status = val;
            return;
        }
        self.write16(addr, @truncate(val >> 16));
        self.write16(addr +% 2, @truncate(val));
    }
};

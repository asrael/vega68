use m68k::{AddressBus, FastMem};

pub const BIOS_SIZE: u32 = 0x0010_0000; // 1 MiB window
pub const CART_BASE: u32 = 0x0100_0000;
pub const CART_SIZE: u32 = 0x0100_0000; // 16 MiB max
pub const RAM_BASE: u32 = 0x0200_0000;
pub const RAM_SIZE: u32 = 0x0040_0000; // 4 MiB
pub const BIOS_PARTITION: u32 = 0x0002_0000; // 128 KiB at the base of RAM
pub const STACK_TOP: u32 = RAM_BASE + BIOS_PARTITION;
pub const VRAM_BASE: u32 = 0x0300_0000;
pub const VRAM_SIZE: u32 = 0x0008_0000; // 512 KiB
pub const PALETTE_BASE: u32 = 0x0308_0000;
pub const PALETTE_SIZE: u32 = 0x0000_0400; // 256 entries x 4 B
pub const MEM_END: u32 = PALETTE_BASE + PALETTE_SIZE;
pub const MMIO_BASE: u32 = 0xFF00_0000;

pub const VDP_STATUS: u32 = 0xFF00_0000;
pub const IRQ_ENABLE: u32 = 0xFF00_0004;
pub const IRQ_ACK: u32 = 0xFF00_0008;
pub const LINE_COMPARE: u32 = 0xFF00_000C;
pub const BRIGHTNESS: u32 = 0xFF00_0010;
pub const PAD_1: u32 = 0xFF00_0100;
pub const PAD_2: u32 = 0xFF00_0104;
pub const DEBUG_PUTC: u32 = 0xFF00_0200;

pub const PAD_UP: u16 = 0x0001;
pub const PAD_DOWN: u16 = 0x0002;
pub const PAD_LEFT: u16 = 0x0004;
pub const PAD_RIGHT: u16 = 0x0008;
pub const PAD_A: u16 = 0x0010;
pub const PAD_B: u16 = 0x0020;
pub const PAD_X: u16 = 0x0040;
pub const PAD_Y: u16 = 0x0080;
pub const PAD_START: u16 = 0x0100;
pub const PAD_SELECT: u16 = 0x0200;
pub const PAD_L: u16 = 0x0400;
pub const PAD_R: u16 = 0x0800;

pub const VISIBLE_LINES: u32 = 180;
pub const LINES_PER_FRAME: u32 = 200;

const DEBUG_OUT_CAP: usize = 64 * 1024;

pub struct Bus {
    pub brightness: u8,
    pub debug_out: Vec<u8>,
    pub irq_enable: u16,
    pub irq_pending: u16,
    pub line: u32,
    pub line_compare: u16,
    pub mem: Vec<u8>,
    pub pads: [u16; 2],
}

impl Bus {
    pub fn new(bios: Vec<u8>) -> Self {
        assert!(bios.len() as u32 <= BIOS_SIZE, "BIOS image exceeds window");

        let mut mem = vec![0u8; MEM_END as usize];
        mem[..bios.len()].copy_from_slice(&bios);

        Self {
            brightness: 255,
            debug_out: Vec::new(),
            irq_enable: 0,
            irq_pending: 0,
            line: 0,
            line_compare: 0,
            mem,
            pads: [0; 2],
        }
    }

    fn mmio_reg(&self, slot: u32) -> u16 {
        match slot {
            VDP_STATUS => (((self.line >= VISIBLE_LINES) as u16) << 15) | self.line as u16,
            IRQ_ENABLE => self.irq_enable,
            IRQ_ACK => self.irq_pending,
            LINE_COMPARE => self.line_compare,
            BRIGHTNESS => self.brightness as u16,
            PAD_1 => self.pads[0],
            PAD_2 => self.pads[1],
            _ => 0,
        }
    }

    fn read(&self, address: u32, bytes: usize) -> u32 {
        let mut v = 0u32;

        for i in 0..bytes {
            let a = address.wrapping_add(i as u32);
            let b = if a >= MMIO_BASE {
                *self
                    .mmio_reg(a & !3)
                    .to_be_bytes()
                    .get((a & 3) as usize)
                    .unwrap_or(&0)
            } else {
                *self.mem.get(a as usize).unwrap_or(&0)
            };
            v = (v << 8) | b as u32;
        }

        v
    }

    fn write(&mut self, address: u32, value: u32, bytes: usize) {
        if address >= MMIO_BASE {
            match address & !3 {
                IRQ_ENABLE => self.irq_enable = value as u16 & 0b11,
                IRQ_ACK => self.irq_pending &= !(value as u16),
                LINE_COMPARE => self.line_compare = value as u16,
                BRIGHTNESS => self.brightness = value as u8,
                DEBUG_PUTC => {
                    if self.debug_out.len() < DEBUG_OUT_CAP {
                        self.debug_out.push(value as u8);
                    }

                    eprint!("{}", (value as u8) as char);
                }
                _ => {}
            }

            return;
        }

        for i in 0..bytes {
            let a = address.wrapping_add(i as u32) as usize;

            if let Some(b) = self.mem.get_mut(a) {
                *b = (value >> (8 * (bytes - 1 - i))) as u8;
            }
        }
    }
}

impl AddressBus for Bus {
    fn fast_mem(&mut self) -> Option<FastMem> {
        Some(FastMem {
            ptr: self.mem.as_mut_ptr(),
            base: 0,
            len: MEM_END,
        })
    }

    fn read_byte(&mut self, a: u32) -> u8 {
        self.read(a, 1) as u8
    }

    fn read_word(&mut self, a: u32) -> u16 {
        self.read(a, 2) as u16
    }

    fn read_long(&mut self, a: u32) -> u32 {
        self.read(a, 4)
    }

    fn write_byte(&mut self, a: u32, v: u8) {
        self.write(a, v as u32, 1)
    }

    fn write_word(&mut self, a: u32, v: u16) {
        self.write(a, v as u32, 2)
    }

    fn write_long(&mut self, a: u32, v: u32) {
        self.write(a, v, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus() -> Bus {
        Bus::new(vec![0; 8])
    }

    #[test]
    fn status_reflects_line_and_vblank() {
        let mut b = bus();

        b.line = 42;
        assert_eq!(b.read_word(VDP_STATUS), 42);

        b.line = 185;
        assert_eq!(b.read_word(VDP_STATUS), 0x8000 | 185);
    }

    #[test]
    fn pads_and_brightness_read_back() {
        let mut b = bus();

        b.pads = [0x0123, 0x0800];

        assert_eq!(b.read_word(PAD_1), 0x0123);
        assert_eq!(b.read_word(PAD_2), 0x0800);

        b.write_word(BRIGHTNESS, 0x77);

        assert_eq!(b.read_word(BRIGHTNESS), 0x77);
    }

    #[test]
    fn debug_putc_is_captured() {
        let mut b = bus();

        for c in b"hi!" {
            b.write_word(DEBUG_PUTC, *c as u16);
        }

        assert_eq!(b.debug_out, b"hi!");
    }

    #[test]
    fn debug_out_stops_at_cap() {
        let mut b = bus();

        for _ in 0..DEBUG_OUT_CAP + 10 {
            b.write_word(DEBUG_PUTC, b'x' as u16);
        }

        assert_eq!(b.debug_out.len(), DEBUG_OUT_CAP);
    }

    #[test]
    fn irq_ack_clears_pending() {
        let mut b = bus();

        b.irq_pending = 0b11;
        b.write_word(IRQ_ACK, 0b01);

        assert_eq!(b.irq_pending, 0b10);
        assert_eq!(b.read_word(IRQ_ACK), 0b10);
    }
}

use m68k::{CpuCore, CpuType, CycleBatchExit};

use crate::bus::{Bus, CART_BASE, LINES_PER_FRAME, VISIBLE_LINES};
use crate::cart::{self, CartError};
use crate::vdp;

/// 100 MHz / 60 fps.
pub const CYCLES_PER_FRAME: u64 = 1_666_667;

pub struct System {
    pub bus: Bus,
    pub cpu: CpuCore,
}

impl System {
    pub fn new(bios: &[u8], cart: &[u8]) -> Result<System, CartError> {
        cart::parse(cart)?;

        let mut bus = Bus::new(bios.to_vec());
        let mut cpu = CpuCore::new();

        bus.mem[CART_BASE as usize..CART_BASE as usize + cart.len()].copy_from_slice(cart);
        cpu.set_cpu_type(CpuType::M68040);
        cpu.reset(&mut bus);

        Ok(System { cpu, bus })
    }

    pub fn run_frame(&mut self) {
        const CYCLES_PER_LINE: i64 = (CYCLES_PER_FRAME / LINES_PER_FRAME as u64) as i64;

        'frame: for line in 0..LINES_PER_FRAME {
            self.bus.line = line;

            if line == VISIBLE_LINES && self.bus.irq_enable & 1 != 0 {
                self.bus.irq_pending |= 1;
            }

            if line == self.bus.line_compare as u32 && self.bus.irq_enable & 2 != 0 {
                self.bus.irq_pending |= 2;
            }

            let level = match self.bus.irq_pending {
                p if p & 1 != 0 => 6,
                p if p & 2 != 0 => 4,
                _ => 0,
            };
            self.cpu.set_irq(level);

            let mut cycles = 0i64;

            while cycles < CYCLES_PER_LINE {
                let budget = (CYCLES_PER_LINE - cycles).min(i32::MAX as i64) as i32;
                let r = self.cpu.run_for_cycles(&mut self.bus, budget);

                let taken = match r.exit {
                    CycleBatchExit::IllegalInstruction { .. } => {
                        self.cpu.take_illegal_exception(&mut self.bus)
                    }
                    CycleBatchExit::AlineTrap { .. } => {
                        self.cpu.take_aline_exception(&mut self.bus)
                    }
                    CycleBatchExit::FlineTrap { .. } => {
                        self.cpu.take_fline_exception(&mut self.bus)
                    }
                    CycleBatchExit::Breakpoint { .. } => {
                        self.cpu.take_bkpt_exception(&mut self.bus)
                    }
                    CycleBatchExit::TrapInstruction { trap_num } => {
                        self.cpu.take_trap_exception(&mut self.bus, trap_num)
                    }
                    _ => 0,
                };

                if r.cycles + taken <= 0 {
                    break 'frame; // halted/stuck: don't spin forever
                }

                cycles += (r.cycles + taken) as i64;
            }
        }
    }

    pub fn render(&self, out: &mut [u32]) {
        vdp::render(&self.bus.mem, self.bus.brightness, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{HEADER_LEN, test_bios, test_cart};

    #[test]
    fn boots_bios_vectors() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();

        m.run_frame();

        assert_eq!(m.bus.brightness, 0x7F);
    }

    #[test]
    fn surfaced_traps_vector_through_the_table() {
        const HANDLER: usize = 0x100;

        for (opcode, vector) in [([0x4a, 0xfc], 4usize), ([0x4e, 0x40], 32)] {
            let entry = CART_BASE + HEADER_LEN as u32;
            let mut bios = vec![0u8; 0x200];

            bios[0..4].copy_from_slice(&crate::bus::STACK_TOP.to_be_bytes());
            bios[4..8].copy_from_slice(&8u32.to_be_bytes());
            bios[8..10].copy_from_slice(&[0x4e, 0xf9]); // jmp entry
            bios[10..14].copy_from_slice(&entry.to_be_bytes());
            bios[vector * 4..vector * 4 + 4].copy_from_slice(&(HANDLER as u32).to_be_bytes());
            bios[HANDLER..HANDLER + 4].copy_from_slice(&[0x33, 0xfc, 0x00, 0x21]); // move.w #'!'
            bios[HANDLER + 4..HANDLER + 8].copy_from_slice(&crate::bus::DEBUG_PUTC.to_be_bytes());
            bios[HANDLER + 8..HANDLER + 10].copy_from_slice(&[0x60, 0xfe]); // bra.s *

            let cart = test_cart(&[opcode[0], opcode[1], 0x60, 0xfe]);
            let mut m = System::new(&bios, &cart).unwrap();

            m.run_frame();

            assert_eq!(
                m.bus.debug_out, b"!",
                "vector {vector} was never taken; the opcode ran as a no-op"
            );
        }
    }

    #[test]
    fn cart_payload_lands_at_window() {
        let code = b"\x60\xfe\0\0";
        let m = System::new(&test_bios(), &test_cart(code)).unwrap();

        assert_eq!(
            &m.bus.mem[CART_BASE as usize + HEADER_LEN..][..code.len()],
            code
        );
    }
}

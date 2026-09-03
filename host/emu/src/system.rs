use crate::apu;
use crate::bus::{Bus, CART_BASE, LINES_PER_FRAME, VISIBLE_LINES};
use crate::cart::{self, CartError};
use crate::cpu::Cpu;
use crate::vdp;

pub const LINE_CYCLES: i32 = 9_033;

const FRAME_SAMPLES: usize = LINES_PER_FRAME as usize * apu::SAMPLES_PER_LINE * 2;
const RESET_RELOAD: u16 = 2;

pub struct System {
    pub bus: Bus,
    pub cpu: Cpu,
    cycles: i32,
    frame: Vec<u32>,
}

impl System {
    pub fn new(bios: &[u8], cart: &[u8]) -> Result<System, CartError> {
        cart::parse(cart)?;

        let mut bus = Bus::new(bios.to_vec());
        let mut cpu = Cpu::new();

        bus.mem[CART_BASE as usize..CART_BASE as usize + cart.len()].copy_from_slice(cart);
        cpu.reset(&mut bus);

        Ok(System {
            bus,
            cpu,
            cycles: 0,
            frame: vec![0; vdp::WIDTH * vdp::HEIGHT],
        })
    }

    pub fn reload(&mut self, cart: &[u8]) -> Result<(), CartError> {
        cart::parse(cart)?;

        self.bus.mem[CART_BASE as usize..CART_BASE as usize + cart.len()].copy_from_slice(cart);

        self.bus.apu.reset();
        self.bus.irq_enable = 0;
        self.bus.irq_pending = 0;
        self.bus.line_compare = 0;
        self.bus.line_interval = 0;
        self.bus.brightness = 255;
        self.bus.reset_reason = RESET_RELOAD;

        self.cpu.reset(&mut self.bus);

        Ok(())
    }

    pub fn render(&self, out: &mut [u32]) {
        out[..vdp::WIDTH * vdp::HEIGHT].copy_from_slice(&self.frame);
    }

    pub fn run_frame(&mut self) {
        for line in 0..LINES_PER_FRAME {
            self.bus.line = line;

            if line == VISIBLE_LINES && self.bus.irq_enable & 1 != 0 {
                self.bus.irq_pending |= 1;
            }

            if line_irq_fires(self.bus.line_compare, self.bus.line_interval, line)
                && self.bus.irq_enable & 2 != 0
            {
                self.bus.irq_pending |= 2;
            }

            let level = match self.bus.irq_pending {
                p if p & 1 != 0 => 6,
                p if p & 2 != 0 => 4,
                _ => 0,
            };
            self.cpu.set_irq(level);

            self.cycles += LINE_CYCLES;

            if self.cycles > 0 {
                let used = self.cpu.run(&mut self.bus, self.cycles as u32);
                self.cycles -= used as i32;
            }

            self.run_line(line);
        }

        debug_assert_eq!(self.bus.apu.frame.len(), FRAME_SAMPLES);
    }

    fn run_line(&mut self, line: u32) {
        let Bus { apu, mem, .. } = &mut self.bus;
        apu.run_line(mem, line);

        if line < VISIBLE_LINES {
            vdp::render_line(
                &self.bus.mem,
                self.bus.brightness,
                line as usize,
                &mut self.frame,
            );
        }
    }
}

fn line_irq_fires(compare: u16, interval: u16, line: u32) -> bool {
    let compare = compare as u32;

    match interval {
        0 => line == compare,
        n => {
            let n = n as u32;
            line >= compare && line < VISIBLE_LINES && (line - compare).is_multiple_of(n)
        }
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

        for (opcode, vector) in [
            ([0x4a, 0xfc], 4usize), // illegal
            ([0x4e, 0x40], 32),     // trap #0
            ([0xa0, 0x00], 10),     // A-line
            ([0xf2, 0x00], 11),     // F-line: fmove on a core with no FPU
        ] {
            let entry = CART_BASE + HEADER_LEN as u32;
            let mut bios = vec![0u8; 0x200];

            bios[0..4].copy_from_slice(&crate::bus::STACK_TOP.to_be_bytes());
            bios[4..8].copy_from_slice(&8u32.to_be_bytes());
            bios[8..10].copy_from_slice(&[0x4e, 0xf9]); // jmp cart entry
            bios[10..14].copy_from_slice(&entry.to_be_bytes());
            bios[vector * 4..vector * 4 + 4].copy_from_slice(&(HANDLER as u32).to_be_bytes());
            bios[HANDLER..HANDLER + 4].copy_from_slice(&[0x33, 0xfc, 0x00, 0x21]); // move.w #'!', DEBUG_PUTC
            bios[HANDLER + 4..HANDLER + 8].copy_from_slice(&crate::bus::DEBUG_PUTC.to_be_bytes());
            bios[HANDLER + 8..HANDLER + 10].copy_from_slice(&[0x60, 0xfe]); // bra.s .

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
    fn line_irq_fires_matches_the_worked_table() {
        let cases: [(u16, u16, u32); 7] = [
            (185, 0, 1),
            (40, 0, 1),
            (40, 2, 70),
            (41, 2, 70),
            (0, 1, 180),
            (170, 5, 2),
            (185, 2, 0),
        ];

        for (compare, interval, want) in cases {
            let count = (0..LINES_PER_FRAME)
                .filter(|&line| line_irq_fires(compare, interval, line))
                .count();

            assert_eq!(count as u32, want, "compare={compare} interval={interval}");
        }
    }

    #[test]
    fn vblank_sets_pending_only_when_enabled() {
        for (irq_enable, want_pending) in [(1u16, true), (0u16, false)] {
            let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();
            m.bus.irq_enable = irq_enable;

            m.run_frame();

            assert_eq!(
                m.bus.irq_pending & 1 != 0,
                want_pending,
                "vblank pending bit wrong for irq_enable={irq_enable:#x}"
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

    #[test]
    fn reload_replaces_cart_bytes_and_sets_the_reload_reason() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();
        let new_cart = test_cart(&[0x4e, 0x71, 0x60, 0xfe]);

        m.reload(&new_cart).unwrap();

        assert_eq!(
            m.bus.reset_reason, 2,
            "reset_reason is not V68_RESET_RELOAD"
        );
        assert_eq!(
            &m.bus.mem[CART_BASE as usize..CART_BASE as usize + new_cart.len()],
            &new_cart[..],
            "cart window was not replaced with the new image"
        );
    }

    #[test]
    fn reload_preserves_ram_across_the_reset() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();
        let noinit_addr = crate::bus::RAM_BASE as usize + crate::bus::BIOS_PARTITION as usize;

        m.bus.mem[noinit_addr] = 0xAB;

        m.reload(&test_cart(&[0x60, 0xfe])).unwrap();

        assert_eq!(
            m.bus.mem[noinit_addr], 0xAB,
            "reload touched RAM outside the cart window"
        );
    }

    #[test]
    fn reload_resets_device_registers_to_bus_new_values() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();

        m.bus.irq_enable = 0b11;
        m.bus.irq_pending = 0b11;
        m.bus.line_compare = 123;
        m.bus.line_interval = 5;
        m.bus.brightness = 10;
        m.bus.write_u8(crate::bus::AUDIO_BASE, 0xFF);

        m.reload(&test_cart(&[0x60, 0xfe])).unwrap();

        assert_eq!(m.bus.irq_enable, 0, "irq_enable not reset");
        assert_eq!(m.bus.irq_pending, 0, "irq_pending not reset");
        assert_eq!(m.bus.line_compare, 0, "line_compare not reset");
        assert_eq!(m.bus.line_interval, 0, "line_interval not reset");
        assert_eq!(m.bus.brightness, 255, "brightness not reset");
        assert_eq!(
            m.bus.read_u8(crate::bus::AUDIO_BASE),
            0,
            "apu register not reset"
        );
    }

    #[test]
    fn run_frame_produces_a_full_audio_frame() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();

        m.run_frame();
        assert_eq!(m.bus.apu.frame.len(), 1600);
    }

    #[test]
    fn a_cart_write_to_the_apu_is_heard_the_same_frame() {
        #[rustfmt::skip]
        let code = [
            0x33, 0xFC, 0x00, 0xFE, 0xFF, 0x00, 0x06, 0x00, // move.w #0xFE, V68_AUDIO_CH(8)
            0x42, 0x39, 0xFF, 0x00, 0x06, 0x02,             // clr.b  V68_AUDIO_CH(8)+2
            0x60, 0xFE,                                     // bra.s  .
        ];

        let mut m = System::new(&test_bios(), &test_cart(&code)).unwrap();
        m.run_frame();

        assert!(
            m.bus.apu.frame.iter().any(|&s| s != 0),
            "square never reached the mix"
        );
    }

    #[test]
    fn a_mid_frame_brightness_write_lands_on_its_own_line() {
        #[rustfmt::skip]
        let code = [
            0x30, 0x39, 0xFF, 0x00, 0x00, 0x00, // loop: move.w VDP_STATUS, d0
            0x02, 0x40, 0x00, 0xFF,             //       andi.w #LINE_MASK, d0
            0x33, 0xC0, 0xFF, 0x00, 0x00, 0x10, //       move.w d0, BRIGHTNESS
            0x60, 0xEE,                         //       bra.s  loop
        ];
        let mut m = System::new(&test_bios(), &test_cart(&code)).unwrap();
        let pal = crate::bus::PALETTE_BASE as usize;

        m.bus.mem[pal..pal + 4].copy_from_slice(&0x00FF_FFFFu32.to_be_bytes());

        m.run_frame();
        m.run_frame();

        let mut out = vec![0u32; vdp::WIDTH * vdp::HEIGHT];
        m.render(&mut out);

        for (y, want) in [(0usize, 0u32), (100, 0x0064_6464), (179, 0x00B3_B3B3)] {
            assert_eq!(
                out[y * vdp::WIDTH],
                want,
                "line {y} was not composed with that line's brightness"
            );
        }
    }

    #[test]
    fn reload_rejects_a_malformed_image_and_leaves_the_machine_untouched() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();
        let noinit_addr = crate::bus::RAM_BASE as usize + crate::bus::BIOS_PARTITION as usize;

        m.bus.reset_reason = 1;
        m.bus.mem[noinit_addr] = 0xCD;
        let cart_before =
            m.bus.mem[CART_BASE as usize..CART_BASE as usize + HEADER_LEN + 2].to_vec();

        let mut bad = test_cart(&[0x60, 0xfe]);
        bad[0] = b'X';

        let err = m.reload(&bad).unwrap_err();

        assert_eq!(err, CartError::BadMagic);
        assert_eq!(
            m.bus.reset_reason, 1,
            "reset_reason changed on a rejected image"
        );
        assert_eq!(
            m.bus.mem[noinit_addr], 0xCD,
            "RAM changed on a rejected image"
        );
        assert_eq!(
            m.bus.mem[CART_BASE as usize..CART_BASE as usize + HEADER_LEN + 2],
            cart_before[..],
            "cart window changed on a rejected image"
        );
    }
}

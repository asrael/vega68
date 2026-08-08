use m68k::{BatchExit, CpuCore, CpuType};

use crate::apu;
use crate::bus::{Bus, CART_BASE, FB_BASE, LINES_PER_FRAME, VDP_MODE, VISIBLE_LINES};
use crate::cart::{self, CartError};
use crate::vdp;

/// 100 MHz @ ~5 cycles/instruction, 60 fps, 200 lines/frame.
pub const INSTRUCTIONS_PER_LINE: u32 = 1_667;

const FRAME_SAMPLES: usize = LINES_PER_FRAME as usize * apu::SAMPLES_PER_LINE * 2;
const RESET_RELOAD: u16 = 2;

pub struct System {
    pub bus: Bus,
    pub cpu: CpuCore,
}

fn line_irq_fires(compare: u16, interval: u16, line: u32) -> bool {
    let compare = compare as u32;

    match interval {
        0 => line == compare,
        n => {
            let n = n as u32;
            line >= compare && line < VISIBLE_LINES && (line - compare) % n == 0
        }
    }
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

    pub fn reload(&mut self, cart: &[u8]) -> Result<(), CartError> {
        cart::parse(cart)?;

        self.bus.mem[CART_BASE as usize..CART_BASE as usize + cart.len()].copy_from_slice(cart);

        self.bus.apu.reset();
        self.bus.tpu.reset();
        self.bus.irq_enable = 0;
        self.bus.irq_pending = 0;
        self.bus.line_compare = 0;
        self.bus.line_interval = 0;
        self.bus.brightness = 255;
        self.bus.reset_reason = RESET_RELOAD;
        self.bus.mem[VDP_MODE as usize..VDP_MODE as usize + 2].fill(0);
        self.bus.mem[FB_BASE as usize..FB_BASE as usize + 4].fill(0);

        self.cpu.reset(&mut self.bus);

        Ok(())
    }

    pub fn render(&self, out: &mut [u32]) {
        vdp::render(&self.bus.mem, self.bus.brightness, out);
    }

    pub fn run_frame(&mut self) {
        let Bus { tpu, mem, .. } = &mut self.bus;
        tpu.frame_start(mem);

        let lines_run = self.run_cpu_lines();
        self.top_up_frame(lines_run);
    }

    // Runs the CPU line by line, rendering that line's audio as it completes.
    // Returns how many lines actually got their audio rendered this call --
    // fewer than LINES_PER_FRAME if the CPU stuck partway through.
    fn run_cpu_lines(&mut self) -> u32 {
        let mut lines_run = 0;

        'frame: for line in 0..LINES_PER_FRAME {
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

            let mut instructions = 0u32;

            while instructions < INSTRUCTIONS_PER_LINE {
                let budget = INSTRUCTIONS_PER_LINE - instructions;
                let r = self.cpu.run_batch(&mut self.bus, budget, &[]);

                let taken = match r.exit {
                    BatchExit::IllegalInstruction { .. } => {
                        self.cpu.take_illegal_exception(&mut self.bus);
                        1
                    }

                    BatchExit::AlineTrap { .. } => {
                        self.cpu.take_aline_exception(&mut self.bus);
                        1
                    }

                    BatchExit::FlineTrap { .. } => {
                        self.cpu.take_fline_exception(&mut self.bus);
                        1
                    }

                    BatchExit::Breakpoint { .. } => {
                        self.cpu.take_bkpt_exception(&mut self.bus);
                        1
                    }

                    BatchExit::TrapInstruction { trap_num } => {
                        self.cpu.take_trap_exception(&mut self.bus, trap_num);
                        1
                    }

                    BatchExit::BudgetExhausted => 0,
                    BatchExit::Stopped => break,
                    BatchExit::WatchedPc { .. } => unreachable!("watch list is empty"),
                };

                if r.instructions + taken == 0 {
                    break 'frame; // stuck: don't spin forever
                }

                instructions += r.instructions + taken;
            }

            let Bus { apu, mem, .. } = &mut self.bus;
            apu.run_line(mem, line);
            lines_run += 1;
        }

        lines_run
    }

    // A stuck CPU (`run_cpu_lines` returning short) leaves the audio frame
    // partial; top up the remaining lines so it is always full. Tracking
    // `lines_run` explicitly (not the frame's own length) matters when the
    // CPU sticks on line 0 before that line's `apu.run_line` ever runs: the
    // frame buffer is still whatever the *previous* call to `run_frame` left
    // it at (a full 1600-sample frame), so a length-based check would no-op
    // and silently replay stale audio instead of rebuilding this frame.
    fn top_up_frame(&mut self, mut lines_run: u32) {
        while lines_run < LINES_PER_FRAME {
            let Bus { apu, mem, .. } = &mut self.bus;
            apu.run_line(mem, lines_run);
            lines_run += 1;
        }

        debug_assert_eq!(self.bus.apu.frame.len(), FRAME_SAMPLES);
    }
}

#[cfg(test)]
mod tests {
    use m68k::AddressBus;
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
    fn line_irq_fires_matches_the_worked_table() {
        let cases: [(u16, u16, u32); 7] = [
            (185, 0, 1), // decision 1: interval 0 still fires in vblank
            (40, 0, 1),
            (40, 2, 70),
            (41, 2, 70), // odd compare, even interval: phase, not modulus
            (0, 1, 180),
            (170, 5, 2),
            (185, 2, 0), // repeat clamp: no fire in vblank
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
        let new_cart = test_cart(&[0x4e, 0x71, 0x60, 0xfe]); // nop; bra.s *

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

        m.bus.mem[noinit_addr] = 0xAB; // stand-in for a cart .noinit byte

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
        m.bus.write_byte(crate::bus::AUDIO_BASE, 0xFF);
        m.bus.tpu.head = 40;
        m.bus.tpu.tail = 80;
        m.bus.tpu.pixels = 12345;
        m.bus.tpu.deficit = 999;
        m.bus.write_word(crate::bus::VDP_MODE, 0x0003);
        m.bus.write_long(crate::bus::FB_BASE, 0x1234_5678);

        m.reload(&test_cart(&[0x60, 0xfe])).unwrap();

        assert_eq!(m.bus.irq_enable, 0, "irq_enable not reset");
        assert_eq!(m.bus.irq_pending, 0, "irq_pending not reset");
        assert_eq!(m.bus.line_compare, 0, "line_compare not reset");
        assert_eq!(m.bus.line_interval, 0, "line_interval not reset");
        assert_eq!(m.bus.brightness, 255, "brightness not reset");
        assert_eq!(m.bus.read_byte(crate::bus::AUDIO_BASE), 0, "apu register not reset");
        assert_eq!(m.bus.tpu.head, 0, "tpu head not reset");
        assert_eq!(m.bus.tpu.tail, 0, "tpu tail not reset");
        assert_eq!(m.bus.tpu.pixels, 0, "tpu pixels not reset");
        assert_eq!(m.bus.tpu.deficit, 0, "tpu deficit not reset");
        assert_eq!(m.bus.read_word(crate::bus::VDP_MODE), 0, "VDP_MODE not reset");
        assert_eq!(m.bus.read_long(crate::bus::FB_BASE), 0, "FB_BASE not reset");
    }

    #[test]
    fn run_frame_redrains_a_command_the_budget_deferred_to_the_next_frame() {
        use crate::bus::{TPU_TAIL, TPU_RAM_BASE};

        const RING: u32 = 0x40;
        const COLOR: u32 = 0x1000;
        const WIDTH: u16 = 1900;
        const HEIGHT: u16 = 1900;

        let fill = |flags: u32, x0: u32, y0: u32, x1: u32, y1: u32, color: u32| -> [u32; 5] {
            [0x0200_0000 | flags, x0 << 16 | y0, x1 << 16 | y1, color, 0]
        };

        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();

        m.bus.write_long(TPU_RAM_BASE, RING);
        m.bus.write_long(TPU_RAM_BASE + 4, 16); // ring_words: power of two, room for 2 FILL commands
        m.bus.write_long(TPU_RAM_BASE + 8, COLOR);
        m.bus.write_long(TPU_RAM_BASE + 12, 0); // z_base unused
        m.bus.write_word(TPU_RAM_BASE + 16, WIDTH);
        m.bus.write_word(TPU_RAM_BASE + 18, HEIGHT);

        // cmd1 covers the whole target (3,610,000 px, cost 902,500): alone it
        // overshoots PIXEL_BUDGET (833,333) by 69,167 and must defer cmd2.
        let cmd1 = fill(0x01, 0, 0, WIDTH as u32, HEIGHT as u32, 5);
        let cmd2 = fill(0x01, 0, 0, 2, 2, 9); // 4 px, cost 1

        for (i, w) in cmd1.into_iter().chain(cmd2).enumerate() {
            m.bus.write_long(TPU_RAM_BASE + RING + i as u32 * 4, w);
        }

        m.bus.write_word(TPU_TAIL, 10); // tail: both commands queued

        assert_ne!(m.bus.tpu.head, m.bus.tpu.tail, "command 2 must not run before the budget resets");
        assert_eq!(m.bus.read_byte(TPU_RAM_BASE + COLOR), 5, "command 2 ran before its budget was granted");

        m.run_frame();

        assert_eq!(m.bus.tpu.head, m.bus.tpu.tail, "ring not fully drained after frame_start");
        assert_eq!(m.bus.read_byte(TPU_RAM_BASE + COLOR), 9, "command 2 never ran after the budget reset");
        assert_eq!(m.bus.tpu.pixels, 1, "frame_start must zero pixels before re-draining");
    }

    #[test]
    fn run_frame_produces_a_full_audio_frame() {
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();

        m.run_frame();
        assert_eq!(m.bus.apu.frame.len(), 1600);
    }

    #[test]
    fn top_up_rebuilds_a_stale_frame_when_the_cpu_sticks_before_line_zero() {
        // Regression seam for a bug where the stuck-CPU top-up gated on
        // `apu.frame.len() < 1600`: if the CPU stuck on line 0 before that
        // line's `apu.run_line` ran, the frame buffer was still the
        // *previous* frame's full 1600 samples, so the length check no-oped
        // and stale audio got replayed. A real zero-instruction CPU stall is
        // impractical to construct from a test cart (STOP and every
        // exception path always retire >= 1 instruction in this emulator),
        // so this exercises the fixed seam (`top_up_frame`, driven by an
        // explicit `lines_run` count rather than the frame's own length)
        // directly with `lines_run == 0`, the exact precondition a
        // stuck-at-line-0 CPU leaves behind.
        let mut m = System::new(&test_bios(), &test_cart(&[0x60, 0xfe])).unwrap();
        m.run_frame();
        let real = m.bus.apu.frame.clone();

        m.bus.apu.frame = vec![i16::MAX; 1600]; // stale sentinel frame, as if never cleared
        m.top_up_frame(0);

        assert_ne!(
            m.bus.apu.frame,
            vec![i16::MAX; 1600],
            "top-up must not no-op just because the stale frame was already full length"
        );
        assert_eq!(
            m.bus.apu.frame, real,
            "top-up from line 0 must rebuild the same frame a normal run produces"
        );
    }

    #[test]
    fn a_cart_write_to_the_apu_is_heard_the_same_frame() {
        // move.w #0x00FE, PERIOD(ch 8); move.b #0, ATTEN — then loop
        let base = 0xFF00_0400u32 + 8 * 0x40;
        let mut code = vec![];
        code.extend([0x33, 0xFC, 0x00, 0xFE]); // move.w #0x00FE, (xxx).l
        code.extend(base.to_be_bytes());
        code.extend([0x42, 0x39]); // clr.b (xxx).l
        code.extend((base + 2).to_be_bytes());
        code.extend([0x60, 0xFE]); // bra.s *

        let mut m = System::new(&test_bios(), &test_cart(&code)).unwrap();
        m.run_frame();

        assert!(m.bus.apu.frame.iter().any(|&s| s != 0), "square never reached the mix");
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
        bad[0] = b'X'; // bad magic

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

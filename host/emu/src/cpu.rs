//! Musashi binding. The C core is one global; every `Cpu` owns a context
//! copy and swaps it in under `LOCK` for the duration of each call, with
//! `BUS` pointing at the caller's bus so the memory callbacks can reach it.

use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::bus::Bus;

const CPU_TYPE_68LC040: u32 = 8;

static LOCK: Mutex<()> = Mutex::new(());
static mut BUS: *mut Bus = ptr::null_mut();
static mut RETIRED: u64 = 0;

pub struct Cpu {
    ctx: Box<[u64]>,
    irq: u8,
    pub retired: u64,
}

unsafe extern "C" {
    fn m68k_context_size() -> u32;
    fn m68k_execute(cycles: i32) -> i32;
    fn m68k_get_context(dst: *mut c_void) -> u32;
    fn m68k_init();
    fn m68k_pulse_reset();
    fn m68k_set_context(src: *mut c_void);
    fn m68k_set_cpu_type(cpu_type: u32);
    fn m68k_set_instr_hook_callback(hook: Option<unsafe extern "C" fn(u32)>);
    fn m68k_set_irq(level: u32);
}

impl Default for Cpu {
    fn default() -> Cpu {
        Cpu::new()
    }
}

impl Cpu {
    pub fn new() -> Cpu {
        let _lock = lock();

        unsafe {
            let words = (m68k_context_size() as usize).div_ceil(8);
            let mut ctx = vec![0u64; words].into_boxed_slice();

            m68k_set_context(ctx.as_mut_ptr().cast());
            m68k_init();
            m68k_set_cpu_type(CPU_TYPE_68LC040);
            m68k_set_instr_hook_callback(Some(count));
            m68k_get_context(ctx.as_mut_ptr().cast());

            Cpu {
                ctx,
                irq: 0,
                retired: 0,
            }
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.with(bus, || unsafe { m68k_pulse_reset() });
    }

    pub fn run(&mut self, bus: &mut Bus, cycles: u32) -> u32 {
        let irq = self.irq;

        self.with(bus, || unsafe {
            m68k_set_irq(irq as u32);
            m68k_execute(cycles as i32) as u32
        })
    }

    pub fn set_irq(&mut self, level: u8) {
        self.irq = level;
    }

    fn with<T>(&mut self, bus: &mut Bus, f: impl FnOnce() -> T) -> T {
        let _lock = lock();

        unsafe {
            m68k_set_context(self.ctx.as_mut_ptr().cast());
            BUS = bus;
            let before = RETIRED;

            let r = f();

            self.retired += RETIRED - before;
            BUS = ptr::null_mut();
            m68k_get_context(self.ctx.as_mut_ptr().cast());

            r
        }
    }
}

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

unsafe fn bus<'a>() -> &'a mut Bus {
    unsafe { &mut *BUS }
}

unsafe extern "C" fn count(_pc: u32) {
    unsafe { RETIRED += 1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_memory_8(a: u32) -> u32 {
    unsafe { bus() }.read_u8(a) as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_memory_16(a: u32) -> u32 {
    unsafe { bus() }.read_u16(a) as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_memory_32(a: u32) -> u32 {
    unsafe { bus() }.read_u32(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_write_memory_8(a: u32, v: u32) {
    unsafe { bus() }.write_u8(a, v as u8)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_write_memory_16(a: u32, v: u32) {
    unsafe { bus() }.write_u16(a, v as u16)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_write_memory_32(a: u32, v: u32) {
    unsafe { bus() }.write_u32(a, v)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_disassembler_8(a: u32) -> u32 {
    m68k_read_memory_8(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_disassembler_16(a: u32) -> u32 {
    m68k_read_memory_16(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn m68k_read_disassembler_32(a: u32) -> u32 {
    m68k_read_memory_32(a)
}

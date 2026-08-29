pub mod apu;
mod bios;
pub mod bus;
pub mod cart;
pub mod system;
pub mod vdp;

pub use bios::BIOS;
pub use system::System;

pub fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;

    for b in bytes {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }

    h
}

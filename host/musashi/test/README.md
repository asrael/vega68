Images are checked in. Rebuild after editing a `.s`:
`make -C host/musashi/test` (m68k-elf-as + m68k-elf-ld, same toolchain
as the guest build). Run them: `cargo test -p vega68 --test musashi` —
`host/emu/tests/musashi.rs` is the driver (protocol in `entry.s`:
writes to 0x100004 pass, 0x100000 fail, then `stop`).

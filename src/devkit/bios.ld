/* bios.ld: bios and kernel code at $0 */

ENTRY(_start)

MEMORY
{
    rom (rx)  : ORIGIN = 0x00000000, LENGTH = 4M
    ram (rwx) : ORIGIN = 0x80000000, LENGTH = 1M
}

SECTIONS
{
    .vectors 0x00000000 : { KEEP(*(.vectors)) } > rom
    .text : { *(.text*) *(.rodata*) } > rom
    .data : { _sdata = .; *(.data*) _edata = .; } > ram AT> rom
    .bss (NOLOAD) : { _sbss = .; *(.bss*) *(COMMON) _ebss = .; } > ram
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame) *(.eh_frame_hdr) *(.debug*) }
}

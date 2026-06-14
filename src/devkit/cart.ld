/* cart.ld: 64-byte header and user code at $00400000 */

ENTRY(_start)

MEMORY
{
    rom (rx)  : ORIGIN = 0x00400000, LENGTH = 28M
    ram (rwx) : ORIGIN = 0x80100000, LENGTH = 1M   /* user half ($80100000..$801FFFFF) */
}

SECTIONS
{
    .cartheader 0x00400000 : { KEEP(*(.cartheader)) } > rom
    .text : { *(.text*) *(.rodata*) } > rom
    .data : { _sdata = .; *(.data*) _edata = .; } > ram AT> rom
    .bss (NOLOAD) : { _sbss = .; *(.bss*) *(COMMON) _ebss = .; } > ram
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame) *(.eh_frame_hdr) *(.debug*) }
}

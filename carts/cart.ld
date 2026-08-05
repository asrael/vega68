OUTPUT_ARCH(m68k)
ENTRY(_start)

MEMORY {
  ROM (rx)  : ORIGIN = 0x01000000, LENGTH = 16M
  RAM (rwx) : ORIGIN = 0x02020000, LENGTH = 0x3E0000
}

SECTIONS {
  .v68hdr : { KEEP(*(.v68hdr)) } > ROM

  INCLUDE vega68_sections.ld

  .noinit (NOLOAD) : { *(.noinit*) } > RAM
}

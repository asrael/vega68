OUTPUT_ARCH(m68k)
ENTRY(_start)

MEMORY {
  ROM (rx)  : ORIGIN = 0x00000000, LENGTH = 1M
  RAM (rwx) : ORIGIN = 0x02000000, LENGTH = 128K
}

__stack_top = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS {
  .vectors : { KEEP(*(.vectors)) . = 0x400; } > ROM

  INCLUDE sections.ld

  .noinit (NOLOAD) : {
    . = ALIGN(4);
    *(.noinit*)
  } > RAM
}

.text : {
  *(.text*)
  . = ALIGN(4);
  *(.rodata*)
  . = ALIGN(4);
} > ROM

.data : {
  . = ALIGN(4);
  __data_start = .;
  *(.data*)
  . = ALIGN(4);
  __data_end = .;
} > RAM AT> ROM
__data_load = LOADADDR(.data);

.bss (NOLOAD) : {
  . = ALIGN(4);
  __bss_start = .;
  *(.bss*)
  *(COMMON)
  . = ALIGN(4);
  __bss_end = .;
} > RAM

/DISCARD/ : { *(.comment) *(.note*) }

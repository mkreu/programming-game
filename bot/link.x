SECTIONS
{
  . = 0x10000;
  .text : { *(.text) }
  . = 0x800000;
  .data : { *(.data) }
  .bss : { *(.bss) }
}
ENTRY(main)

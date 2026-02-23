SECTIONS
{
  . = 0x1000;
  .text : { *(.text .text.*) }
  .data : { *(.data) }
  .bss : { *(.bss) }
}
ENTRY(main)
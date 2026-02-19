MEMORY
{
  /* SRAM peripheral memory starts at 0x52000000 */
  RAM : ORIGIN = 0x52000000, LENGTH = 8K
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", RAM);

_hart_stack_size = 1K;

/* Reserve a 1K heap for global allocator initialization via riscv-rt symbols */
_heap_size = 1K;

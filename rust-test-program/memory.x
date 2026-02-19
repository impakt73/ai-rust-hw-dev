MEMORY
{
  /* Main memory starts at 0x80000000 */
  RAM : ORIGIN = 0x80000000, LENGTH = 256M
  /* SRAM peripheral memory starts at 0x52000000 */
  SRAM : ORIGIN = 0x52000000, LENGTH = 8K
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", SRAM);
REGION_ALIAS("REGION_STACK", SRAM);

_hart_stack_size = 2K;

/* Reserve a 6K heap for global allocator initialization via riscv-rt symbols */
_heap_size = 6K;

MEMORY
{
  /* Main memory starts at 0x80000000 */
  RAM : ORIGIN = 0x80000000, LENGTH = 256M
  /* SRAM peripheral memory starts at 0x70000000 */
  SRAM : ORIGIN = 0x70000000, LENGTH = 12K
}

REGION_ALIAS("REGION_TEXT", SRAM);
REGION_ALIAS("REGION_RODATA", SRAM);
REGION_ALIAS("REGION_DATA", SRAM);
REGION_ALIAS("REGION_BSS", SRAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", SRAM);

_hart_stack_size = 1K;

/* Reserve a 4M heap for global allocator initialization via riscv-rt symbols */
_heap_size = 4M;

MEMORY
{
  /* Pocket RTL SDRAM starts at 0x10000000 */
  SDRAM : ORIGIN = 0x10000000, LENGTH = 64M
}

REGION_ALIAS("REGION_TEXT", SDRAM);
REGION_ALIAS("REGION_RODATA", SDRAM);
REGION_ALIAS("REGION_DATA", SDRAM);
REGION_ALIAS("REGION_BSS", SDRAM);
REGION_ALIAS("REGION_HEAP", SDRAM);
REGION_ALIAS("REGION_STACK", SDRAM);

_hart_stack_size = 1K;

/* Reserve a 4M heap for global allocator initialization via riscv-rt symbols */
_heap_size = 4M;

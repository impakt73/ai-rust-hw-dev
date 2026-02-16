MEMORY
{
  /* Main memory starts at 0x80000000 */
  RAM : ORIGIN = 0x80000000, LENGTH = 256M
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", RAM);

/* Stack grows downward from the end of RAM */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);
_hart_stack_size = 1K;

/* Reserve a 1K heap for global allocator initialization via riscv-rt symbols */
_heap_size = 1K;

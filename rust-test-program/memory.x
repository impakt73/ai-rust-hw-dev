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

_hart_stack_size = 256;

/* Heap is intentionally small since on-device SRAM is only 8K.
   Programs that need more heap should use DRAM addresses (0x80000000+). */
_heap_size = 0;


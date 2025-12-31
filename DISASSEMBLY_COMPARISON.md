# Disassembly Comparison: Stack vs Heap Memory Tests

## Stack Memory Test (test_stack_memory.elf) - PASSES
```assembly
80000208 <main>:
  # Writing bytes to FIFO using immediate values
  80000218:  01200513   li   a0,18         # Load 0x12
  8000021c:  00a5a023   sw   a0,0(a1)      # Store WORD to FIFO
  80000220:  03400513   li   a0,52         # Load 0x34
  80000224:  00a5a023   sw   a0,0(a1)      # Store WORD to FIFO
  80000228:  05600513   li   a0,86         # Load 0x56
  8000022c:  00a5a023   sw   a0,0(a1)      # Store WORD to FIFO
  # ... continues with SW (store word) instructions only
```

**Key observation:** Stack test uses ONLY word-level operations (li + sw).
No byte-level stores (sb) or loads (lbu). This works perfectly.

## Heap Memory Test (test_heap_directly.elf) - FAILS
```assembly
80000228 <main>:
  # Allocate heap memory
  80000240:  00800513   li   a0,8
  80000244:  00100593   li   a1,1
  80000248:  ...        (allocator call)
  
  # Load byte values into registers
  80000284:  01200593   li   a1,18         # 0x12
  80000288:  03400613   li   a2,52         # 0x34
  8000028c:  05600693   li   a3,86         # 0x56
  80000290:  07800713   li   a4,120        # 0x78
  80000294:  09a00793   li   a5,154        # 0x9a
  80000298:  0bc00813   li   a6,188        # 0xbc
  8000029c:  0de00893   li   a7,222        # 0xde
  800002a0:  ff000293   li   t0,-16        # 0xf0
  
  # Write bytes to heap using SB (store byte)
  800002a4:  00b50023   sb   a1,0(a0)      # Store BYTE 0x12 at offset 0
  800002a8:  00c500a3   sb   a2,1(a0)      # Store BYTE 0x34 at offset 1
  800002ac:  00d50123   sb   a3,2(a0)      # Store BYTE 0x56 at offset 2
  800002b0:  00e501a3   sb   a4,3(a0)      # Store BYTE 0x78 at offset 3
  800002b8:  00f50223   sb   a5,4(a0)      # Store BYTE 0x9a at offset 4
  800002bc:  010502a3   sb   a6,5(a0)      # Store BYTE 0xbc at offset 5
  800002c0:  01150323   sb   a7,6(a0)      # Store BYTE 0xde at offset 6
  800002c4:  005503a3   sb   t0,7(a0)      # Store BYTE 0xf0 at offset 7
  
  # Read bytes back using LBU (load byte unsigned)
  800002d4:  00054583   lbu  a1,0(a0)      # Load BYTE from offset 0
  800002d8:  00b9a023   sw   a1,0(s3)      # Store to FIFO
  800002dc:  00154583   lbu  a1,1(a0)      # Load BYTE from offset 1
  800002e0:  00b9a023   sw   a1,0(s3)      # Store to FIFO
  # ... continues with LBU + SW pattern
```

**Key observation:** Heap test uses SB (store byte) and LBU (load byte unsigned).
These byte-level operations trigger the RTL bug where adjacent bytes are overwritten.

## The Critical Difference

| Aspect | Stack Test | Heap Test |
|--------|-----------|-----------|
| Write instruction | `sw` (store word) | `sb` (store byte) |
| Read instruction | N/A (direct values) | `lbu` (load byte unsigned) |
| Result | ✓ All bytes correct | ✗ Adjacent bytes zeroed |

The compiler optimizes stack operations to use word-level stores, but uses byte-level
stores for heap because it doesn't know the alignment or can't prove contiguous access.


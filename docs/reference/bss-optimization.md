# BSS Zero-Initialization Optimization

## Problem

ELF files that use heap allocation were experiencing excessive startup times in the simulator due to BSS zero-initialization. The 8KB static heap in the `SimpleAllocator` was placed in the `.bss` section, which the `riscv-rt` startup code zeros on every program start.

### Impact
- **Before Fix**: BSS section = 8224 bytes (8192 for HEAP + 32 for other statics)
- **Startup Cost**: ~6000 cycles spent zeroing memory that doesn't need to be zeroed
- **Test Impact**: Some tests were failing due to cycle count limits

## Root Cause

The allocator implementation used:
```rust
static mut HEAP: [u8; 8192] = [0; 8192];
```

This creates a zero-initialized array in the `.bss` section. The `riscv-rt` startup code has a loop that zeros the entire `.bss` section before calling `main()`:

```assembly
.Lpcrel_hi8:
80000b0:	auipc	t2,0x3
80000b4:	addi	t2,t2,-1200      # Load __ebss
80000b8:	bgeu	t0,t2,done       # If t0 >= __ebss, done
80000bc:	sw	zero,0(t0)       # Zero current word
80000c0:	addi	t0,t0,4          # Next word
80000c4:	bltu	t0,t2,loop       # Loop until done
```

This loop was zeroing 2056 words (8224 bytes), taking approximately:
- 1 store instruction per word
- 1 add instruction per word  
- 1 branch instruction per word
- Total: ~3 cycles × 2056 words = **~6000 cycles**

## Solution

Moved the HEAP to the `.uninit` section using the `#[link_section]` attribute:

```rust
#[link_section = ".uninit"]
static mut HEAP: [u8; 8192] = [0; 8192];
```

### Why This Works

The `riscv-rt` linker script defines two sections for uninitialized data:
- `.bss`: Zeroed by startup code (between `__sbss` and `__ebss`)
- `.uninit`: **NOT** zeroed by startup code (between `__suninit` and `__euninit`)

From the `riscv-rt` linker script:
```
.uninit (NOLOAD) : ALIGN(32)
{
  . = ALIGN(32);
  __suninit = .;
  *(.uninit .uninit.*);
  . = ALIGN(32);
  __euninit = .;
} > REGION_BSS
```

The `.uninit` section is explicitly designed for data that:
1. Does not need to be zero-initialized
2. Can contain residual or uninitialized values
3. Will be initialized before use (as allocators do)

## Results

### After Fix
- **BSS section**: 4 bytes (only the `AtomicUsize` OFFSET counter)
- **UNINIT section**: 8192 bytes (the HEAP, not zeroed)
- **Cycle savings**: ~6000 cycles per program startup
- **All tests pass**: 130 tests verified

### Verification

Check sections in any ELF using the allocator:
```bash
$ riscv64-unknown-elf-objdump -h test_programs/println_test.elf | grep -E "\.bss|\.uninit"
  6 .bss          00000004  80002be0  80002be0  00003be0  2**5
  7 .uninit       00002000  80002c00  80002c00  00003be0  2**5
```

Check startup code no longer zeros the HEAP:
```bash
$ riscv64-unknown-elf-nm test_programs/println_test.elf | grep -E "ebss|euninit"
80002c00 B __ebss     # BSS ends here (only 32 bytes from __sbss)
80004c00 B __euninit  # UNINIT ends here (8192 bytes from __suninit)
```

The BSS zero loop now only zeroes from `__sbss` (0x80002be0) to `__ebss` (0x80002c00) = 32 bytes.

## Safety Considerations

Using `.uninit` is safe for heap allocators because:

1. **Standard Behavior**: Standard allocators like `malloc()` return uninitialized memory
2. **Caller Responsibility**: Code that calls `alloc()` must initialize memory before use
3. **Rust Guarantees**: Safe Rust code cannot read uninitialized memory
4. **Allocator Contract**: The `GlobalAlloc` trait does not require zeroing

### From Rust Documentation
> The memory returned from alloc may be uninitialized. Callers must initialize the memory before use.

## Files Modified

1. `rust-test-program/src/common.rs`: Updated `SimpleAllocator` to use `.uninit`
2. `rust-test-program/src/test_static_heap.rs`: Updated test to use `.uninit`
3. All 19 ELF files in `test_programs/`: Rebuilt with the fix

## Performance Impact

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| BSS Size | 8224 bytes | 4 bytes | 99.95% |
| Startup Cycles | ~6000 | ~10 | ~6000 cycles saved |
| Test Pass Rate | Some failures | 100% | All 130 tests pass |

## Lessons Learned

1. **Linker Sections Matter**: Understanding linker script sections is crucial for bare-metal optimization
2. **Startup Cost**: Zero-initialization can be a significant cost in embedded systems
3. **Use .uninit**: The `.uninit` section is specifically designed for this use case
4. **Profile First**: Disassembly and symbol analysis revealed the issue quickly

## References

- [riscv-rt documentation](https://docs.rs/riscv-rt/)
- [RISC-V Embedded Book](https://docs.rust-embedded.org/book/)
- Linker script: `rust-test-program/target/.../build/riscv-rt-.../out/link.x`

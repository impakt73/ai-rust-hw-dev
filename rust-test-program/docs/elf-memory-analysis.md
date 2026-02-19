# ELF Memory Analysis: Rust Test Programs

**Target:** `riscv32imafc-unknown-none-elf` (release build)  
**Build Profile:** `cargo build --release` (optimized)  
**Toolchain:** riscv64-unknown-elf-readelf, riscv64-unknown-elf-objdump, riscv64-unknown-elf-nm

---

## 1. Memory Architecture

All test programs share the same linker configuration defined in `memory.x` and `linker.ld`.

### Address Map

| Region | Base Address | Size | Purpose |
|--------|-------------|------|---------|
| DRAM (RAM) | `0x80000000` | 256 MiB | Code, read-only data, BSS |
| SRAM | `0x52000000` | 8 KiB | Heap + Stack |

### SRAM Layout (runtime)

```
0x52000000  ┌──────────────────────────┐
            │  .heap  (1 KiB = 1024 B) │  _heap_size = 1K
0x52000400  ├──────────────────────────┤
            │                          │
            │  .stack (up to 7 KiB)    │  SP starts at 0x52002000,
            │  (grows downward)        │  grows down to 0x52000400
            │                          │
0x52002000  └──────────────────────────┘
```

`memory.x` declares `_hart_stack_size = 1K` (minimum guaranteed) and `_heap_size = 1K`.
The actual maximum available stack is 7 KiB (SRAM total − heap).

### ELF PT_LOAD Segment Structure

Each binary contains either 3 or 4 `PT_LOAD` segments:

| Segment | vaddr | Flags | Contents |
|---------|-------|-------|---------|
| LOAD1 (code) | `0x80000000` | R+E | `.text` + `.init.rust` |
| LOAD2 (rodata) | follows LOAD1 | R | `.rodata` + `.eh_frame` |
| LOAD3 (data/bss) | follows LOAD2 | RW | `.data` + `.bss` (only present if BSS ≠ 0) |
| LOAD4 (SRAM) | `0x52000000` | RW | `.heap` + `.stack` (filesz=0, memsz=8192) |

---

## 2. Memory Definitions

| Term | Meaning |
|------|---------|
| **DRAM Load Size** | Sum of all `PT_LOAD` filesizes for DRAM segments — bytes read from the ELF and written into RAM |
| **DRAM BSS** | Zero-initialized region in DRAM (memsz − filesz of the data/bss segment) |
| **DRAM Runtime** | DRAM Load Size + DRAM BSS — total DRAM consumed at startup |
| **SRAM Runtime** | Always 8,192 bytes — full SRAM region reserved for heap + stack |
| **Total Runtime** | DRAM Runtime + SRAM Runtime |

---

## 3. Complete Memory Summary Table

All sizes are in bytes. Columns correspond to individual ELF sections / segments.

| Binary | `.text` | `.init.rust` | `.rodata` | `.eh_frame` | `.data` | `.bss` (DRAM) | DRAM Load | DRAM Runtime | SRAM | **Total Runtime** |
|--------|---------|-------------|---------|------------|---------|--------------|-----------|-------------|------|-----------------|
| `simple_test` | 440 | 38 | 112 | 44 | 0 | 0 | 634 | 634 | 8,192 | **8,826** |
| `test_fp_math` | 440 | 38 | 112 | 44 | 0 | 0 | 634 | 634 | 8,192 | **8,826** |
| `rust_test` | 492 | 38 | 112 | 44 | 0 | 0 | 686 | 686 | 8,192 | **8,878** |
| `test_atomic_simple` | 528 | 38 | 112 | 44 | 0 | 0 | 722 | 722 | 8,192 | **8,914** |
| `test_panic` | 520 | 38 | 156 | 76 | 0 | 0 | 790 | 790 | 8,192 | **8,982** |
| `test_memory_pattern` | 500 | 38 | 112 | 44 | 0 | 0 | 694 | 694 | 8,192 | **8,886** |
| `test_atomic` | 1,000 | 38 | 112 | 44 | 0 | 4 | 1,194 | 1,198 | 8,192 | **9,390** |
| `test_audio_loop` | 648 | 38 | 176 | 44 | 0 | 0 | 906 | 906 | 8,192 | **9,098** |
| `test_audio_pattern` | 724 | 38 | 176 | 44 | 0 | 0 | 982 | 982 | 8,192 | **9,174** |
| `test_image_data` | 804 | 38 | 112 | 44 | 0 | 0 | 998 | 998 | 8,192 | **9,190** |
| `test_led_loop` | 796 | 38 | 112 | 44 | 0 | 0 | 990 | 990 | 8,192 | **9,182** |
| `test_dma_copy` | 1,212 | 38 | 112 | 44 | 0 | 0 | 1,406 | 1,406 | 8,192 | **9,598** |
| `test_video_loop` | 1,640 | 38 | 296 | 268 | 0 | 0 | 2,242 | 2,242 | 8,192 | **10,434** |
| `test_video_pattern` | 1,820 | 38 | 296 | 268 | 0 | 0 | 2,422 | 2,422 | 8,192 | **10,614** |
| `test_static_heap` | 1,856 | 38 | 252 | 308 | 0 | 0 | 2,454 | 2,454 | 8,192 | **10,646** |
| `test_stack_memory` | 1,972 | 38 | 268 | 308 | 0 | 0 | 2,586 | 2,586 | 8,192 | **10,778** |
| `test_sim_view` | 1,960 | 38 | 384 | 268 | 0 | 0 | 2,650 | 2,650 | 8,192 | **10,842** |
| `hello_world` | 1,584 | 38 | 192 | 308 | 0 | 0 | 2,122 | 2,122 | 8,192 | **10,314** |
| `test_byte_store_simple` | 1,760 | 38 | 240 | 308 | 0 | 0 | 2,346 | 2,346 | 8,192 | **10,538** |
| `test_alloc_only` | 2,816 | 38 | 948 | 444 | 0 | 28 | 4,246 | 4,274 | 8,192 | **12,466** |
| `test_heap_directly` | 3,256 | 38 | 1,080 | 444 | 0 | 28 | 4,818 | 4,846 | 8,192 | **13,038** |
| `test_allocator` | 4,168 | 38 | 1,240 | 920 | 0 | 28 | 6,366 | 6,394 | 8,192 | **14,586** |
| `minimal_postcard_test` | 5,812 | 38 | 1,628 | 1,092 | 0 | 28 | 8,446 | 8,474 | 8,192 | **16,666** |
| `minimal_postcard_test2` | 5,852 | 38 | 1,504 | 1,092 | 0 | 28 | 8,486 | 8,514 | 8,192 | **16,706** |
| `minimal_debug_test` | 6,008 | 38 | 1,628 | 1,092 | 0 | 28 | 8,766 | 8,794 | 8,192 | **16,986** |
| `println_test` | 8,440 | 38 | 1,636 | 1,564 | 0 | 28 | 11,678 | 11,706 | 8,192 | **19,898** |
| `packet_test` | 9,408 | 38 | 1,656 | 1,188 | 0 | 28 | 12,290 | 12,318 | 8,192 | **20,510** |

> **Note on `.bss` in DRAM:** The 28-byte BSS region present in all heap-allocator programs
> is the `embedded_alloc::LlffHeap` allocator state structure (the `static HEAP` global).
> Programs that never initialize the heap have this symbol eliminated by the optimizer.

> **Note on `.init.rust`:** This 38-byte section is the riscv-rt startup stub that initializes
> BSS, copies `.data`, and calls `main`. It is identical across all 28 binaries.

---

## 4. Sorted by Total Runtime Memory

| Rank | Binary | Total Runtime (bytes) | DRAM Runtime | SRAM |
|------|--------|-----------------------|-------------|------|
| 1 | `packet_test` | **20,510** | 12,318 | 8,192 |
| 2 | `println_test` | **19,898** | 11,706 | 8,192 |
| 3 | `minimal_debug_test` | **16,986** | 8,794 | 8,192 |
| 4 | `minimal_postcard_test2` | **16,706** | 8,514 | 8,192 |
| 5 | `minimal_postcard_test` | **16,666** | 8,474 | 8,192 |
| 6 | `test_allocator` | **14,586** | 6,394 | 8,192 |
| 7 | `test_heap_directly` | **13,038** | 4,846 | 8,192 |
| 8 | `test_alloc_only` | **12,466** | 4,274 | 8,192 |
| 9 | `test_sim_view` | **10,842** | 2,650 | 8,192 |
| 10 | `test_stack_memory` | **10,778** | 2,586 | 8,192 |
| 11 | `test_static_heap` | **10,646** | 2,454 | 8,192 |
| 12 | `test_video_pattern` | **10,614** | 2,422 | 8,192 |
| 13 | `test_byte_store_simple` | **10,538** | 2,346 | 8,192 |
| 14 | `test_video_loop` | **10,434** | 2,242 | 8,192 |
| 15 | `hello_world` | **10,314** | 2,122 | 8,192 |
| 16 | `test_dma_copy` | **9,598** | 1,406 | 8,192 |
| 17 | `test_atomic` | **9,390** | 1,198 | 8,192 |
| 18 | `test_image_data` | **9,190** | 998 | 8,192 |
| 19 | `test_led_loop` | **9,182** | 990 | 8,192 |
| 20 | `test_audio_pattern` | **9,174** | 982 | 8,192 |
| 21 | `test_audio_loop` | **9,098** | 906 | 8,192 |
| 22 | `test_panic` | **8,982** | 790 | 8,192 |
| 23 | `test_memory_pattern` | **8,886** | 694 | 8,192 |
| 24 | `rust_test` | **8,878** | 686 | 8,192 |
| 25 | `test_atomic_simple` | **8,914** | 722 | 8,192 |
| 26 | `simple_test` | **8,826** | 634 | 8,192 |
| 27 | `test_fp_math` | **8,826** | 634 | 8,192 |

**Range:** 8,826 bytes (smallest) to 20,510 bytes (largest)  
**Constant base:** Every binary requires exactly 8,192 bytes of SRAM at runtime.  
**Variable component:** DRAM load ranges from 634 bytes to 12,318 bytes.

---

## 5. Detailed Analysis: Large-Memory Binaries

The five binaries with the highest DRAM load (≥ 8 KiB) are analyzed in depth below.

### 5.1 `packet_test` — 20,510 bytes total (largest)

**Source:** `src/packet_test.rs`  
**Purpose:** Sends and receives structured protocol packets (DebugPacket, EchoPacket, DataU32Packet, AssertPacket) over the FIFO using `serde` + `postcard` serialization.

**Memory breakdown:**

| Section | Size | Contribution |
|---------|------|-------------|
| `.text` + `.init.rust` | 9,446 bytes | 77% of DRAM load |
| `.rodata` + `.eh_frame` | 2,844 bytes | 23% of DRAM load |
| `.bss` (DRAM) | 28 bytes | `LlffHeap` allocator state |
| **DRAM Load** | **12,290 bytes** | |
| SRAM | 8,192 bytes | heap (1 KiB) + stack (up to 7 KiB) |
| **Total Runtime** | **20,510 bytes** | |

**Largest code symbols (from nm --size-sort):**

| Symbol (demangled) | Size | Description |
|--------------------|------|-------------|
| `main` | 1,172 bytes | Main test logic — 4 packet send/receive steps |
| `AssertPacket::serialize` | 1,078 bytes | serde serializer for AssertPacket (has String field) |
| `core::fmt::Formatter::pad_integral` | 606 bytes | Integer formatting |
| `PacketHeader::serialize` | 666 bytes | serde serializer for PacketHeader |
| `DebugPacket::serialize` | 528 bytes | serde serializer for DebugPacket (has String field) |
| `linked_list_allocator::HoleList::allocate_first_fit` | 318 bytes | heap alloc |
| `linked_list_allocator::HoleList::deallocate` | 486 bytes | heap dealloc |
| `core::fmt::Formatter::pad` | 450 bytes | string padding |
| `core::fmt::write` (via fmt infra) | ~470 bytes | core formatting |
| `memcpy` (compiler-builtins) | 418 bytes | byte-copy routine |
| `core::str::count::do_count_chars` | 382 bytes | UTF-8 char counting |
| `RawVecInner::finish_grow` + variants | ~800 bytes total | Vec growth logic |

**Root causes of large size:**

1. **`serde` derive macros on four protocol types** — each `#[derive(Serialize)]` generates a
   full serialization function. The `DebugPacket` and `AssertPacket` structs each contain an
   `alloc::string::String` field, which requires dynamic allocation and triggers the full
   heap/Vec/RawVec machinery.

2. **String handling infrastructure** — importing `alloc::string::String` pulls in
   `String::push_str`, `String::write_char`, `alloc::fmt::format`, and several
   `core::fmt::Formatter` methods (~2,000 bytes combined).

3. **Heap allocator** — the `linked_list_allocator` used by `embedded-alloc` adds ~1,500 bytes of
   allocate/deallocate logic, plus the `RawVec` growth machinery for Vec resizing.

4. **`postcard::to_allocvec`** — this function serializes to a heap-allocated `Vec<u8>`, requiring
   the full Vec lifecycle code.

---

### 5.2 `println_test` — 19,898 bytes total

**Source:** `src/println_test.rs`  
**Purpose:** Tests the `rvprintln!` macro with plain and formatted output.

**Memory breakdown:**

| Section | Size | Contribution |
|---------|------|-------------|
| `.text` + `.init.rust` | 8,478 bytes | 73% of DRAM load |
| `.rodata` + `.eh_frame` | 3,200 bytes | 27% of DRAM load |
| `.bss` (DRAM) | 28 bytes | `LlffHeap` state |
| **DRAM Load** | **11,678 bytes** | |
| SRAM | 8,192 bytes | |
| **Total Runtime** | **19,898 bytes** | |

`println_test` has the **largest `.rodata` + `.eh_frame`** of all binaries (3,200 bytes), even
exceeding `packet_test` (2,844 bytes). This is because `rvprintln!` with format arguments
generates extensive unwind and format-string metadata.

**Largest code symbols:**

| Symbol (demangled) | Size | Description |
|--------------------|------|-------------|
| `riscv_shared::macros::send_debug_message` | 764 bytes | Core rvprintln implementation |
| `core::fmt::write` | 470 bytes | Core formatting dispatch |
| `alloc::fmt::format_inner` | 326 bytes | `format!` internals |
| `alloc::fmt::format` | 130 bytes | `format!` wrapper |
| `core::fmt::Formatter::pad_integral` | 606 bytes | Integer formatting |
| `core::fmt::Formatter::pad` | 450 bytes | String padding |
| `String::write_char` | 242 bytes | Character append |
| `String::write_str` | 96 bytes | String append |
| `linked_list_allocator` functions | ~800 bytes | Heap allocation |
| `memcpy` | 418 bytes | Byte copy |

**Root causes:**

1. **`rvprintln!` macro pulls in `format!`** — even a simple `rvprintln!("Hello")` links in
   the entire `alloc::fmt::format` + `core::fmt::write` path (~1,600 bytes), because the macro
   expands to `format_args!()` processing.

2. **Formatted argument `{}`** — the call `rvprintln!("The answer is {}", 42)` additionally
   brings in `core::fmt::Display for u32` and integer formatting routines.

3. **Large `.eh_frame`** — the DWARF unwind tables for `println_test` are 1,564 bytes — the
   largest of all binaries. The deep call chain (`rvprintln!` → `format` → `write` → allocator)
   creates more unwind frames than any other binary.

---

### 5.3 `minimal_debug_test` — 16,986 bytes total

**Source:** `src/minimal_debug_test.rs`  
**Purpose:** Serializes a 2-field struct (`{ a: u32, b: u32 }`) with `postcard::to_allocvec`, then
sends both individual bytes and 4-byte chunks over the FIFO.

**Memory breakdown:**

| Section | Size | Contribution |
|---------|------|-------------|
| `.text` + `.init.rust` | 6,046 bytes | 69% of DRAM load |
| `.rodata` + `.eh_frame` | 2,720 bytes | 31% of DRAM load |
| `.bss` (DRAM) | 28 bytes | |
| **DRAM Load** | **8,766 bytes** | |
| SRAM | 8,192 bytes | |
| **Total Runtime** | **16,986 bytes** | |

Despite operating on a trivially small struct (8 bytes), this binary is 16,986 bytes because
it imports `postcard::to_allocvec` and `serde::Serialize`. The serde + postcard machinery
is a fixed cost regardless of what is being serialized.

**Key fixed costs from serde + postcard + alloc:**

| Component | Approximate Code Bytes |
|-----------|----------------------|
| `linked_list_allocator` (alloc/dealloc) | ~900 bytes |
| `embedded_alloc::Heap::init` | 96 bytes |
| `RawVec` / Vec growth machinery | ~600 bytes |
| `postcard` encoding infrastructure | ~500 bytes |
| `serde::Serialize` glue code | ~400 bytes |
| `core::fmt` (error paths) | ~1,000 bytes |
| Miscellaneous support routines | ~2,500 bytes |

---

### 5.4 `minimal_postcard_test` and `minimal_postcard_test2` — ~16,700 bytes total

**Sources:** `src/minimal_postcard_test.rs`, `src/minimal_postcard_test2.rs`  
**Purpose:** Serialize the same 2-field struct and transmit bytes over the FIFO. These are
nearly identical to `minimal_debug_test` but omit the extra "individual byte" transmission path.

Both binaries differ only in how they chunk the serialized bytes:
- `minimal_postcard_test` sends each byte individually.
- `minimal_postcard_test2` packs bytes into 4-byte words before sending.

**Memory comparison:**

| Binary | `.text` | DRAM Load | Total Runtime |
|--------|---------|-----------|--------------|
| `minimal_postcard_test` | 5,812 bytes | 8,446 bytes | 16,666 bytes |
| `minimal_postcard_test2` | 5,852 bytes | 8,486 bytes | 16,706 bytes |
| `minimal_debug_test` | 6,008 bytes | 8,766 bytes | 16,986 bytes |

`minimal_debug_test` is 320 bytes larger in `.text` because it includes two independent
transmission paths (individual bytes *and* 4-byte chunks), while the other two only have one.

---

## 6. Medium-Memory Analysis: Heap Allocator Programs

The three binaries in the 12–15 KiB range use the heap allocator (`embedded-alloc`) but not
`serde`/`postcard`. The allocator alone costs approximately **3,640 bytes** of code and data.

### 6.1 `test_allocator` — 14,586 bytes

**Source:** `src/test_allocator.rs`  
Uses `alloc::vec![]` to create a `Vec<u8>` with 8 known values, then writes them to the FIFO.

| Section | Size |
|---------|------|
| `.text` | 4,168 bytes |
| `.rodata` + `.eh_frame` | 2,160 bytes |
| DRAM Load | 6,366 bytes |
| Total Runtime | **14,586 bytes** |

The `Vec` type requires more code than a raw `alloc` call: it brings in `RawVec`,
`Vec::push`, iterator support, and drop glue — approximately 700 bytes more than
`test_alloc_only`.

### 6.2 `test_heap_directly` — 13,038 bytes

**Source:** `src/test_heap_directly.rs`  
Uses `GlobalAlloc::alloc` directly to allocate 8 bytes, writing and reading back individual bytes,
then outputs debug addresses and values over the FIFO.

### 6.3 `test_alloc_only` — 12,466 bytes

**Source:** `src/test_alloc_only.rs`  
The smallest heap-using program. Calls `GlobalAlloc::alloc` once, writes a known pattern,
reads it back, and sends markers to the FIFO.

**Allocator cost breakdown (from `test_alloc_only`):**

| Component | Symbol | Size |
|-----------|--------|------|
| Heap init | `embedded_alloc::Heap::init` | 96 bytes |
| Heap alloc | `GlobalAlloc::alloc` impl | 102 bytes |
| Linked-list allocate | `HoleList::allocate_first_fit` | 318 bytes |
| Linked-list deallocate | `HoleList::deallocate` | 486 bytes |
| Linked-list init | `linked_list_allocator::Heap::init` | 114 bytes |
| alloc error handler | `__rdl_alloc_error_handler` | 124 bytes |
| realloc | `__rust_realloc` | 104 bytes |
| error formatting | `unwrap_failed`, `result::panic` | ~200 bytes |
| **Total allocator overhead** | | **~1,544 bytes** |

---

## 7. Low-Memory Programs

Programs that never initialize the heap or use `extern crate alloc` remain very compact.
The optimizer eliminates the uninitialized `static HEAP` global entirely.

### Common code present in every binary

All 28 binaries share a common runtime baseline from `riscv-rt`:

| Component | Size |
|-----------|------|
| `.init.rust` startup stub | 38 bytes |
| Default trap/exception handlers | ~150 bytes |
| `_start_trap_rust` (trap entry) | ~104 bytes |
| `write_tohost` / `default_panic_handler` | ~30 bytes |
| `fifo_write_word` / `fifo_read_word` | ~50 bytes |
| Minimal `.rodata` (riscv-rt symbols) | 112 bytes |
| `.eh_frame` (minimal) | 44 bytes |

**Baseline code size: ~528 bytes** — present in every binary.

### Smallest binaries

| Binary | Notable Feature | Total Runtime |
|--------|----------------|--------------|
| `simple_test` | Immediately writes SUCCESS; no peripherals | 8,826 bytes |
| `test_fp_math` | RV32F floating-point math; no output routing | 8,826 bytes |
| `rust_test` | ALU, memory, loop, function tests; writes to DRAM | 8,878 bytes |
| `test_atomic_simple` | Inline RISC-V AMO assembly | 8,914 bytes |
| `test_panic` | Triggers a panic (goes through panic handler) | 8,982 bytes |

Both `simple_test` and `test_fp_math` produce identical binary sizes (8,826 bytes) because the
optimizer reduces both to the same minimal code path: the startup stub + a few instructions + the
`write_tohost` call. The floating-point operations in `test_fp_math` are entirely computed at
compile time (constant folding) and generate zero runtime code beyond the success path.

### `test_static_heap` — 10,646 bytes (notable special case)

**Source:** `src/test_static_heap.rs`  
This binary declares `static mut STATIC_HEAP: [u8; 8192] = [0; 8192]` with
`#[link_section = ".uninit"]`, intending to reserve 8 KiB in the `.uninit` ELF section.

**However, the release build optimizer eliminates this symbol entirely.** In the compiled binary:
- `.uninit` section size = 0 bytes
- The symbol `STATIC_HEAP` does not appear in the symbol table
- The raw pointer writes to `STATIC_HEAP` (`core::ptr::write(ptr.add(i), ...)`) are compiled to
  writes targeting a constant stack-derived address, and the values are never read externally —
  so the optimizer is free to remove the symbol

As a result, `test_static_heap` uses no more DRAM than other comparable programs (2,454 bytes
load, 10,646 bytes total) — the intended 8 KiB static buffer is absent in the release binary.

---

## 8. Dynamic Runtime Memory Usage

The ELF segment analysis above covers only **static** memory requirements (code, constant data,
and zero-initialized regions). Several programs additionally write to arbitrary DRAM addresses at
runtime. These accesses succeed because the system has 256 MiB of DRAM, but the addresses do not
appear in the ELF and are not reflected in the segment sizes.

| Binary | Dynamic Memory Usage | Address | Size |
|--------|---------------------|---------|------|
| `rust_test` | Scratch memory (loads/stores) | `0x80001000` | 12 bytes |
| `test_memory_pattern` | 256-byte test pattern | `0x80001000` | 256 bytes |
| `test_dma_copy` | Source array | `0x80001000` | 64 bytes |
| `test_dma_copy` | Destination array | `0x80002000` | 64 bytes |
| `test_image_data` | 4×4 RGBA8 framebuffer | `0x80002000` | 64 bytes |
| `test_video_loop` | 64×64 RGB8 framebuffer | `0x80001000` | 12,288 bytes |
| `test_video_pattern` | 64×64 RGBA8 framebuffer | `0x80001000` | 16,384 bytes |
| `test_audio_pattern` | Audio sample buffer | `0x80002000` | 256 bytes (64 × 4) |
| `test_audio_loop` | Audio sample buffer (precomputed) | `0x80002000` | 4,096 bytes (1024 × 4) |
| `test_sim_view` | 64×64 RGB8 framebuffer | `0x80001000` | 12,288 bytes |
| `test_sim_view` | Audio sample buffer | `0x80004000` | 4,096 bytes |

For programs using the heap allocator, the **heap** (1 KiB at `0x52000000`) is available for
dynamic allocation via `alloc::vec!`, `to_allocvec`, etc. The maximum single allocation supported
by the 1 KiB heap is less than 1,024 bytes (minus allocator overhead of ~32 bytes per allocation).

---

## 9. Section Size Progression: Impact of Features

The table below illustrates how enabling each feature tier increases the DRAM load size, starting
from the smallest possible binary.

| Feature Added | Representative Binary | DRAM Load | Δ from Baseline |
|--------------|-----------------------|-----------|----------------|
| Bare minimum (startup + tohost) | `simple_test` | 634 bytes | — |
| + FIFO I/O | `hello_world` | 2,122 bytes | +1,488 bytes |
| + Heap allocator (LlffHeap) | `test_alloc_only` | 4,246 bytes | +3,612 bytes |
| + `Vec<u8>` usage | `test_allocator` | 6,366 bytes | +5,732 bytes |
| + `serde` + `postcard` serialize | `minimal_postcard_test` | 8,446 bytes | +7,812 bytes |
| + `rvprintln!` formatting | `println_test` | 11,678 bytes | +11,044 bytes |
| + Complex protocol types | `packet_test` | 12,290 bytes | +11,656 bytes |

---

## 10. Key Findings

1. **SRAM dominates small programs.** For the 12 smallest binaries, the 8,192-byte SRAM
   reservation (heap + stack) accounts for 85–93% of total runtime memory. Reducing SRAM would
   have the greatest impact on these programs.

2. **`serde` + `postcard` has a minimum fixed cost of ~7,800 bytes** in code and data.
   This is unavoidable when `to_allocvec` is used, even for trivially small structs.
   The serialization derive macros, postcard encoder, `Vec`, `RawVec`, and heap allocator
   are all mandatory components.

3. **`rvprintln!` is expensive.** Including even one call to `rvprintln!` triggers the full
   `alloc::fmt::format` + `core::fmt::write` pipeline, adding ~6,600 bytes of DRAM overhead
   compared to a program that uses only the raw FIFO interface.

4. **The heap allocator alone costs ~3,600 bytes.** The `embedded-alloc` / `linked_list_allocator`
   pair must be included even when only a single small allocation is performed. The allocator BSS
   state (`LlffHeap`) occupies 28 bytes in DRAM.

5. **`test_static_heap`'s 8 KiB static array is optimized away** in release builds. The LLVM
   optimizer removes the `STATIC_HEAP: [u8; 8192]` symbol because the values written via raw
   pointers are never visible outside the function, and the array is linked into `.uninit`
   (no zero-initialization). The resulting binary is smaller than expected (10,646 bytes vs.
   a hypothetical 18,838 bytes if the array were present).

6. **`test_fp_math` compiles to the same size as `simple_test`** (both 8,826 bytes). All
   floating-point operations are constant-folded at compile time; no RV32F instructions are
   generated in the `main` function path beyond the comparison checks.

7. **`.eh_frame` is a significant contributor to read-only data.** This DWARF unwind table grows
   proportionally with the number of function call frames. `println_test` has the largest
   `.eh_frame` (1,564 bytes) due to its deep call chain. For embedded targets that never use
   C++ exceptions or backtraces, this section could be eliminated by passing `-C panic=abort`
   to `rustc`.

8. **All data sections (`.data`) are zero.** No binary uses pre-initialized mutable global data.
   All mutable global state is either zero-initialized (`.bss`) or uninitialized (`.uninit`).

---

## Appendix: Raw `size` Tool Output

The following is the output of `riscv64-unknown-elf-size` for each binary (Berkeley format).
Note that `bss` in this output combines the DRAM `.bss` and the SRAM memsz (stack + heap = 8,192).

```
Binary                      text    data     bss   total
rust_test                    686       0    8192    8878
simple_test                  634       0    8192    8826
test_fp_math                 634       0    8192    8826
test_atomic_simple           722       0    8192    8914
test_panic                   790       0    8192    8982
test_memory_pattern          694       0    8192    8886
test_audio_loop              906       0    8192    9098
test_audio_pattern           982       0    8192    9174
test_image_data              998       0    8192    9190
test_led_loop                990       0    8192    9182
test_dma_copy               1406       0    8192    9598
test_atomic                 1194       0    8196    9390
hello_world                 2122       0    8192   10314
test_byte_store_simple      2346       0    8192   10538
test_sim_view               2650       0    8192   10842
test_stack_memory           2586       0    8192   10778
test_static_heap            2454       0    8192   10646
test_video_loop             2242       0    8192   10434
test_video_pattern          2422       0    8192   10614
test_alloc_only             4246       0    8220   12466
test_heap_directly          4818       0    8220   13038
test_allocator              6366       0    8220   14586
minimal_postcard_test       8446       0    8220   16666
minimal_postcard_test2      8486       0    8220   16706
minimal_debug_test          8766       0    8220   16986
println_test               11678       0    8220   19898
packet_test                12290       0    8220   20510
```

> In the `size` tool output, `bss = 8192` means no DRAM BSS (only SRAM);
> `bss = 8196` means 4 bytes DRAM BSS + 8192 SRAM;
> `bss = 8220` means 28 bytes DRAM BSS + 8192 SRAM.

---

*Report generated from release builds targeting `riscv32imafc-unknown-none-elf`.*

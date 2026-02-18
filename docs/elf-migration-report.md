# ELF Migration Report: cpu-sim Test Cases

This report documents the analysis of all test cases in the `cpu-sim` crate that used ELF
files and the outcome of the migration effort to replace them with directly specified program
instructions.

## Summary

| Test file | Test name | ELF used | Migrated? |
|---|---|---|---|
| `test_simulation_core.rs` | `test_global_max_cycles_safety_margin` | `hello_world` | ✅ Migrated |
| `test_trace.rs` | `test_vcd_generation` | `simple_test` | ✅ Migrated |
| `test_fifo.rs` | `test_fifo_hello_world` | `hello_world` | ✅ Migrated |
| `test_dma.rs` | `test_dma_copy` | `test_dma_copy` | ✅ Migrated |
| `test_interactive_simulator.rs` | `test_interactive_simulator_step_cycle` | `simple_test` | ✅ Migrated |
| `test_interactive_simulator.rs` | `test_interactive_simulator_simple_program` | `simple_test` | ✅ Migrated |
| `test_interactive_simulator.rs` | `test_interactive_simulator_multiple_programs` | `simple_test` | ✅ Migrated |
| `test_interactive_simulator.rs` | `test_interactive_simulator_step_result` | `simple_test` | ✅ Migrated |
| `test_interactive_simulator.rs` | `test_interactive_simulator_load_elf` | `simple_test` | ❌ Cannot migrate |
| `test_interactive_simulator.rs` | `test_interactive_simulator_register_video_device` | `test_video_pattern` | ❌ Cannot migrate |
| `test_interactive_simulator.rs` | `test_interactive_simulator_register_audio_device` | `test_audio_pattern` | ❌ Cannot migrate |
| `test_packet_protocol.rs` | `test_packet_protocol_end_to_end` | `packet_test` | ❌ Cannot migrate |
| `test_packet_protocol.rs` | `test_println_macro` | `println_test` | ❌ Cannot migrate |

---

## Migrated Tests

The following tests were successfully converted to use directly specified instruction sequences.
New helper functions were added to `tests/common/mod.rs` to support these migrations.

### `test_global_max_cycles_safety_margin` (`test_simulation_core.rs`)

**Original:** Loaded the `hello_world` ELF (a FIFO echo program) to get a non-trivial cycle
count and verify GLOBAL_MAX_CYCLES has sufficient headroom.

**Replacement:** A new `create_loop_program(iterations)` helper executes a tight decrement loop
for 500 iterations and then exits via tohost. This exercises ~18 000 cycles — much more than the
simple arithmetic test and still well within the 50 % threshold, giving a meaningful measurement
of the safety margin.

---

### `test_vcd_generation` (`test_trace.rs`)

**Original:** Used `run_elf` with `simple_test` (a program that immediately writes SUCCESS_CODE
to tohost) solely to produce waveform data.

**Replacement:** Uses `run_program` with the existing `create_test_program()` helper. The VCD
content assertions are unchanged; the program produces the same observable CPU activity.

---

### `test_fifo_hello_world` (`test_fifo.rs`)

**Original:** Loaded `hello_world` — a bare-metal Rust program that reads words from the
FIFO RX queue and echoes them back until the queue is empty or a zero word is received.

**Replacement:** A new `create_fifo_echo_program()` helper implements the identical echo logic
in 14 RISC-V instructions using direct loads/stores to the FIFO memory-mapped registers:

```
lui  x1, FIFO_DATA          // x1 = FIFO_DATA address  (0x4000_3000)
addi x2, x1, 4              // x2 = FIFO_STATUS address (0x4000_3004)
loop:
  lw   x3, 0(x2)            // x3 = FIFO_STATUS
  andi x4, x3, 1            // x4 = RX_VALID bit
  beq  x4, x0, done         // if RX empty → done
  lw   x5, 0(x1)            // x5 = word from FIFO_DATA
  beq  x5, x0, done         // if zero word → done
  sw   x5, 0(x1)            // echo word to TX
  jal  x0, loop
done:
  lui  x6, SIM_CONTROL_BASE
  addi x7, x0, SUCCESS_CODE
  sw   x7, 0(x6)
  ebreak
  jal  x0, 0
```

TX_READY is always 1 in simulation (infinite buffer), so the write-readiness check present in
the original Rust code is safely omitted.

---

### `test_dma_copy` (`test_dma.rs`)

**Original:** Loaded `test_dma_copy` — a bare-metal Rust program that writes a 64-byte
sequential pattern to `SRC_BASE`, clears `DST_BASE`, triggers a DMA copy, polls for completion,
and verifies each byte.

**Replacement:** A new `create_dma_copy_program()` helper implements the same sequence in 38
RISC-V instructions using three loops (write-pattern, clear, verify) and direct writes to the
DMA peripheral registers:

- Loops use `SB`/`LBU` for byte-level access (same as the original Rust code).
- DMA control sequence: write `DMA_SRC_ADDR`, `DMA_DST_ADDR`, `DMA_SIZE`, then `DMA_DISPATCH`.
- Completion detection: poll `DMA_STATUS` until BUSY bit (bit 0) clears.
- Failure path: writes tohost = 1 on any byte mismatch.

---

### `test_interactive_simulator_step_cycle`, `test_interactive_simulator_simple_program`, `test_interactive_simulator_multiple_programs`, `test_interactive_simulator_step_result` (`test_interactive_simulator.rs`)

**Original:** All four tests loaded `simple_test` (a bare-metal Rust program that immediately
writes SUCCESS_CODE = 42 to tohost) via `InteractiveSimulator::load_elf`.

**Replacement:** A new `create_simple_exit_program()` helper produces the same five-instruction
program in raw bytes:

```
lui  x1, SIM_CONTROL_BASE
addi x2, x0, 42
sw   x2, 0(x1)
ebreak
jal  x0, 0
```

A new `InteractiveSimulator::load_program(start_addr, &[u8])` API method was added to write
raw program bytes into simulator memory and boot the CPU from that address. Its implementation
is identical to `load_elf_internal` except it calls `view.write_memory_region` instead of the
ELF loader.

---

## Tests That Cannot Be Migrated

The following tests fundamentally require ELF files and cannot be replaced with directly
specified instruction sequences.

---

### `test_interactive_simulator_load_elf` (`test_interactive_simulator.rs`)

**ELF used:** `simple_test`

**Reason:** This test exists specifically to verify that the `InteractiveSimulator::load_elf`
public API works correctly — it calls `load_elf`, checks the `Result` is `Ok`, and nothing more.
By definition the test cannot be implemented without actually loading an ELF file; replacing the
ELF would mean not testing the API under test.

---

### `test_interactive_simulator_register_video_device` (`test_interactive_simulator.rs`)

**ELF used:** `test_video_pattern`

**Reason:** The program renders three distinct video frames (64 × 64 pixels, RGBA8 — 16 384 bytes
per frame) into a framebuffer, configures the Video peripheral, and triggers three present
operations with back-pressure handshaking. The full framebuffer fill would require either
16 384 store instructions per frame (impractical) or a nested loop whose byte count alone would
exceed a "comparable to existing inline tests" threshold. The present/ready handshake protocol
adds further non-trivial polling logic. The test provides meaningful coverage of the video device
and its interaction with the simulator, and the program complexity is best maintained in Rust
source.

---

### `test_interactive_simulator_register_audio_device` (`test_interactive_simulator.rs`)

**ELF used:** `test_audio_pattern`

**Reason:** The program generates a sine wave (requiring integer trigonometry via a lookup
table), writes 500 stereo samples across multiple DMA batches, and implements back-pressure
polling between the DMA and the audio peripheral's sample-buffer-ready signal. Expressing
sine-wave generation in raw RISC-V instructions is impractical, and the multi-stage DMA/audio
interlock protocol would require dozens of additional instructions not comparable to the
existing inline test patterns.

---

### `test_packet_protocol_end_to_end` (`test_packet_protocol.rs`)

**ELF used:** `packet_test`

**Reason:** The CPU-side program uses:
- `extern crate alloc` — heap allocation at runtime
- The `postcard` crate for `serde`-based binary serialization of structured packets
  (`DebugPacket`, `EchoPacket`, `DataU32Packet`, `AssertPacket`)
- Dynamic `String` construction

These requirements cannot be expressed in raw RISC-V instructions; they demand a full Rust
runtime with heap allocation, a serialization library, and the `riscv_shared::protocol`
type system. The test verifies the complete bidirectional packet communication protocol
between CPU and host and must remain ELF-based.

---

### `test_println_macro` (`test_packet_protocol.rs`)

**ELF used:** `println_test`

**Reason:** The program exercises the `rvprintln!` macro from `riscv_shared`, which internally
uses `postcard` serialization to encode formatted `DebugPacket` messages and write them
word-by-word to the FIFO TX register. The macro expansion depends on heap allocation
(`extern crate alloc`) and the full `postcard`/`serde` serialization stack. The test verifies
that the macro produces correctly serialized packets with the right magic number, packet type,
debug level, and message content. This behavior cannot be reproduced with a fixed instruction
sequence.

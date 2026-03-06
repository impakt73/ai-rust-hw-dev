# Memory Map

This document is the **source of truth** for all memory-mapped addresses used in the project.

The canonical definitions of memory-map ranges and base addresses live in [`riscv_shared/src/bus.rs`](../riscv_shared/src/bus.rs); individual peripherals may define additional address and register constants in their own modules (for example, `riscv_shared/src/dma.rs`, `riscv_shared/src/sim_control.rs`, and `riscv_shared/src/fifo.rs`).

## Address Map Overview

```
Address Range            | Device              | Type | Description
-------------------------|---------------------|------|----------------------------
0xF0000000 - 0xF0000003  | SimControl          | Rust | Simulation control (tohost)
0x90000000 - 0x9000000F  | Video               | Rust | Video frame buffer
0xA0000000 - 0xA000000F  | Audio               | Rust | Audio buffer
0xB0000000 - 0xB0000007  | FIFO                | Rust | Host communication FIFO
0xC0000000 - 0xC0000013  | DMA                 | Rust | DMA controller
0x20000000 - 0x2000000F  | System Controller   | RTL  | CPU boot and reset control
0x50000000 - 0x5000000F  | LED Controller      | RTL  | 8-bit LED output register
0x60000000 - 0x6000000F  | Clock Peripheral    | RTL  | Elapsed time counters (us/ms/s)
0x70000000 - 0x70002FFF  | SRAM Peripheral     | RTL  | 12KB on-chip SRAM
0x80000000 - 0xFFFFFFFF  | DRAM                | Both | System memory (2 GiB)
```

## Peripheral Types

- **Rust peripherals** (`0x80000000 - 0xFFFFFFFF`):
  Handled by the Rust `SystemBus`, simulation-only. Not synthesized to FPGA.
- **RTL peripherals** (`0x00000000 - 0x7FFFFFFF`):
  Handled by the SystemVerilog `bus.sv` address decoder (top-bit split + top-nibble select),
  synthesizable to FPGA.
- **DRAM** (`0x80000000 - 0xFFFFFFFF`): Main system memory. Accessed through the external
  memory interface on both RTL and Rust sides.

## Rust Peripherals (upper-half address space)

### SimControl (0xF0000000)

The SimControl device provides the `tohost` register used to signal program
termination to the simulator.

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | TOHOST   | WO     | Write to halt simulation. Value is captured as exit code. |

**Constants:** `SIM_CONTROL_BASE`, `TOHOST_ADDR`

**Usage:**
```rust
lui(reg, SIM_CONTROL_BASE);  // Load SimControl base address
addi(val, 0, 42);            // Load success code
sw(reg, val, 0);             // Write to tohost → halts simulation
```

### Video (0x90000000)

The Video device provides a framebuffer interface for rendering.

| Offset | Register      | Access | Description |
|--------|---------------|--------|-------------|
| 0x00   | VIDEO_ADDR    | RW     | Framebuffer address in DRAM |
| 0x04   | VIDEO_CONFIG  | RW     | Width, height, format configuration |
| 0x08   | VIDEO_STATUS  | RO     | Status flags (FRAME_READY, PRESENT_READY) |
| 0x0C   | VIDEO_PRESENT | WO     | Trigger frame presentation |

**Constants:** `VIDEO_BASE`, `VIDEO_ADDR`, `VIDEO_CONFIG`, `VIDEO_STATUS`, `VIDEO_PRESENT`

### Audio (0xA0000000)

The Audio device provides a sample buffer interface for audio output.

| Offset | Register     | Access | Description |
|--------|--------------|--------|-------------|
| 0x00   | AUDIO_ADDR   | RW     | Sample buffer address in DRAM |
| 0x04   | AUDIO_CONFIG | RW     | Sample rate, channels, sample count |
| 0x08   | AUDIO_STATUS | RO     | Status flags (DMA_READY, SAMPLE_BUFFER_READY) |
| 0x0C   | AUDIO_DMA    | WO     | Trigger DMA transfer from buffer |

**Constants:** `AUDIO_BASE`, `AUDIO_ADDR`, `AUDIO_CONFIG`, `AUDIO_STATUS`, `AUDIO_DMA`

### FIFO (0xB0000000)

The FIFO device provides bidirectional host communication using a packet protocol.

| Offset | Register    | Access | Description |
|--------|-------------|--------|-------------|
| 0x00   | FIFO_DATA   | RW     | Read/write FIFO data (32-bit words) |
| 0x04   | FIFO_STATUS | RO     | Status: bit 0 = RX_VALID, bit 1 = TX_READY |

**Constants:** `FIFO_BASE`, `FIFO_DATA`, `FIFO_STATUS`, `RX_VALID`, `TX_READY`

### DMA (0xC0000000)

The DMA device provides hardware-accelerated memory-to-memory transfers.

| Offset | Register     | Access | Description |
|--------|--------------|--------|-------------|
| 0x00   | DMA_SRC_ADDR | RW     | Source address |
| 0x04   | DMA_DST_ADDR | RW     | Destination address |
| 0x08   | DMA_SIZE     | RW     | Transfer size in bytes |
| 0x0C   | DMA_STATUS   | RO     | Status: bit 0 = BUSY |
| 0x10   | DMA_DISPATCH | WO     | Write 1 to trigger transfer |

**Constants:** `DMA_BASE`, `DMA_SRC_ADDR`, `DMA_DST_ADDR`, `DMA_SIZE`, `DMA_STATUS`, `DMA_DISPATCH`

## RTL Peripherals (top-nibble decoded windows)

### LED Controller (0x50000000)

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | LED_OUT  | RW     | Bits [7:0]: LED output data |

- **Access sizes:** Byte, halfword, word
- **Latency:** Single-cycle (ready = 1'b1)
- **Constants:** `LED_BASE`, `LED_SIZE`, `LED_OUT_OFFSET`

### Clock Peripheral (0x60000000)

| Offset | Register   | Access | Description |
|--------|------------|--------|-------------|
| 0x00   | ELAPSED_US | RO     | Elapsed microseconds since reset |
| 0x04   | ELAPSED_MS | RO     | Elapsed milliseconds since reset |
| 0x08   | ELAPSED_S  | RO     | Elapsed seconds since reset |

- **Access sizes:** Word (32-bit)
- **Latency:** Single-cycle (ready = 1'b1)
- **Note:** Clock frequency is configurable via `CLK_FREQ_HZ` parameter
- **Constants:** `CLOCK_BASE`, `CLOCK_SIZE`, `CLOCK_ELAPSED_US_OFFSET`, `CLOCK_ELAPSED_MS_OFFSET`, `CLOCK_ELAPSED_S_OFFSET`

### SRAM Peripheral (0x70000000)

12KB of general-purpose on-chip SRAM. Used by `rust-test-program` for the text, rodata, data, bss, and stack sections.

- **Size:** 12KB (0x70000000 – 0x70002FFF)
- **Access sizes:** Byte, halfword, word
- **Latency:** Writes: single-cycle (ready asserted immediately); Reads: 1-cycle latency (ready asserted in the cycle after request)
- **Constants:** `SRAM_BASE`, `SRAM_SIZE`

### System Controller (0x20000000)

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | STATUS   | RO     | Bit 0: cpu_booting, Bit 1: cpu_halted |
| 0x04   | RESET    | WO     | Write 1: system reset, Write 2: CPU reset |
| 0x08   | BOOT     | WO     | Write boot address to start CPU |
| 0x0C   | HALT     | RW     | Halt termination code (write requests CPU halt next cycle) |

- **Constants:** `SYSCTRL_BASE`, `SYSCTRL_SIZE`, `SYSCTRL_STATUS_OFFSET`, `SYSCTRL_RESET_OFFSET`, `SYSCTRL_BOOT_OFFSET`, `SYSCTRL_HALT_OFFSET`

## DRAM (0x80000000 - 0xFFFFFFFF)

System memory occupying the upper 2 GiB of the 32-bit address space. All test
programs should use addresses within this range for data storage.

**Constants:** `DRAM_BASE`, `DRAM_END`

```rust
lui(reg, 0x80000000);  // Load DRAM base address
sw(reg, val, offset);   // Store data
```

# Memory Map

This document is the **source of truth** for all memory-mapped addresses used in the project.

The canonical definitions of memory-map ranges and base addresses live in [`riscv_shared/src/bus.rs`](../riscv_shared/src/bus.rs); individual peripherals may define additional address and register constants in their own modules (for example, `riscv_shared/src/dma.rs`, `riscv_shared/src/sim_control.rs`, and `riscv_shared/src/fifo.rs`).

## Address Map Overview

```
Address Range            | Device              | Type | Description
-------------------------|---------------------|------|----------------------------
0x40000000 - 0x40000003  | SimControl          | Rust | Simulation control (tohost)
0x40001000 - 0x4000100F  | Video               | Rust | Video frame buffer
0x40002000 - 0x4000200F  | Audio               | Rust | Audio buffer
0x40003000 - 0x40003007  | FIFO                | Rust | Host communication FIFO
0x40004000 - 0x40004013  | DMA                 | Rust | DMA controller
0x40005000 - 0x4FFFFFFF  | Reserved (Rust)     | Rust | Reserved for future Rust peripherals
0x50000000 - 0x5000000F  | LED Controller      | RTL  | 8-bit LED output register
0x51000000 - 0x5100000F  | Clock Peripheral    | RTL  | Elapsed time counters (us/ms/s)
0x52000000 - 0x520000FF  | UART Controller     | RTL  | UART TX/RX with 8-byte FIFOs
0x53000000 - 0x5300000F  | System Controller   | RTL  | CPU boot and reset control
0x53000010 - 0x5FFFFFFF  | Reserved (RTL)      | RTL  | Reserved for future RTL peripherals
0x80000000 - 0xFFFFFFFF  | DRAM                | Both | System memory (2 GiB)
```

## Peripheral Types

- **Rust peripherals** (`0x40000000 - 0x4FFFFFFF`): Handled by the Rust `SystemBus`,
  simulation-only. Not synthesized to FPGA.
- **RTL peripherals** (`0x50000000 - 0x5FFFFFFF`): Handled by the SystemVerilog `bus.sv`
  address decoder, synthesizable to FPGA.
- **DRAM** (`0x80000000 - 0xFFFFFFFF`): Main system memory. Accessed through the external
  memory interface on both RTL and Rust sides.

## Rust Peripherals (0x40000000 - 0x4FFFFFFF)

### SimControl (0x40000000)

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

### Video (0x40001000)

The Video device provides a framebuffer interface for rendering.

| Offset | Register      | Access | Description |
|--------|---------------|--------|-------------|
| 0x00   | VIDEO_ADDR    | RW     | Framebuffer address in DRAM |
| 0x04   | VIDEO_CONFIG  | RW     | Width, height, format configuration |
| 0x08   | VIDEO_STATUS  | RO     | Status flags (FRAME_READY, PRESENT_READY) |
| 0x0C   | VIDEO_PRESENT | WO     | Trigger frame presentation |

**Constants:** `VIDEO_BASE`, `VIDEO_ADDR`, `VIDEO_CONFIG`, `VIDEO_STATUS`, `VIDEO_PRESENT`

### Audio (0x40002000)

The Audio device provides a sample buffer interface for audio output.

| Offset | Register     | Access | Description |
|--------|--------------|--------|-------------|
| 0x00   | AUDIO_ADDR   | RW     | Sample buffer address in DRAM |
| 0x04   | AUDIO_CONFIG | RW     | Sample rate, channels, sample count |
| 0x08   | AUDIO_STATUS | RO     | Status flags (DMA_READY, SAMPLE_BUFFER_READY) |
| 0x0C   | AUDIO_DMA    | WO     | Trigger DMA transfer from buffer |

**Constants:** `AUDIO_BASE`, `AUDIO_ADDR`, `AUDIO_CONFIG`, `AUDIO_STATUS`, `AUDIO_DMA`

### FIFO (0x40003000)

The FIFO device provides bidirectional host communication using a packet protocol.

| Offset | Register    | Access | Description |
|--------|-------------|--------|-------------|
| 0x00   | FIFO_DATA   | RW     | Read/write FIFO data (32-bit words) |
| 0x04   | FIFO_STATUS | RO     | Status: bit 0 = RX_VALID, bit 1 = TX_READY |

**Constants:** `FIFO_BASE`, `FIFO_DATA`, `FIFO_STATUS`, `RX_VALID`, `TX_READY`

### DMA (0x40004000)

The DMA device provides hardware-accelerated memory-to-memory transfers.

| Offset | Register     | Access | Description |
|--------|--------------|--------|-------------|
| 0x00   | DMA_SRC_ADDR | RW     | Source address |
| 0x04   | DMA_DST_ADDR | RW     | Destination address |
| 0x08   | DMA_SIZE     | RW     | Transfer size in bytes |
| 0x0C   | DMA_STATUS   | RO     | Status: bit 0 = BUSY |
| 0x10   | DMA_DISPATCH | WO     | Write 1 to trigger transfer |

**Constants:** `DMA_BASE`, `DMA_SRC_ADDR`, `DMA_DST_ADDR`, `DMA_SIZE`, `DMA_STATUS`, `DMA_DISPATCH`

## RTL Peripherals (0x50000000 - 0x5FFFFFFF)

### LED Controller (0x50000000)

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | LED_OUT  | RW     | Bits [7:0]: LED output data |

- **Access sizes:** Byte, halfword, word
- **Latency:** Single-cycle (ready = 1'b1)
- **Constants:** `LED_BASE`, `LED_SIZE`, `LED_OUT_OFFSET`

### Clock Peripheral (0x51000000)

| Offset | Register   | Access | Description |
|--------|------------|--------|-------------|
| 0x00   | ELAPSED_US | RO     | Elapsed microseconds since reset |
| 0x04   | ELAPSED_MS | RO     | Elapsed milliseconds since reset |
| 0x08   | ELAPSED_S  | RO     | Elapsed seconds since reset |

- **Access sizes:** Word (32-bit)
- **Latency:** Single-cycle (ready = 1'b1)
- **Note:** Clock frequency is configurable via `CLK_FREQ_HZ` parameter
- **Constants:** `CLOCK_BASE`, `CLOCK_SIZE`, `CLOCK_ELAPSED_US_OFFSET`, `CLOCK_ELAPSED_MS_OFFSET`, `CLOCK_ELAPSED_S_OFFSET`

### UART Controller (0x52000000)

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | TXDATA   | WO     | Write byte to TX FIFO |
| 0x04   | RXDATA   | RO     | Read byte from RX FIFO |
| 0x08   | STATUS   | RO     | FIFO status flags (see below) |
| 0x0C   | CTRL     | RW     | Control register (reserved) |

**STATUS Register Bits:**

| Bit | Name     | Description |
|-----|----------|-------------|
| 0   | TX_FULL  | TX FIFO is full |
| 1   | TX_EMPTY | TX FIFO is empty (all data transmitted) |
| 2   | TX_BUSY  | TX shift register is active |
| 4   | RX_FULL  | RX FIFO is full |
| 5   | RX_EMPTY | RX FIFO is empty (no data available) |
| 6   | RX_BUSY  | RX shift register is active |
| 7   | RX_ERROR | Framing error detected |

- **Size:** 256 bytes
- **Constants:** `UART_BASE`, `UART_SIZE`, `UART_TXDATA_OFFSET`, `UART_RXDATA_OFFSET`, `UART_STATUS_OFFSET`, `UART_CTRL_OFFSET`

### System Controller (0x53000000)

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | STATUS   | RO     | Bit 0: cpu_booting, Bit 1: cpu_halted |
| 0x04   | RESET    | WO     | Write 1: system reset, Write 2: CPU reset |
| 0x08   | BOOT     | WO     | Write boot address to start CPU |

- **Constants:** `SYSCTRL_BASE`, `SYSCTRL_SIZE`, `SYSCTRL_STATUS_OFFSET`, `SYSCTRL_RESET_OFFSET`, `SYSCTRL_BOOT_OFFSET`

## DRAM (0x80000000 - 0xFFFFFFFF)

System memory occupying the upper 2 GiB of the 32-bit address space. All test
programs should use addresses within this range for data storage.

**Constants:** `DRAM_BASE`, `DRAM_END`

```rust
lui(reg, 0x80000000);  // Load DRAM base address
sw(reg, val, offset);   // Store data
```

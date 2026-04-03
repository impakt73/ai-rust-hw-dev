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
0x20000000 - 0x2000001F  | System Controller   | RTL  | CPU control, LED output, elapsed time
0x30000000 - 0x300063FF  | GFX2D Peripheral    | RTL  | Scroll registers and CPU-visible tile/font/palette RAMs
0x50000000 - 0x5000000F  | Gamepad Peripheral  | RTL  | Live gamepad button state register
0x70000000 - 0x70002FFF  | SRAM Peripheral     | RTL  | 12KB on-chip SRAM
0x80000000 - 0x8FFFFFFF  | DRAM                | Rust | System memory (256 MiB)
```

## Peripheral Types

- **Rust peripherals** (`0x80000000 - 0xFFFFFFFF`):
  Handled by the Rust `SystemBus` on the host. On FPGA backends, these same
  addresses are reached remotely through the host bus interface. DRAM is part
  of this Rust peripheral range in this project.
- **RTL peripherals** (`0x00000000 - 0x7FFFFFFF`):
  Handled by the SystemVerilog `registered_bus.sv` address decoder (top-bit split + top-nibble select),
  synthesizable to FPGA.
  Decode is window-based on the top nibble (`addr[31:28]`), so accesses within a given
  256 MiB RTL window may intentionally mirror/alias at the peripheral level.
- **DRAM** (`0x80000000 - 0x8FFFFFFF`): Main system memory implemented as a
  Rust peripheral and handled by the Rust `SystemBus` path (including FPGA
  host-side access through the host bus).

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

### GFX2D Peripheral (0x30000000)

Tile/sprite text renderer control space plus the three video-clocked RAMs that back
the renderer.

| Offset | Region/Register | Access | Description |
|--------|-----------------|--------|-------------|
| 0x0000 | SCROLL_X        | RW     | Horizontal scroll value, word access only |
| 0x0004 | SCROLL_Y        | RW     | Vertical scroll value, word access only |
| 0x0008 | CONTROL         | RW     | Bit 0 holds the video sync generator in reset when set to 1; reset default is 0 |
| 0x000C | FRAME_COUNTER   | RO     | Counts completed frame-start pulses; resets to 0 on peripheral reset |
| 0x1000-0x13FF | CHAR_MAP RAM | WO | 8-bit tile IDs, byte access only |
| 0x2000-0x5FFF | FONT RAM | WO | 8-bit per-pixel palette indices, byte access only |
| 0x6000-0x63FF | PALETTE RAM | WO | 24-bit RGB entries in 32-bit words, word access only |

- **Size:** 25 KiB (`0x30000000 – 0x300063FF`) in the default 32x32-tile configuration.
- **Access sizes:** `SCROLL_X`, `SCROLL_Y`, `CONTROL`, `FRAME_COUNTER`, and `PALETTE RAM`
  accept aligned word accesses only;
  `CHAR_MAP RAM` and `FONT RAM` accept byte accesses only. Unsupported sizes are acknowledged
  and dropped.
- **CPU visibility:** the renderer owns the RAM read ports exclusively; CPU accesses to the RAM
  windows are write-only and reads return zero.
- **Palette write format:** writes ignore `wdata[31:24]`.
- **Clocking:** CPU accesses cross into the video clock domain through `bus_cdc_bridge`; the
  backing RAMs use `video_clk` on both ports.
- **Constants:** `GFX2D_BASE`, `GFX2D_SIZE`, `GFX2D_SCROLL_X_OFFSET`, `GFX2D_SCROLL_Y_OFFSET`,
  `GFX2D_CONTROL_OFFSET`, `GFX2D_FRAME_COUNTER_OFFSET`,
  `GFX2D_CONTROL_SCANOUT_HOLD_RESET`, `GFX2D_CHAR_MAP_OFFSET`, `GFX2D_CHAR_MAP_SIZE`,
  `GFX2D_FONT_OFFSET`, `GFX2D_FONT_SIZE`, `GFX2D_PALETTE_OFFSET`, `GFX2D_PALETTE_SIZE`

### Gamepad Peripheral (0x50000000)

Single read-only register exposing live button state sampled in the system bus clock domain.

| Offset | Register | Access | Description |
|--------|----------|--------|-------------|
| 0x00   | GAMEPAD_STATE | RO | Bits [9:0] reflect dpad/buttons/triggers; bits [31:10] read as 0 |

- **Bit mapping:** [0] up, [1] down, [2] left, [3] right, [4] A, [5] B, [6] X, [7] Y, [8] L, [9] R
- **Access sizes:** Only aligned 32-bit word reads from offset `0x00` return button state. Byte reads, halfword reads, unaligned word reads, and reads to other offsets are acknowledged but return zero.
- **Latency:** Single response register in the bus clock domain (no CDC path).
- **Platform note:** The Analogue Pocket top inverts `cont1_key[9:0]` before driving the peripheral because Pocket button inputs are active-low.
- **Constants:** `GAMEPAD_BASE`, `GAMEPAD_SIZE`, `GAMEPAD_STATE_OFFSET`, `GAMEPAD_DPAD_UP`, `GAMEPAD_DPAD_DOWN`, `GAMEPAD_DPAD_LEFT`, `GAMEPAD_DPAD_RIGHT`, `GAMEPAD_BTN_A`, `GAMEPAD_BTN_B`, `GAMEPAD_BTN_X`, `GAMEPAD_BTN_Y`, `GAMEPAD_TRIG_L`, `GAMEPAD_TRIG_R`

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
| 0x04   | RESET    | WO     | Write-data bit 0 selects reset type: 0 = system reset, 1 = CPU reset (halt → reset pulse → wait for cpu_booting) |
| 0x08   | BOOT     | WO     | Write boot address to start CPU |
| 0x0C   | HALT     | RW     | Halt termination code (write requests CPU halt next cycle) |
| 0x10   | LED_OUT  | RW     | Bits [7:0]: LED output data |
| 0x14   | ELAPSED_US | RO   | Elapsed microseconds since reset |
| 0x18   | ELAPSED_MS | RO   | Elapsed milliseconds since reset |
| 0x1C   | ELAPSED_S  | RO   | Elapsed seconds since reset |

- **Access sizes:** `LED_OUT` supports byte, halfword, and word accesses; the elapsed-time registers are word reads.
- **Latency:** Single-cycle (ready = 1'b1)
- **Note:** Elapsed-time counters use the system controller `CLK_FREQ_HZ` parameter.
- **Constants:** `SYSCTRL_BASE`, `SYSCTRL_SIZE`, `SYSCTRL_STATUS_OFFSET`, `SYSCTRL_RESET_OFFSET`, `SYSCTRL_BOOT_OFFSET`, `SYSCTRL_HALT_OFFSET`, `SYSCTRL_LED_OUT_OFFSET`, `SYSCTRL_ELAPSED_US_OFFSET`, `SYSCTRL_ELAPSED_MS_OFFSET`, `SYSCTRL_ELAPSED_S_OFFSET`
- **CPU reset sequencing:** `RESET` writes with bit 0 set hold `req_cpu_halt` high until `cpu_halted`, pulse `cpu_rst_n` low for one cycle, and block new A-channel requests until `cpu_booting` reasserts and the D-channel completion response is returned.

## DRAM (0x80000000 - 0x8FFFFFFF)

System memory occupying a 256 MiB window in the upper-half Rust peripheral space. All test
programs should use addresses within this range for data storage.

**Constants:** `DRAM_BASE`, `DRAM_END`

```rust
lui(reg, 0x80000000);  // Load DRAM base address
sw(reg, val, offset);   // Store data
```

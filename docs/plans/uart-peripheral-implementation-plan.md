# UART Peripheral Implementation Plan
## RTL-Based UART with FIFO Interface

**Author:** GitHub Copilot Hardware-Software Integration Architect  
**Date:** January 28, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Based on:** `rtl/peripherals/led_controller.sv` reference implementation  
**Status:** Completed

> **Note:** This is a historical planning document. The module names have since been updated:
> - `top.sv` → renamed to `cpu.sv` (CPU core module)
> - `top_with_peripherals.sv` → renamed to `top.sv` (top-level module with RTL peripherals)

---

## Executive Summary

This document provides a detailed technical implementation plan for adding an RTL-based UART (Universal Asynchronous Receiver/Transmitter) peripheral to the RISC-V CPU. The UART will:

1. **Support configurable baud rates** via module parameters
2. **Allow system clock frequency configuration** for clock-domain independence
3. **Use 8N1 format** (8 data bits, no parity, 1 stop bit)
4. **Provide FIFO-based TX/RX interfaces** for easy CPU interaction
5. **Expose status registers** for flow control (FIFO full/empty)
6. **Connect directly to external RX/TX pins** for FPGA deployment
7. **Support hardware loopback mode** for simulation testing (TX internally connected to RX)

**Key Architecture Decisions:**
- **Single Clock Domain:** Run UART logic on system clock with oversampling. This simplifies the design by avoiding separate clock domains while maintaining robust data recovery.
- **Hardware Loopback for Testing:** A new `ENABLE_UART_LOOPBACK` parameter (enabled by default) connects TX directly to RX internally. This allows CPU test programs to validate UART functionality without requiring Rust-side signal injection, greatly simplifying the verification infrastructure.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Module Parameters](#2-module-parameters)
3. [Memory Map Design](#3-memory-map-design)
4. [Register Specification](#4-register-specification)
5. [RTL Design](#5-rtl-design)
6. [Top-Level Integration](#6-top-level-integration)
7. [Rust Integration Layer](#7-rust-integration-layer)
8. [Testing Strategy](#8-testing-strategy)
9. [Implementation Checklist](#9-implementation-checklist)
10. [Future Extensions](#10-future-extensions)

---

## 1. Architecture Overview

### 1.1 UART Block Diagram

```
                      ┌─────────────────────────────────────────────────────────────┐
                      │                    uart.sv                                   │
                      │                                                              │
  ┌───────────────────┼───────────────────────┐   ┌─────────────────────────────────┐
  │   CPU Interface   │                       │   │    External Pins                │
  │                   │                       │   │                                 │
  │  addr[31:0] ──────┼──▶ ┌─────────────┐    │   │                                 │
  │  wdata[31:0] ─────┼──▶ │  Register   │    │   │                                 │
  │  rdata[31:0] ◀────┼─── │  File       │    │   │                                 │
  │  we ──────────────┼──▶ │             │    │   │                                 │
  │  re ──────────────┼──▶ │ TXDATA      │    │   │                                 │
  │  size[1:0] ───────┼──▶ │ RXDATA      │    │   │                                 │
  │  ready ◀──────────┼─── │ STATUS      │    │   │                                 │
  │                   │    └─────┬───────┘    │   │                                 │
  └───────────────────┼──────────┼────────────┘   │                                 │
                      │          │                │                                 │
                      │    ┌─────▼─────┐          │                                 │
                      │    │  TX FIFO  │──────────┼──▶ ┌──────────┐                 │
                      │    │  (8 deep) │          │    │    TX    │    tx_out ──────┼──▶
                      │    └───────────┘          │    │  Shift   │                 │
                      │                           │    │  Reg     │                 │
                      │    ┌───────────┐          │    └──────────┘                 │
                      │    │  RX FIFO  │◀─────────┼─── ┌──────────┐                 │
                      │    │  (8 deep) │          │    │    RX    │    rx_in ◀──────┼───
                      │    └───────────┘          │    │  Shift   │                 │
                      │                           │    │  Reg     │                 │
                      │    ┌───────────┐          │    └──────────┘                 │
                      │    │   Baud    │          │                                 │
                      │    │   Rate    │          │                                 │
                      │    │ Generator │          │                                 │
                      │    └───────────┘          │                                 │
                      │                           │                                 │
                      └───────────────────────────┴─────────────────────────────────┘
```

### 1.2 Design Philosophy

**Single Clock Domain:**
- All UART logic runs on the system clock (`clk`)
- Baud rate generation uses a clock divider (counter-based)
- RX oversampling (16x) for robust bit sampling
- No clock domain crossing complexities

**FIFO-Based Interface:**
- 8-entry TX FIFO: CPU writes bytes, UART transmits asynchronously
- 8-entry RX FIFO: UART receives bytes, CPU reads when ready
- Status register indicates FIFO full/empty states
- Decouples CPU timing from UART baud rate

**8N1 Format:**
- 8 data bits (fixed)
- No parity (simplified design)
- 1 stop bit (standard)
- Start bit detection with oversampling

### 1.3 Key Features

| Feature | Description |
|---------|-------------|
| Data Width | 8 bits (fixed) |
| Stop Bits | 1 bit (fixed) |
| Parity | None (fixed for initial implementation) |
| TX FIFO | 8 entries x 8 bits |
| RX FIFO | 8 entries x 8 bits |
| Baud Rate | Configurable via module parameter |
| Clock | System clock (configurable frequency parameter) |
| Flow Control | None (status-based polling) |

---

## 2. Module Parameters

### 2.1 Core Parameters

```systemverilog
module uart #(
    // System clock frequency in Hz (required for baud rate calculation)
    // Example: 50_000_000 for 50 MHz, 100_000_000 for 100 MHz
    parameter int CLK_FREQ_HZ = 50_000_000,
    
    // Target baud rate in bits per second
    // Standard rates: 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600
    parameter int BAUD_RATE = 115200,
    
    // FIFO depth (number of entries in TX and RX FIFOs)
    // Must be power of 2 for efficient pointer arithmetic
    parameter int FIFO_DEPTH = 8
) (
    // ... ports ...
);
```

### 2.2 Parameter Validation

At synthesis/elaboration time, the module should validate parameters:

```systemverilog
// Compile-time assertions
initial begin
    // Validate FIFO depth is power of 2
    if ((FIFO_DEPTH & (FIFO_DEPTH - 1)) != 0 || FIFO_DEPTH < 2) begin
        $fatal(1, "UART: FIFO_DEPTH must be power of 2 and >= 2, got %0d", FIFO_DEPTH);
    end
    
    // Validate baud rate is achievable with given clock
    if (CLK_FREQ_HZ / BAUD_RATE < 16) begin
        $fatal(1, "UART: Baud rate %0d too high for clock %0d Hz (need 16x oversampling)",
               BAUD_RATE, CLK_FREQ_HZ);
    end
end
```

### 2.3 Baud Rate Calculation

**Clock Divider Value:**
```
CLKS_PER_BIT = CLK_FREQ_HZ / BAUD_RATE
```

**Example Values:**

| CLK_FREQ_HZ | BAUD_RATE | CLKS_PER_BIT | Actual Baud Rate | Error |
|-------------|-----------|--------------|------------------|-------|
| 50,000,000  | 115,200   | 434          | 115,207          | 0.01% |
| 50,000,000  | 9,600     | 5,208        | 9,600            | 0.00% |
| 100,000,000 | 115,200   | 868          | 115,207          | 0.01% |
| 25,000,000  | 115,200   | 217          | 115,207          | 0.01% |

**RX Oversampling:**
- Sample RX at 16x baud rate
- Take sample at bit center (count 7-8) for noise immunity
- `CLKS_PER_SAMPLE = CLK_FREQ_HZ / (BAUD_RATE * 16)`

---

## 3. Memory Map Design

### 3.1 UART Address Allocation

Based on the existing memory map in `AGENTS.md` and `rtl-peripheral-implementation-plan.md`:

```
Address Range          | Device           | Type | Size    | Description
-----------------------|------------------|------|---------|----------------------------
0x52000000-0x520000FF | UART             | RTL  | 256 B   | UART controller
```

**UART Base Address:** `0x52000000`

### 3.2 Register Offsets

```
Offset | Name      | Access | Reset      | Description
-------|-----------|--------|------------|---------------------------------------
0x00   | TXDATA    | WO     | -          | Transmit data register (write to TX FIFO)
0x04   | RXDATA    | RO     | -          | Receive data register (read from RX FIFO)
0x08   | STATUS    | RO     | 0x00000022 | Status register (FIFO status, errors)
0x0C   | CTRL      | RW     | 0x00000000 | Control register (reserved for future)
0x10   | Reserved  | -      | -          | Reserved for future use
...    | ...       | ...    | ...        | ...
0xFC   | Reserved  | -      | -          | Reserved for future use
```

### 3.3 Address Decoder Update

**In top_with_peripherals.sv:**

```systemverilog
// Address range definitions (add to existing constants)
localparam UART_BASE  = 32'h52000000;
localparam UART_LIMIT = 32'h52000100;  // 256 bytes

// Updated address decoder
logic sel_led;
logic sel_uart;
logic sel_external;
logic sel_unmapped_rtl;

always_comb begin
    sel_led          = 1'b0;
    sel_uart         = 1'b0;
    sel_unmapped_rtl = 1'b0;
    sel_external     = 1'b0;
    
    if (cpu_dmem_addr >= LED_BASE && cpu_dmem_addr < LED_LIMIT) begin
        sel_led = 1'b1;
    end
    else if (cpu_dmem_addr >= UART_BASE && cpu_dmem_addr < UART_LIMIT) begin
        sel_uart = 1'b1;
    end
    else if (cpu_dmem_addr >= RTL_PERIPH_BASE && cpu_dmem_addr < RTL_PERIPH_LIMIT) begin
        sel_unmapped_rtl = 1'b1;
    end
    else begin
        sel_external = 1'b1;
    end
end
```

---

## 4. Register Specification

### 4.1 TXDATA Register (0x52000000)

**Purpose:** Write data to transmit FIFO

| Bits | Name | Access | Description |
|------|------|--------|-------------|
| 7:0  | DATA | WO     | Byte to transmit |
| 31:8 | -    | -      | Reserved (writes ignored) |

**Behavior:**
- Writing to this register pushes a byte to the TX FIFO
- If TX FIFO is full, write has no effect (check STATUS.TX_FULL first)
- Only lower 8 bits are used; upper bits are ignored
- Reading returns 0 (write-only register)

**CPU Usage Example:**
```c
// Check if TX FIFO has space
while (UART_STATUS & UART_TX_FULL);  // Wait for space

// Write byte to transmit
UART_TXDATA = byte_to_send;
```

### 4.2 RXDATA Register (0x52000004)

**Purpose:** Read data from receive FIFO

| Bits | Name | Access | Description |
|------|------|--------|-------------|
| 7:0  | DATA | RO     | Received byte |
| 31:8 | -    | -      | Reserved (reads as 0) |

**Behavior:**
- Reading this register pops a byte from the RX FIFO
- If RX FIFO is empty, returns 0 (check STATUS.RX_EMPTY first)
- Writing has no effect (read-only register)

**CPU Usage Example:**
```c
// Check if data is available
if (!(UART_STATUS & UART_RX_EMPTY)) {
    uint8_t received = UART_RXDATA;
    // Process received byte
}
```

### 4.3 STATUS Register (0x52000008)

**Purpose:** FIFO and transmitter/receiver status

| Bit | Name     | Access | Reset | Description |
|-----|----------|--------|-------|-------------|
| 0   | TX_FULL  | RO     | 0     | TX FIFO is full (cannot accept more data) |
| 1   | TX_EMPTY | RO     | 1     | TX FIFO is empty (all data transmitted) |
| 2   | TX_BUSY  | RO     | 0     | TX shift register active (transmitting) |
| 3   | Reserved | -      | 0     | Reserved |
| 4   | RX_FULL  | RO     | 0     | RX FIFO is full (data may be lost) |
| 5   | RX_EMPTY | RO     | 1     | RX FIFO is empty (no data available) |
| 6   | RX_BUSY  | RO     | 0     | RX shift register active (receiving) |
| 7   | RX_ERROR | RO     | 0     | Framing error detected (cleared on STATUS read) |
| 31:8| Reserved | -      | 0     | Reserved |

**Reset Value:** `0x00000022` (TX_EMPTY=1, RX_EMPTY=1)

**Status Bit Details:**

- **TX_FULL (bit 0):** Set when TX FIFO contains FIFO_DEPTH entries. CPU must wait before writing more data.
- **TX_EMPTY (bit 1):** Set when TX FIFO is empty AND TX shift register is idle. Indicates all data has been transmitted.
- **TX_BUSY (bit 2):** Set when TX shift register is actively transmitting a byte.
- **RX_FULL (bit 4):** Set when RX FIFO contains FIFO_DEPTH entries. Incoming bytes will be lost until space is made.
- **RX_EMPTY (bit 5):** Set when RX FIFO has no data. Reading RXDATA returns 0.
- **RX_BUSY (bit 6):** Set when RX shift register is actively receiving a byte.
- **RX_ERROR (bit 7):** Set on framing error (missing stop bit). Cleared when STATUS is read.

### 4.4 CTRL Register (0x5200000C)

**Purpose:** Control register (reserved for future extensions)

| Bit | Name     | Access | Reset | Description |
|-----|----------|--------|-------|-------------|
| 0   | TX_EN    | RW     | 0     | Reserved (TX always enabled in v1) |
| 1   | RX_EN    | RW     | 0     | Reserved (RX always enabled in v1) |
| 31:2| Reserved | -      | 0     | Reserved for future use |

**Reset Value:** `0x00000000`

**Note:** In the initial implementation, transmitter and receiver are always enabled. The CTRL register is reserved for future features such as:
- Enable/disable TX/RX
- Loopback mode for testing
- Interrupt enable bits
- Parity configuration

---

## 5. RTL Design

### 5.1 Module Interface

```systemverilog
module uart #(
    parameter int CLK_FREQ_HZ = 50_000_000,
    parameter int BAUD_RATE   = 115200,
    parameter int FIFO_DEPTH  = 8
) (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit)
    input  logic [31:0] wdata,     // Write data
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable
    input  logic        re,        // Read enable
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word)
    output logic        ready,     // Operation complete (always ready)
    
    // External UART pins
    output logic        tx_out,    // Serial transmit output (active high)
    input  logic        rx_in      // Serial receive input (active high)
);
```

### 5.2 Baud Rate Generator

```systemverilog
// Calculate clock divisor at compile time
localparam int CLKS_PER_BIT = CLK_FREQ_HZ / BAUD_RATE;

// Baud rate counter
logic [$clog2(CLKS_PER_BIT)-1:0] tx_baud_counter;
logic [$clog2(CLKS_PER_BIT)-1:0] rx_baud_counter;

// TX baud tick (one clock cycle high per bit period)
logic tx_baud_tick;
assign tx_baud_tick = (tx_baud_counter == 0);

// RX sampling (16x oversampling for start bit detection)
localparam int CLKS_PER_SAMPLE = CLKS_PER_BIT / 16;
```

### 5.3 TX FIFO and Shift Register

```systemverilog
// TX FIFO
logic [7:0] tx_fifo [0:FIFO_DEPTH-1];
logic [$clog2(FIFO_DEPTH)-1:0] tx_wr_ptr;
logic [$clog2(FIFO_DEPTH)-1:0] tx_rd_ptr;
logic [$clog2(FIFO_DEPTH):0]   tx_fifo_count;  // Extra bit for full detection

logic tx_fifo_full;
logic tx_fifo_empty;
assign tx_fifo_full  = (tx_fifo_count == FIFO_DEPTH);
assign tx_fifo_empty = (tx_fifo_count == 0);

// TX State Machine
typedef enum logic [2:0] {
    TX_IDLE,
    TX_START_BIT,
    TX_DATA_BITS,
    TX_STOP_BIT
} tx_state_t;

tx_state_t tx_state;
logic [7:0] tx_shift_reg;
logic [2:0] tx_bit_index;  // 0-7 for 8 data bits
logic tx_busy;
```

### 5.4 RX FIFO and Shift Register

```systemverilog
// RX FIFO
logic [7:0] rx_fifo [0:FIFO_DEPTH-1];
logic [$clog2(FIFO_DEPTH)-1:0] rx_wr_ptr;
logic [$clog2(FIFO_DEPTH)-1:0] rx_rd_ptr;
logic [$clog2(FIFO_DEPTH):0]   rx_fifo_count;

logic rx_fifo_full;
logic rx_fifo_empty;
assign rx_fifo_full  = (rx_fifo_count == FIFO_DEPTH);
assign rx_fifo_empty = (rx_fifo_count == 0);

// RX State Machine
typedef enum logic [2:0] {
    RX_IDLE,
    RX_START_BIT,
    RX_DATA_BITS,
    RX_STOP_BIT
} rx_state_t;

rx_state_t rx_state;
logic [7:0] rx_shift_reg;
logic [2:0] rx_bit_index;
logic [3:0] rx_sample_count;  // 0-15 for oversampling
logic rx_busy;
logic rx_error;
logic rx_fifo_write_int;  // Pulses high for one cycle when writing to RX FIFO
```

### 5.5 TX State Machine Logic

```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        tx_state <= TX_IDLE;
        tx_out <= 1'b1;  // Idle high
        tx_shift_reg <= 8'h00;
        tx_bit_index <= 3'b0;
        tx_baud_counter <= '0;
        tx_busy <= 1'b0;
    end else begin
        case (tx_state)
            TX_IDLE: begin
                tx_out <= 1'b1;
                tx_busy <= 1'b0;
                if (!tx_fifo_empty) begin
                    // Load byte from FIFO (FIFO management block handles rd_ptr)
                    tx_shift_reg <= tx_fifo[tx_rd_ptr];
                    tx_state <= TX_START_BIT;
                    tx_baud_counter <= CLKS_PER_BIT - 1;
                    tx_busy <= 1'b1;
                end
            end
            
            TX_START_BIT: begin
                tx_out <= 1'b0;  // Start bit is low
                if (tx_baud_tick) begin
                    tx_baud_counter <= CLKS_PER_BIT - 1;
                    tx_state <= TX_DATA_BITS;
                    tx_bit_index <= 3'b0;
                end else begin
                    tx_baud_counter <= tx_baud_counter - 1'b1;
                end
            end
            
            TX_DATA_BITS: begin
                tx_out <= tx_shift_reg[0];  // LSB first
                if (tx_baud_tick) begin
                    tx_shift_reg <= {1'b0, tx_shift_reg[7:1]};  // Shift right
                    if (tx_bit_index == 3'd7) begin
                        tx_state <= TX_STOP_BIT;
                    end else begin
                        tx_bit_index <= tx_bit_index + 1'b1;
                    end
                    tx_baud_counter <= CLKS_PER_BIT - 1;
                end else begin
                    tx_baud_counter <= tx_baud_counter - 1'b1;
                end
            end
            
            TX_STOP_BIT: begin
                tx_out <= 1'b1;  // Stop bit is high
                if (tx_baud_tick) begin
                    tx_state <= TX_IDLE;
                end else begin
                    tx_baud_counter <= tx_baud_counter - 1'b1;
                end
            end
            
            default: tx_state <= TX_IDLE;
        endcase
    end
end
```

### 5.6 RX State Machine Logic

```systemverilog
// Input synchronizer (2-FF for metastability)
logic rx_sync_0, rx_sync_1;
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_sync_0 <= 1'b1;
        rx_sync_1 <= 1'b1;
    end else begin
        rx_sync_0 <= rx_in;
        rx_sync_1 <= rx_sync_0;
    end
end

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_state <= RX_IDLE;
        rx_shift_reg <= 8'h00;
        rx_bit_index <= 3'b0;
        rx_sample_count <= 4'b0;
        rx_baud_counter <= '0;
        rx_wr_ptr <= '0;
        rx_busy <= 1'b0;
        rx_error <= 1'b0;
        rx_fifo_write_int <= 1'b0;
    end else begin
        case (rx_state)
            RX_IDLE: begin
                rx_busy <= 1'b0;
                rx_fifo_write_int <= 1'b0;  // Ensure pulse is cleared
                if (rx_sync_1 == 1'b0) begin  // Falling edge detected (start bit)
                    rx_state <= RX_START_BIT;
                    rx_sample_count <= 4'd0;
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    rx_busy <= 1'b1;
                end
            end
            
            RX_START_BIT: begin
                rx_fifo_write_int <= 1'b0;  // Ensure pulse is cleared
                if (rx_baud_counter == 0) begin
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    if (rx_sample_count == 4'd7) begin
                        // Sample at middle of start bit
                        if (rx_sync_1 == 1'b0) begin
                            // Valid start bit
                            rx_state <= RX_DATA_BITS;
                            rx_sample_count <= 4'd0;
                            rx_bit_index <= 3'd0;
                        end else begin
                            // False start - return to idle
                            rx_state <= RX_IDLE;
                        end
                    end else begin
                        rx_sample_count <= rx_sample_count + 1'b1;
                    end
                end else begin
                    rx_baud_counter <= rx_baud_counter - 1'b1;
                end
            end
            
            RX_DATA_BITS: begin
                rx_fifo_write_int <= 1'b0;  // Ensure pulse is cleared
                if (rx_baud_counter == 0) begin
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    if (rx_sample_count == 4'd7) begin
                        // Sample at middle of data bit (after 8 samples = half bit period)
                        // With 16x oversampling, we sample at count 7 (middle of bit)
                        // This provides maximum timing margin for robust data recovery
                        rx_shift_reg <= {rx_sync_1, rx_shift_reg[7:1]};  // LSB first
                        rx_bit_index <= rx_bit_index + 1'b1;
                    end
                    if (rx_sample_count == 4'd15) begin
                        // End of bit period - advance to next bit or stop
                        rx_sample_count <= 4'd0;
                        if (rx_bit_index == 3'd0) begin  // Just wrapped from 7 to 0
                            rx_state <= RX_STOP_BIT;
                        end
                    end else begin
                        rx_sample_count <= rx_sample_count + 1'b1;
                    end
                end else begin
                    rx_baud_counter <= rx_baud_counter - 1'b1;
                end
            end
            
            RX_STOP_BIT: begin
                rx_fifo_write_int <= 1'b0;  // Default: no write (ensures single-cycle pulse)
                if (rx_baud_counter == 0) begin
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    if (rx_sample_count == 4'd7) begin
                        // Sample stop bit at middle of bit (after 8 samples)
                        // This provides maximum timing margin for stop bit detection
                        if (rx_sync_1 == 1'b1) begin
                            // Valid stop bit - signal FIFO write
                            // (FIFO management block handles actual write)
                            rx_fifo_write_int <= 1'b1;
                        end else begin
                            // Framing error
                            rx_error <= 1'b1;
                        end
                        rx_state <= RX_IDLE;
                    end else begin
                        rx_sample_count <= rx_sample_count + 1'b1;
                    end
                end else begin
                    rx_baud_counter <= rx_baud_counter - 1'b1;
                end
            end
            
            default: begin
                rx_state <= RX_IDLE;
                rx_fifo_write_int <= 1'b0;
            end
        endcase
        
        // Handle rx_error clearing (from STATUS register read)
        if (clear_rx_error) begin
            rx_error <= 1'b0;
        end
    end
end
```

### 5.7 Register File Access Logic

```systemverilog
// UART is single-cycle - always ready
assign ready = 1'b1;

// Register offset decode (byte offset within 256B UART window)
logic [7:0] reg_offset;
assign reg_offset = addr[7:0];

// ============================================================
// TX FIFO Management
// ============================================================

// TX FIFO write signal (CPU writes to TXDATA register)
logic tx_fifo_write;
assign tx_fifo_write = we && (reg_offset == 8'h00) && !tx_fifo_full;

// TX FIFO read signal (TX state machine reads from FIFO)
// This is set when transitioning from TX_IDLE and loading byte
logic tx_fifo_read_int;
assign tx_fifo_read_int = (tx_state == TX_IDLE) && !tx_fifo_empty;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        tx_wr_ptr <= '0;
        tx_rd_ptr <= '0;
        tx_fifo_count <= '0;
    end else begin
        // Handle FIFO pointer and count updates
        case ({tx_fifo_write, tx_fifo_read_int})
            2'b10: begin  // Write only
                tx_wr_ptr <= tx_wr_ptr + 1'b1;
                tx_fifo_count <= tx_fifo_count + 1'b1;
            end
            2'b01: begin  // Read only (TX state machine read)
                tx_rd_ptr <= tx_rd_ptr + 1'b1;
                tx_fifo_count <= tx_fifo_count - 1'b1;
            end
            2'b11: begin  // Simultaneous read/write: count unchanged
                tx_wr_ptr <= tx_wr_ptr + 1'b1;
                tx_rd_ptr <= tx_rd_ptr + 1'b1;
            end
            default: ;  // 2'b00: no change
        endcase
        
        // Write to FIFO memory
        if (tx_fifo_write) begin
            tx_fifo[tx_wr_ptr] <= wdata[7:0];
        end
    end
end

// ============================================================
// RX FIFO Management
// ============================================================

// RX FIFO read signal (CPU reads from RXDATA register)
logic rx_fifo_read;
assign rx_fifo_read = re && (reg_offset == 8'h04) && !rx_fifo_empty;

// Note: rx_fifo_write_int is declared in the RX block definition section (5.4)
// and is driven by the RX state machine when transitioning from RX_STOP_BIT
// to RX_IDLE with a valid stop bit. It pulses high for one cycle.

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_rd_ptr <= '0;
        rx_wr_ptr <= '0;
        rx_fifo_count <= '0;
    end else begin
        // Handle FIFO pointer and count updates
        case ({rx_fifo_write_int, rx_fifo_read})
            2'b10: begin  // Write only (RX state machine)
                rx_wr_ptr <= rx_wr_ptr + 1'b1;
                rx_fifo_count <= rx_fifo_count + 1'b1;
            end
            2'b01: begin  // Read only (CPU read)
                rx_rd_ptr <= rx_rd_ptr + 1'b1;
                rx_fifo_count <= rx_fifo_count - 1'b1;
            end
            2'b11: begin  // Simultaneous read/write: count unchanged
                rx_wr_ptr <= rx_wr_ptr + 1'b1;
                rx_rd_ptr <= rx_rd_ptr + 1'b1;
            end
            default: ;  // 2'b00: no change
        endcase
        
        // Write to FIFO memory (performed by RX state machine)
        if (rx_fifo_write_int && !rx_fifo_full) begin
            rx_fifo[rx_wr_ptr] <= rx_shift_reg;
        end
    end
end

// ============================================================
// Status Register Computation
// ============================================================

// TX_EMPTY: FIFO empty AND transmitter idle
logic tx_empty_status;
assign tx_empty_status = tx_fifo_empty && (tx_state == TX_IDLE);

// Read data mux
always_comb begin
    rdata = 32'h0;
    
    if (re) begin
        case (reg_offset)
            8'h00: rdata = 32'h0;  // TXDATA is write-only
            8'h04: rdata = rx_fifo_empty ? 32'h0 : {24'h0, rx_fifo[rx_rd_ptr]};  // RXDATA
            8'h08: rdata = {24'h0,                      // STATUS
                          rx_error,
                          rx_busy,
                          rx_fifo_empty,
                          rx_fifo_full,
                          1'b0,
                          tx_busy,
                          tx_empty_status,             // TX_EMPTY (FIFO empty AND idle)
                          tx_fifo_full};
            8'h0C: rdata = 32'h0;  // CTRL (reserved)
            default: rdata = 32'h0;
        endcase
    end
end

// ============================================================
// RX Error Management
// ============================================================

// Note: rx_error is managed in the RX state machine (section 5.6)
// It is set when a framing error is detected (missing stop bit)
// It is cleared when the STATUS register is read

logic clear_rx_error;
assign clear_rx_error = re && (reg_offset == 8'h08);

// rx_error update logic is integrated into RX state machine:
// - Set when rx_sync_1 != 1'b1 at stop bit sample time
// - Cleared when STATUS register is read (clear_rx_error signal)
```

**Note on RX State Machine Integration:**

The RX state machine (section 5.6) should be modified to:
1. Set `rx_fifo_write_int` for one cycle when writing to FIFO
2. Handle `rx_error` clearing via `clear_rx_error` signal
3. Not manage `rx_wr_ptr` directly (done in FIFO management block above)

Here's the updated RX_STOP_BIT state logic:

```systemverilog
RX_STOP_BIT: begin
    if (rx_baud_counter == 0) begin
        rx_baud_counter <= CLKS_PER_SAMPLE - 1;
        if (rx_sample_count == 4'd15) begin
            // Sample stop bit
            if (rx_sync_1 == 1'b1) begin
                // Valid stop bit - signal FIFO write
                rx_fifo_write_int <= 1'b1;
            end else begin
                // Framing error - set error flag
                rx_error_set <= 1'b1;
            end
            rx_state <= RX_IDLE;
        end else begin
            rx_sample_count <= rx_sample_count + 1'b1;
        end
    end else begin
        rx_baud_counter <= rx_baud_counter - 1'b1;
    end
end
```


---

## 6. Top-Level Integration

### 6.1 Update top_with_peripherals.sv

**Add UART Parameters and Loopback Support:**

The `ENABLE_UART_LOOPBACK` parameter controls whether the UART TX output is internally connected to the RX input. This is enabled by default for simulation testing and disabled for FPGA deployment where external pins are used.

```systemverilog
module top_with_peripherals #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b1,
    // UART Parameters
    parameter int UART_CLK_FREQ_HZ = 50_000_000,
    parameter int UART_BAUD_RATE   = 115200,
    // UART Loopback: When enabled (default), TX is internally connected to RX
    // for simulation testing. Disable for FPGA deployment with external pins.
    parameter bit ENABLE_UART_LOOPBACK = 1'b1
) (
    // ... existing ports ...
    
    // LED peripheral outputs (existing)
    output logic [7:0]  led_out,
    
    // UART peripheral pins (active only when ENABLE_UART_LOOPBACK = 0)
    output logic        uart_tx,    // UART transmit output (directly connected to uart_tx_internal)
    input  logic        uart_rx     // UART receive input (directly connected to uart_rx_internal)
);
```

**Internal UART Signals and Loopback Logic:**

```systemverilog
// Internal UART signals
logic uart_tx_internal;  // TX output from UART module
logic uart_rx_internal;  // RX input to UART module

// Loopback or external connection
generate
    if (ENABLE_UART_LOOPBACK) begin : gen_loopback
        // Internal loopback: connect TX directly to RX for testing
        assign uart_rx_internal = uart_tx_internal;
        // Still expose TX externally for debugging/monitoring
        assign uart_tx = uart_tx_internal;
        // uart_rx input port is ignored in loopback mode
    end else begin : gen_external
        // External connection: use actual RX/TX pins
        assign uart_rx_internal = uart_rx;
        assign uart_tx = uart_tx_internal;
    end
endgenerate
```

**Add UART Instance and Address Decoding:**

```systemverilog
// UART Controller Interface Signals
logic [31:0] uart_rdata;
logic        uart_ready;

// Address Decoder Update
logic sel_led;
logic sel_uart;
logic sel_external;
logic sel_unmapped_rtl;

always_comb begin
    sel_led          = 1'b0;
    sel_uart         = 1'b0;
    sel_unmapped_rtl = 1'b0;
    sel_external     = 1'b0;
    
    if (cpu_dmem_addr >= LED_BASE && cpu_dmem_addr < LED_LIMIT) begin
        sel_led = 1'b1;
    end
    else if (cpu_dmem_addr >= UART_BASE && cpu_dmem_addr < UART_LIMIT) begin
        sel_uart = 1'b1;
    end
    else if (cpu_dmem_addr >= RTL_PERIPH_BASE && cpu_dmem_addr < RTL_PERIPH_LIMIT) begin
        sel_unmapped_rtl = 1'b1;
    end
    else begin
        sel_external = 1'b1;
    end
end

// Response Multiplexer Update
always_comb begin
    cpu_dmem_rdata = 32'h0;
    cpu_dmem_ready = 1'b0;
    
    if (sel_led) begin
        cpu_dmem_rdata = led_rdata;
        cpu_dmem_ready = led_ready;
    end else if (sel_uart) begin
        cpu_dmem_rdata = uart_rdata;
        cpu_dmem_ready = uart_ready;
    end else if (sel_unmapped_rtl) begin
        cpu_dmem_rdata = 32'h0;
        cpu_dmem_ready = 1'b1;
    end else if (sel_external) begin
        cpu_dmem_rdata = ext_mem_rdata;
        cpu_dmem_ready = ext_mem_ready;
    end
end

// UART Controller Instantiation
uart #(
    .CLK_FREQ_HZ(UART_CLK_FREQ_HZ),
    .BAUD_RATE(UART_BAUD_RATE),
    .FIFO_DEPTH(8)
) uart_ctrl (
    .clk(clk),
    .rst_n(rst_n),
    
    // CPU interface
    .addr(cpu_dmem_addr),
    .wdata(cpu_dmem_wdata),
    .rdata(uart_rdata),
    .we(cpu_dmem_we && sel_uart),
    .re(cpu_dmem_re && sel_uart),
    .size(cpu_dmem_size),
    .ready(uart_ready),
    
    // Internal signals (connected via loopback or external pins)
    .tx_out(uart_tx_internal),
    .rx_in(uart_rx_internal)
);
```

**FPGA Top Module Example (with loopback disabled):**

When deploying to FPGA, instantiate with `ENABLE_UART_LOOPBACK = 0`:

```systemverilog
module fpga_top (
    input  logic       clk_50mhz,
    input  logic       rst_btn_n,
    
    // LED outputs
    output logic [7:0] led,
    
    // UART external pins (directly connected to FPGA I/O)
    output logic       uart_tx_pin,
    input  logic       uart_rx_pin
);
    top_with_peripherals #(
        .ENABLE_M_EXT(1'b1),
        .ENABLE_F_EXT(1'b1),
        .UART_CLK_FREQ_HZ(50_000_000),
        .UART_BAUD_RATE(115200),
        .ENABLE_UART_LOOPBACK(1'b0)  // Disable loopback for real hardware
    ) cpu_system (
        .clk(clk_50mhz),
        .rst_n(rst_btn_n),
        .boot_addr(32'h80000000),
        // ... memory interfaces ...
        .led_out(led),
        .uart_tx(uart_tx_pin),
        .uart_rx(uart_rx_pin)
    );
endmodule
```

### 6.2 Update riscv_core/src/lib.rs

Add UART-specific file to CPU runtime:

```rust
// Helper function to create a runtime for the full CPU
pub fn create_cpu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "top_with_peripherals.sv",       // Top-level wrapper with RTL peripherals
        "top.sv",                        // CPU core
        "peripherals/led_controller.sv", // LED controller peripheral
        "peripherals/uart.sv",           // UART controller peripheral (NEW)
        "fetch_buffer.sv",               // RV32C fetch buffer
        // ... rest unchanged ...
    ])
}
```

### 6.3 Update fpga/fpga_top.sv

Update the FPGA top module to connect UART external pins and disable loopback:

```systemverilog
module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b0,
    parameter bit ENABLE_F_EXT = 1'b0
) (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,
    
    // Reset button (active low)
    input  logic       rst_n_btn,
    
    // LED outputs
    output logic [7:0] led,
    
    // UART external pins (NEW)
    output logic       uart_tx,
    input  logic       uart_rx
);
    // ... existing code ...
    
    // ============================================================
    // CPU Core with Peripherals
    // ============================================================
    top_with_peripherals #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .ENABLE_UART_LOOPBACK(1'b0)  // Disable loopback for FPGA - use external pins
    ) cpu (
        .clk(sys_clk),
        .rst_n(rst_n),
        .boot_addr(BOOT_ADDR),
        
        // ... existing memory and debug connections ...
        
        // LED peripheral
        .led_out(led_out),
        
        // UART peripheral (NEW)
        .uart_tx(uart_tx),
        .uart_rx(uart_rx),
        
        // System control
        .halted(halted),
        .instr_complete(instr_complete),
        
        // ... debug outputs ...
    );
    
    // ... rest of module ...
endmodule
```

### 6.4 Update fpga/ice40hx8k.pcf

Add UART pin constraints (pin numbers are examples - update for actual board):

```
# UART Pins (update pin numbers for specific board)
set_io uart_tx P12    # UART transmit output
set_io uart_rx P13    # UART receive input
```

---

## 7. Rust Integration Layer

### 7.1 Update riscv_shared/src/bus.rs

Add UART constants:

```rust
// UART Controller Peripheral (RTL)
pub const UART_BASE: u32 = 0x5200_0000;
pub const UART_SIZE: u32 = 0x0000_0100;  // 256 bytes

// UART register offsets
pub const UART_TXDATA_OFFSET: u32 = 0x00;
pub const UART_RXDATA_OFFSET: u32 = 0x04;
pub const UART_STATUS_OFFSET: u32 = 0x08;
pub const UART_CTRL_OFFSET: u32 = 0x0C;

// UART status register bit masks
pub const UART_STATUS_TX_FULL: u32  = 1 << 0;
pub const UART_STATUS_TX_EMPTY: u32 = 1 << 1;
pub const UART_STATUS_TX_BUSY: u32  = 1 << 2;
pub const UART_STATUS_RX_FULL: u32  = 1 << 4;
pub const UART_STATUS_RX_EMPTY: u32 = 1 << 5;
pub const UART_STATUS_RX_BUSY: u32  = 1 << 6;
pub const UART_STATUS_RX_ERROR: u32 = 1 << 7;

/// Helper function to get UART TXDATA register address
pub const fn uart_txdata_addr() -> u32 {
    UART_BASE + UART_TXDATA_OFFSET
}

/// Helper function to get UART RXDATA register address
pub const fn uart_rxdata_addr() -> u32 {
    UART_BASE + UART_RXDATA_OFFSET
}

/// Helper function to get UART STATUS register address
pub const fn uart_status_addr() -> u32 {
    UART_BASE + UART_STATUS_OFFSET
}
```

### 7.2 Rust Integration Notes

**No UART Signal Accessors Required:**

With the `ENABLE_UART_LOOPBACK` parameter enabled by default, there is no need to expose the raw UART TX/RX signals to the Rust simulation code. The hardware loopback handles the connection internally, which means:

- Tests can write data to the TX FIFO and read it back from the RX FIFO
- No complex signal injection or capture logic is needed in Rust
- Test programs validate data integrity entirely within the CPU program

This significantly simplifies the Rust integration layer - only the bus constants are needed.

---

## 8. Testing Strategy

The UART testing strategy consists of two levels:

1. **RTL-Focused Tests** (`testbench/tests/uart_test.rs`) - Lower-level tests that directly verify the UART Verilog module in isolation, without the full CPU
2. **CPU-Level Tests** (`cpu-sim/tests/test_uart.rs`) - Higher-level integration tests that run RISC-V programs through the CPU simulator with hardware loopback

### 8.1 RTL-Focused Tests (Module-Level)

RTL-focused tests directly instantiate the UART Verilog module via Verilator and test its behavior at the signal level. These tests are located in `testbench/tests/uart_test.rs` and follow the same pattern as existing RTL tests (e.g., `alu_test.rs`, `regfile_test.rs`).

**Purpose:**
- Verify UART module behavior in isolation
- Test edge cases and timing requirements
- Debug issues at the module level without full CPU complexity
- Fast iteration during UART RTL development

**Test File: testbench/tests/uart_test.rs**

```rust
use riscv_core::{create_uart_runtime, Uart};

// Clock cycle macro for UART tests
macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

// Helper to wait for specified number of clock cycles
fn wait_cycles(dut: &mut Uart, cycles: u32) {
    for _ in 0..cycles {
        clock_cycle!(dut);
    }
}

// ============================================================================
// UART RTL Module Tests
// ============================================================================

#[test]
fn test_uart_reset_state() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Apply reset
    dut.rst_n = 0;
    dut.we = 0;
    dut.re = 0;
    clock_cycle!(dut);
    
    // Release reset
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    // Verify TX line is idle high
    assert_eq!(dut.tx_out, 1, "TX should be idle high after reset");
    
    // Verify ready signal is asserted (single-cycle peripheral)
    assert_eq!(dut.ready, 1, "UART should be ready");
}

#[test]
fn test_uart_tx_idle_high() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset and initialize
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    
    // Verify TX stays high for multiple cycles without any writes
    for _ in 0..100 {
        clock_cycle!(dut);
        assert_eq!(dut.tx_out, 1, "TX should remain idle high");
    }
}

#[test]
fn test_uart_status_register_initial() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    // Read STATUS register (offset 0x08)
    dut.addr = 0x08;
    dut.re = 1;
    dut.eval();
    
    let status = dut.rdata;
    
    // Check TX_EMPTY (bit 1) is set
    assert!((status & 0x02) != 0, "TX_EMPTY should be set initially");
    
    // Check RX_EMPTY (bit 5) is set
    assert!((status & 0x20) != 0, "RX_EMPTY should be set initially");
    
    // Check TX_FULL (bit 0) is clear
    assert!((status & 0x01) == 0, "TX_FULL should be clear initially");
}

#[test]
fn test_uart_tx_fifo_write() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    // Write a byte to TXDATA (offset 0x00)
    dut.addr = 0x00;
    dut.wdata = 0x55;
    dut.we = 1;
    dut.size = 0b10;  // Word access
    clock_cycle!(dut);
    dut.we = 0;
    
    // Read STATUS register
    dut.addr = 0x08;
    dut.re = 1;
    dut.eval();
    
    // TX_EMPTY should now be clear (data in FIFO)
    // Note: Depending on implementation, TX_EMPTY may clear after TX starts
}

#[test]
fn test_uart_tx_start_bit() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    // Write a byte to trigger transmission
    dut.addr = 0x00;
    dut.wdata = 0xAA;
    dut.we = 1;
    dut.size = 0b10;
    clock_cycle!(dut);
    dut.we = 0;
    
    // TX should eventually go low for start bit
    let mut saw_start_bit = false;
    for _ in 0..1000 {
        clock_cycle!(dut);
        if dut.tx_out == 0 {
            saw_start_bit = true;
            break;
        }
    }
    
    assert!(saw_start_bit, "TX should transition to low for start bit");
}

#[test]
fn test_uart_loopback_single_byte() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    let test_byte: u32 = 0xA5;
    
    // Write byte to TXDATA
    dut.addr = 0x00;
    dut.wdata = test_byte;
    dut.we = 1;
    dut.size = 0b10;
    clock_cycle!(dut);
    dut.we = 0;
    
    // Connect TX to RX (simulate loopback)
    // In the actual module, this is done via ENABLE_UART_LOOPBACK parameter
    // For direct module testing, we manually connect the signals
    
    // Wait for transmission to complete (depends on baud rate)
    // At 115200 baud with 50MHz clock, one bit = ~434 clocks
    // Full frame = 10 bits = ~4340 clocks
    for _ in 0..5000 {
        // Loopback: connect tx_out to rx_in
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);
    }
    
    // Check RX_EMPTY in STATUS register
    dut.addr = 0x08;
    dut.re = 1;
    dut.eval();
    
    let status = dut.rdata;
    let rx_empty = (status & 0x20) != 0;
    
    if !rx_empty {
        // Read RXDATA
        dut.re = 0;
        dut.addr = 0x04;
        dut.re = 1;
        dut.eval();
        
        let received = dut.rdata & 0xFF;
        assert_eq!(received, test_byte, "Received byte should match transmitted byte");
    }
}

#[test]
fn test_uart_tx_fifo_full() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");
    
    // Reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
    
    // Write 8 bytes to fill the FIFO
    for i in 0..8 {
        dut.addr = 0x00;
        dut.wdata = i as u32;
        dut.we = 1;
        dut.size = 0b10;
        clock_cycle!(dut);
    }
    dut.we = 0;
    
    // Check STATUS for TX_FULL
    dut.addr = 0x08;
    dut.re = 1;
    dut.eval();
    
    let status = dut.rdata;
    let tx_full = (status & 0x01) != 0;
    
    assert!(tx_full, "TX_FULL should be set after writing 8 bytes");
}
```

**RTL Test Categories:**

| Test Name | Description | Verifies |
|-----------|-------------|----------|
| `test_uart_reset_state` | Check state after reset | TX idle high, ready asserted |
| `test_uart_tx_idle_high` | TX line without activity | TX stays high |
| `test_uart_status_register_initial` | STATUS after reset | TX_EMPTY=1, RX_EMPTY=1 |
| `test_uart_tx_fifo_write` | Write to TXDATA register | Byte accepted into FIFO |
| `test_uart_tx_start_bit` | Trigger TX transmission | Start bit (low) appears |
| `test_uart_loopback_single_byte` | TX→RX via loopback | Data integrity |
| `test_uart_tx_fifo_full` | Fill TX FIFO | TX_FULL status set |
| `test_uart_baud_timing` | Bit timing accuracy | Correct clocks per bit |
| `test_uart_rx_oversampling` | RX bit sampling | Sample at mid-bit |

### 8.2 Hardware Loopback Approach (CPU-Level)

**Key Design Decision:** CPU-level UART testing is performed using the hardware loopback feature (`ENABLE_UART_LOOPBACK = 1`). This eliminates the need for:
- Rust-side UART signal monitoring in the CPU simulator
- Complex TX capture and RX injection helpers
- Cycle-accurate timing simulation in Rust

**Testing Flow:**
1. CPU test program writes bytes to TX FIFO (TXDATA register)
2. UART hardware transmits the data serially
3. With loopback enabled, TX connects directly to RX
4. RX hardware receives the data and stores in RX FIFO
5. CPU test program reads bytes from RX FIFO (RXDATA register)
6. Program validates received data matches transmitted data
7. Program writes success/failure to tohost register

### 8.3 CPU-Level Test Categories

**1. Register Access Tests (CPU-level):**
- Verify memory-mapped register addresses
- Test STATUS register initial state (TX_EMPTY, RX_EMPTY)
- Test TXDATA write behavior
- Test TX_FULL status when FIFO fills

**2. Loopback Data Integrity Tests (CPU-level):**
All loopback tests run with hardware loopback enabled (default). The test program:
- Writes test data to TX FIFO
- Waits for transmission/reception to complete
- Reads data from RX FIFO
- Validates received data matches transmitted data
- Reports pass/fail via tohost register

**Test Patterns:**
- Single byte: 0x55, 0xAA, 0x00, 0xFF
- Multi-byte sequence: 0x01, 0x02, 0x03, ...
- Full FIFO: 8 consecutive bytes

### 8.4 Test File: cpu-sim/tests/test_uart.rs

```rust
//! UART Controller RTL Peripheral Tests
//!
//! Tests for the UART controller peripheral (RTL-based).
//! Address: 0x52000000
//! Features: TX/RX FIFOs, hardware loopback for testing
//!
//! Note: All tests assume ENABLE_UART_LOOPBACK = 1 (default),
//! which internally connects TX to RX for data integrity testing.

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::bus::{
    UART_BASE, UART_TXDATA_OFFSET, UART_RXDATA_OFFSET, UART_STATUS_OFFSET,
    UART_STATUS_TX_EMPTY, UART_STATUS_TX_FULL, UART_STATUS_RX_EMPTY
};

/// Helper function to initialize test logger
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Generate tohost termination sequence
fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
    vec![
        lui(addr_reg, 0x10000000),
        addi(value_reg, 0, 1),
        sw(addr_reg, value_reg, 0),
        jal(0, 0),
    ]
}

// ============================================================================
// UART Constant Tests
// ============================================================================

#[test]
fn test_uart_constants() {
    assert_eq!(UART_BASE, 0x52000000, "UART base address");
    assert_eq!(UART_TXDATA_OFFSET, 0x00, "TXDATA register offset");
    assert_eq!(UART_RXDATA_OFFSET, 0x04, "RXDATA register offset");
    assert_eq!(UART_STATUS_OFFSET, 0x08, "STATUS register offset");
}

// ============================================================================
// TX FIFO Tests
// ============================================================================

#[test]
fn test_uart_tx_write_byte() {
    init_test_logger();

    // Write a byte to TX FIFO
    let mut instructions = vec![
        lui(15, UART_BASE),           // Load UART base address (full 32-bit, low 12 bits zero)
        addi(14, 0, 0x55),            // Load byte to transmit
        sw(15, 14, UART_TXDATA_OFFSET as i32),  // Write to TXDATA
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );
}

#[test]
fn test_uart_status_initial_state() {
    init_test_logger();

    // Read STATUS register - should show TX_EMPTY and RX_EMPTY
    let mut instructions = vec![
        lui(15, UART_BASE),           // Load UART base address (full 32-bit, low 12 bits zero)
        lw(14, 15, UART_STATUS_OFFSET as i32),  // Read STATUS
        // Store STATUS value to memory for verification
        lui(13, 0x80000000),          // Load DRAM base
        sw(13, 14, 0x100),            // Store STATUS to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let status_value = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let status_clone = status_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            // Read STATUS value from memory where program stored it
            if let Some(val) = sim.read_word(0x80000100) {
                *status_clone.lock().unwrap() = val;
            }
        }),
    )
    .expect("Simulation should succeed");

    let status = *status_value.lock().unwrap();
    assert!(
        (status & UART_STATUS_TX_EMPTY) != 0,
        "TX_EMPTY should be set initially"
    );
    assert!(
        (status & UART_STATUS_RX_EMPTY) != 0,
        "RX_EMPTY should be set initially"
    );
    assert!(
        (status & UART_STATUS_TX_FULL) == 0,
        "TX_FULL should be clear initially"
    );
}

// ============================================================================
// UART Loopback Test (Hardware Loopback)
// ============================================================================

#[test]
fn test_uart_loopback_single_byte() {
    init_test_logger();
    
    // Test program that sends a byte via TX and receives it via RX (hardware loopback)
    // Uses ENABLE_UART_LOOPBACK = 1 (default) so TX is connected to RX internally
    //
    // Algorithm:
    // 1. Write test byte (0xA5) to TXDATA
    // 2. Poll STATUS until TX_EMPTY (transmission complete)
    // 3. Poll STATUS until !RX_EMPTY (data received)
    // 4. Read RXDATA and compare with sent byte
    // 5. Write success (1) or failure (0) to tohost
    
    let mut instructions = vec![
        // x15 = UART base address
        lui(15, UART_BASE),           // Load UART base address (full 32-bit, low 12 bits zero)
        
        // x14 = test byte (0xA5)
        addi(14, 0, 0xA5),
        
        // Write test byte to TXDATA (offset 0)
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        
        // Poll for TX_EMPTY: wait until all data transmitted
        // Loop: read STATUS, check TX_EMPTY bit, branch if not set
        lw(13, 15, UART_STATUS_OFFSET as i32),  // x13 = STATUS
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),  // x12 = TX_EMPTY bit
        beq(12, 0, -8),  // If TX_EMPTY == 0, loop back to lw
        
        // Poll for !RX_EMPTY: wait until data received
        lw(13, 15, UART_STATUS_OFFSET as i32),  // x13 = STATUS
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),  // x12 = RX_EMPTY bit
        bne(12, 0, -8),  // If RX_EMPTY != 0, loop back to lw
        
        // Read received byte from RXDATA (offset 4)
        lw(11, 15, UART_RXDATA_OFFSET as i32),  // x11 = received byte
        
        // Compare with sent byte (x14 = 0xA5)
        // If equal, write 1 to tohost; else write 0
        lui(10, 0x10000000),          // x10 = tohost address (full 32-bit, low 12 bits zero)
        beq(11, 14, 8),  // If received == sent, skip to success
        sw(10, 0, 0),    // Write 0 (failure) to tohost
        jal(0, 8),       // Jump over success (skip 2 instructions = 8 bytes)
        addi(9, 0, 1),   // x9 = 1 (success)
        sw(10, 9, 0),    // Write 1 (success) to tohost
        jal(0, 0),       // Infinite loop
    ];
    
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES * 10,  // Allow more cycles for UART timing
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Loopback test: received byte should match sent byte"
    );
}
```

### 8.5 Test Scenarios

**RTL-Focused Tests (testbench/tests/uart_test.rs):**

| Test Name | Description | Expected Result |
|-----------|-------------|-----------------|
| `test_uart_reset_state` | Check state after reset | TX idle high, ready asserted |
| `test_uart_tx_idle_high` | TX line without activity | TX stays high |
| `test_uart_status_register_initial` | STATUS after reset | TX_EMPTY=1, RX_EMPTY=1 |
| `test_uart_tx_fifo_write` | Write to TXDATA register | Byte accepted into FIFO |
| `test_uart_tx_start_bit` | Trigger TX transmission | Start bit (low) appears |
| `test_uart_loopback_single_byte` | TX→RX via manual loopback | Data integrity |
| `test_uart_tx_fifo_full` | Fill TX FIFO | TX_FULL status set |
| `test_uart_baud_timing` | Bit timing accuracy | Correct clocks per bit |

**CPU-Level Tests (cpu-sim/tests/test_uart.rs):**

| Test Name | Description | Expected Result |
|-----------|-------------|-----------------|
| `test_uart_constants` | Verify UART memory map constants | All addresses correct |
| `test_uart_tx_write_byte` | Write single byte to TX FIFO | No error, byte queued |
| `test_uart_status_initial` | Check initial STATUS register | TX_EMPTY=1, RX_EMPTY=1 |
| `test_uart_tx_fifo_full` | Write 9+ bytes to TX FIFO | TX_FULL set after 8 writes |
| `test_uart_rx_read_empty` | Read RXDATA when empty | Returns 0, no crash |
| `test_uart_loopback_single_byte` | Send/receive 0xA5 via hardware loopback | Received == Transmitted |
| `test_uart_loopback_pattern` | Send/receive 0x00, 0xFF, 0xAA, 0x55 via loopback | All patterns match |
| `test_uart_loopback_multi_byte` | Send/receive 8 bytes via loopback | All bytes match in order |

**Note:** Framing error tests are not included in the initial implementation since hardware loopback produces valid frames. Framing error testing would require external test equipment or a modified UART that can inject invalid frames.

---

## 9. Implementation Checklist

### Phase 1: RTL Implementation

- [ ] Create `rtl/peripherals/uart.sv`
  - [ ] Module header with parameters (CLK_FREQ_HZ, BAUD_RATE, FIFO_DEPTH)
  - [ ] Baud rate generator
  - [ ] TX FIFO implementation
  - [ ] TX state machine (IDLE, START, DATA, STOP)
  - [ ] RX input synchronizer (2-FF)
  - [ ] RX FIFO implementation
  - [ ] RX state machine with oversampling
  - [ ] Register file (TXDATA, RXDATA, STATUS, CTRL)
  - [ ] Compile-time parameter validation
  
- [ ] Update `rtl/top_with_peripherals.sv`
  - [ ] Add ENABLE_UART_LOOPBACK parameter (default = 1)
  - [ ] Add UART_CLK_FREQ_HZ and UART_BAUD_RATE parameters
  - [ ] Add uart_tx/uart_rx port signals
  - [ ] Add internal uart_tx_internal/uart_rx_internal signals
  - [ ] Add generate block for loopback vs external connection
  - [ ] Add UART address decoder constants
  - [ ] Update address decode logic
  - [ ] Update response mux logic
  - [ ] Instantiate UART module with internal signals

- [ ] Lint RTL with Verilator
  - [ ] `verilator --lint-only rtl/peripherals/uart.sv`
  - [ ] `verilator --lint-only rtl/top_with_peripherals.sv`

### Phase 2: Rust Integration

- [ ] Update `riscv_shared/src/bus.rs`
  - [ ] Add UART_BASE, UART_SIZE constants
  - [ ] Add UART register offset constants
  - [ ] Add UART status bit masks
  - [ ] Add helper functions

- [ ] Update `riscv_core/src/lib.rs`
  - [ ] Add `Uart` struct with `#[verilog]` attribute
  - [ ] Add `create_uart_runtime()` helper function
  - [ ] Add uart.sv to create_cpu_runtime()

- [ ] Clear Verilator cache
  - [ ] `cargo clean`

### Phase 3: RTL-Focused Testing

- [ ] Create `testbench/tests/uart_test.rs`
  - [ ] test_uart_reset_state()
  - [ ] test_uart_tx_idle_high()
  - [ ] test_uart_status_register_initial()
  - [ ] test_uart_tx_fifo_write()
  - [ ] test_uart_tx_start_bit()
  - [ ] test_uart_loopback_single_byte() (manual signal loopback)
  - [ ] test_uart_tx_fifo_full()
  - [ ] test_uart_baud_timing()

- [ ] Run RTL tests
  - [ ] `cargo test -p testbench --verbose`

### Phase 4: CPU-Level Testing

- [ ] Create `cpu-sim/tests/test_uart.rs`
  - [ ] test_uart_constants()
  - [ ] test_uart_tx_write_byte()
  - [ ] test_uart_status_initial_state()
  - [ ] test_uart_tx_fifo_full()
  - [ ] test_uart_rx_read_empty()
  - [ ] test_uart_loopback_single_byte() (hardware loopback)
  - [ ] test_uart_loopback_pattern()
  - [ ] test_uart_loopback_multi_byte()

- [ ] Run all tests
  - [ ] `cargo test --verbose`

### Phase 5: FPGA Integration

**Note:** The `fpga/fpga_top.sv` module already includes `uart_tx` and `uart_rx` external pins, and the `fpga/ice40hx8k.pcf` pin constraint file already has the UART pin assignments. The only required change is updating the `top_with_peripherals` instantiation.

- [ ] Update `fpga/fpga_top.sv`
  - [ ] Update `top_with_peripherals` instantiation with `ENABLE_UART_LOOPBACK = 0`

- [ ] Verify FPGA synthesis still works
  - [ ] `cd fpga && make`
  - [ ] Check for timing violations or resource overflow
  - [ ] Review synthesis report for UART resource usage

### Phase 6: Documentation

- [ ] Update `AGENTS.md`
  - [ ] Add UART to memory map table
  - [ ] Add UART usage example

- [ ] Update `riscv_shared/src/bus.rs` documentation
  - [ ] Document UART register map

---

## 10. Future Extensions

### 10.1 Version 2 Enhancements

**Parity Support:**
- Add CTRL register bits for parity enable and mode (even/odd)
- Modify TX/RX state machines to include parity bit
- Add STATUS bit for parity error

**Configurable Stop Bits:**
- Add CTRL register bit for 1 vs 2 stop bits
- Modify TX state machine for second stop bit

**Interrupt Support:**
- Add interrupt output signal
- Add CTRL register interrupt enable bits (TX empty, RX full, error)
- Add interrupt status register

### 10.2 Version 3 Enhancements

**Hardware Flow Control (RTS/CTS):**
- Add rts_n output and cts_n input signals
- Automatic TX pause when CTS deasserted
- RTS control based on RX FIFO level

**Runtime Baud Rate Configuration:**
- Add divisor register for software-programmable baud rate
- Maintain CLK_FREQ_HZ as compile-time parameter

### 10.3 FPGA Deployment Notes

**Pin Assignment:**
```
Signal    | Direction | FPGA Pin | Voltage
----------|-----------|----------|--------
uart_tx   | Output    | TBD      | 3.3V LVCMOS
uart_rx   | Input     | TBD      | 3.3V LVCMOS
```

**External Connections:**
- USB-UART bridge (e.g., FTDI FT232R, CP2102, CH340)
- Direct connection to another UART device
- Level shifter if connecting to RS-232 levels

---

## Appendix A: UART Protocol Reference

### 8N1 Frame Format

```
          ┌─────┬───┬───┬───┬───┬───┬───┬───┬───┬──────┐
          │Start│ D0│ D1│ D2│ D3│ D4│ D5│ D6│ D7│ Stop │
          │ Bit │   │   │   │   │   │   │   │   │ Bit  │
          └─────┴───┴───┴───┴───┴───┴───┴───┴───┴──────┘
               └────────── 8 Data Bits ──────────┘
               
Idle = High (1)
Start Bit = Low (0)  
Data Bits = LSB first
Stop Bit = High (1)

Total frame: 10 bit periods per byte
```

### Timing Example at 115200 baud

```
Bit Period = 1/115200 = 8.68 µs
Full Frame = 10 × 8.68 µs = 86.8 µs
Bytes per Second = 115200 / 10 = 11,520 bytes/sec
```

---

## Appendix B: Related Files

| File | Purpose |
|------|---------|
| `rtl/peripherals/led_controller.sv` | Reference RTL peripheral implementation |
| `rtl/top_with_peripherals.sv` | Peripheral integration wrapper |
| `riscv_shared/src/bus.rs` | Memory map constants |
| `cpu-sim/tests/test_led.rs` | Reference test implementation |
| `docs/plans/rtl-peripheral-implementation-plan.md` | Overall peripheral architecture |

---

**End of Document**

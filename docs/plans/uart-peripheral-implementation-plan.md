# UART Peripheral Implementation Plan
## RTL-Based UART with FIFO Interface

**Author:** GitHub Copilot Hardware-Software Integration Architect  
**Date:** January 28, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Based on:** `rtl/peripherals/led_controller.sv` reference implementation  
**Status:** Technical Implementation Plan - Ready for Review

---

## Executive Summary

This document provides a detailed technical implementation plan for adding an RTL-based UART (Universal Asynchronous Receiver/Transmitter) peripheral to the RISC-V CPU. The UART will:

1. **Support configurable baud rates** via module parameters
2. **Allow system clock frequency configuration** for clock-domain independence
3. **Use 8N1 format** (8 data bits, no parity, 1 stop bit)
4. **Provide FIFO-based TX/RX interfaces** for easy CPU interaction
5. **Expose status registers** for flow control (FIFO full/empty)
6. **Connect directly to external RX/TX pins** for FPGA deployment

**Key Architecture Decision:** Run UART logic on system clock with oversampling. This simplifies the design by avoiding separate clock domains while maintaining robust data recovery.

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
    end else begin
        case (rx_state)
            RX_IDLE: begin
                rx_busy <= 1'b0;
                if (rx_sync_1 == 1'b0) begin  // Falling edge detected (start bit)
                    rx_state <= RX_START_BIT;
                    rx_sample_count <= 4'd0;
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    rx_busy <= 1'b1;
                end
            end
            
            RX_START_BIT: begin
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
                if (rx_baud_counter == 0) begin
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    if (rx_sample_count == 4'd15) begin
                        // Sample at middle of data bit (16 samples per bit)
                        rx_shift_reg <= {rx_sync_1, rx_shift_reg[7:1]};  // LSB first
                        rx_sample_count <= 4'd0;
                        if (rx_bit_index == 3'd7) begin
                            rx_state <= RX_STOP_BIT;
                        end else begin
                            rx_bit_index <= rx_bit_index + 1'b1;
                        end
                    end else begin
                        rx_sample_count <= rx_sample_count + 1'b1;
                    end
                end else begin
                    rx_baud_counter <= rx_baud_counter - 1'b1;
                end
            end
            
            RX_STOP_BIT: begin
                rx_fifo_write_int <= 1'b0;  // Default: no write
                if (rx_baud_counter == 0) begin
                    rx_baud_counter <= CLKS_PER_SAMPLE - 1;
                    if (rx_sample_count == 4'd15) begin
                        // Sample stop bit
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

// Register offset decode (lower bits of address)
logic [3:0] reg_offset;
assign reg_offset = addr[3:0];

// ============================================================
// TX FIFO Management
// ============================================================

// TX FIFO write signal (CPU writes to TXDATA register)
logic tx_fifo_write;
assign tx_fifo_write = we && (reg_offset == 4'h0) && !tx_fifo_full;

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
assign rx_fifo_read = re && (reg_offset == 4'h4) && !rx_fifo_empty;

// RX FIFO write signal (RX state machine writes received byte)
// This is set in RX_STOP_BIT state when a valid byte is received
logic rx_fifo_write_int;
// Note: This signal is set by the RX state machine when transitioning
// from RX_STOP_BIT to RX_IDLE with a valid stop bit

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
            4'h0: rdata = 32'h0;  // TXDATA is write-only
            4'h4: rdata = rx_fifo_empty ? 32'h0 : {24'h0, rx_fifo[rx_rd_ptr]};  // RXDATA
            4'h8: rdata = {24'h0,                      // STATUS
                          rx_error,
                          rx_busy,
                          rx_fifo_empty,
                          rx_fifo_full,
                          1'b0,
                          tx_busy,
                          tx_empty_status,             // TX_EMPTY (FIFO empty AND idle)
                          tx_fifo_full};
            4'hC: rdata = 32'h0;  // CTRL (reserved)
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
assign clear_rx_error = re && (reg_offset == 4'h8);

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

**Add UART External Pins:**

```systemverilog
module top_with_peripherals #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b1,
    // UART Parameters
    parameter int UART_CLK_FREQ_HZ = 50_000_000,
    parameter int UART_BAUD_RATE   = 115200
) (
    // ... existing ports ...
    
    // LED peripheral outputs (existing)
    output logic [7:0]  led_out,
    
    // UART peripheral pins (NEW)
    output logic        uart_tx,    // UART transmit output
    input  logic        uart_rx     // UART receive input
);
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
    
    // External pins
    .tx_out(uart_tx),
    .rx_in(uart_rx)
);
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

### 7.2 Update cpu-sim/src/sim.rs

Add UART signal accessors in `SimulatorView`:

```rust
impl<'a> SimulatorView<'a> {
    // ... existing methods ...
    
    /// Read the current UART TX output signal
    ///
    /// Returns the raw tx_out signal from the UART module.
    /// This is useful for verifying UART transmission in tests.
    pub fn uart_tx(&self) -> bool {
        self.cpu.uart_tx != 0
    }
    
    /// Set the UART RX input signal
    ///
    /// This allows tests to inject serial data into the UART receiver.
    /// Call this to simulate external UART transmission.
    pub fn set_uart_rx(&mut self, value: bool) {
        // Note: This requires the cpu to be mutable
        // May need to adjust the architecture for this
    }
}
```

---

## 8. Testing Strategy

### 8.1 Test Categories

**1. Unit Tests (RTL-focused):**
- Baud rate generator timing accuracy
- TX FIFO write/read behavior
- RX FIFO write/read behavior
- Status register correctness

**2. Integration Tests (CPU + UART):**
- Memory-mapped register access
- TX data transmission via CPU writes
- RX data reception via CPU reads
- FIFO overflow/underflow handling

**3. Loopback Tests:**
- Connect TX output to RX input (external loopback)
- Verify data integrity for various patterns
- Test at different simulated baud rates

### 8.2 Test File: cpu-sim/tests/test_uart.rs

```rust
//! UART Controller RTL Peripheral Tests
//!
//! Tests for the UART controller peripheral (RTL-based).
//! Address: 0x52000000
//! Features: TX/RX FIFOs, status register

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
        lui(15, UART_BASE >> 12),     // Load UART base address
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
        lui(15, UART_BASE >> 12),     // Load UART base address
        lw(14, 15, UART_STATUS_OFFSET as i32),  // Read STATUS
        // Store STATUS value to memory for verification
        lui(13, 0x80000),             // Load DRAM base
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
// UART Loopback Test (External)
// ============================================================================

#[test]
fn test_uart_loopback() {
    init_test_logger();
    
    // This test requires external loopback connection (TX->RX)
    // For simulation, we can monitor the TX signal and feed it to RX
    
    // Test data pattern
    let test_byte: u8 = 0xA5;
    
    // TODO: Implement loopback test with signal monitoring
    // This requires access to uart_tx and ability to drive uart_rx
}
```

### 8.3 UART Loopback Test Helper

For proper loopback testing, create a helper that:

1. Captures TX output transitions
2. Measures timing between transitions
3. Decodes the transmitted byte
4. Injects the byte back to RX

```rust
/// UART Loopback Test Helper
struct UartLoopbackTester {
    clk_freq_hz: u32,
    baud_rate: u32,
    clks_per_bit: u32,
    
    // TX capture state
    tx_last_value: bool,
    tx_transition_count: u32,
    tx_bit_count: u32,
    tx_shift_reg: u8,
    
    // RX injection state  
    rx_bit_timer: u32,
    rx_bits_to_send: Vec<bool>,
    rx_bit_index: usize,
}

impl UartLoopbackTester {
    pub fn new(clk_freq_hz: u32, baud_rate: u32) -> Self {
        Self {
            clk_freq_hz,
            baud_rate,
            clks_per_bit: clk_freq_hz / baud_rate,
            tx_last_value: true,
            tx_transition_count: 0,
            tx_bit_count: 0,
            tx_shift_reg: 0,
            rx_bit_timer: 0,
            rx_bits_to_send: Vec::new(),
            rx_bit_index: 0,
        }
    }
    
    /// Process one clock cycle of loopback
    /// Returns the value to drive on uart_rx
    pub fn tick(&mut self, tx_value: bool) -> bool {
        // Capture TX and decode bytes
        // ... implementation details ...
        
        // Return RX value based on captured TX
        true  // Idle high
    }
}
```

### 8.4 Test Scenarios

| Test Name | Description | Expected Result |
|-----------|-------------|-----------------|
| `test_uart_constants` | Verify UART memory map constants | All addresses correct |
| `test_uart_tx_write_byte` | Write single byte to TX FIFO | No error, byte queued |
| `test_uart_status_initial` | Check initial STATUS register | TX_EMPTY=1, RX_EMPTY=1 |
| `test_uart_tx_fifo_full` | Write 9+ bytes to TX FIFO | TX_FULL set after 8 writes |
| `test_uart_rx_read_empty` | Read RXDATA when empty | Returns 0, no crash |
| `test_uart_loopback_byte` | Send/receive single byte (loopback) | Received == Transmitted |
| `test_uart_loopback_pattern` | Send/receive 0x00, 0xFF, 0xAA, 0x55 | All patterns match |
| `test_uart_framing_error` | Inject missing stop bit | RX_ERROR set in STATUS |

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
  - [ ] Add UART module parameters
  - [ ] Add uart_tx/uart_rx port signals
  - [ ] Add UART address decoder constants
  - [ ] Update address decode logic
  - [ ] Update response mux logic
  - [ ] Instantiate UART module

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
  - [ ] Add uart.sv to create_cpu_runtime()

- [ ] Update `cpu-sim/src/sim.rs`
  - [ ] Add uart_tx() accessor to SimulatorView
  - [ ] Consider uart_rx injection mechanism

- [ ] Clear Verilator cache
  - [ ] `cargo clean`

### Phase 3: Testing

- [ ] Create `cpu-sim/tests/test_uart.rs`
  - [ ] test_uart_constants()
  - [ ] test_uart_tx_write_byte()
  - [ ] test_uart_status_initial_state()
  - [ ] test_uart_tx_fifo_full()
  - [ ] test_uart_rx_read_empty()
  
- [ ] Create UART loopback test infrastructure
  - [ ] UartLoopbackTester helper struct
  - [ ] test_uart_loopback_byte()
  - [ ] test_uart_loopback_pattern()

- [ ] Run all tests
  - [ ] `cargo test --verbose`

### Phase 4: Documentation

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

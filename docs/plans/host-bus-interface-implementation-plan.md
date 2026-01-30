# Host Bus Interface Implementation Plan
## RTL-Based Host Communication Interface with Serialized Bus Transactions

**Author:** GitHub Copilot FPGA Architect Agent  
**Date:** January 30, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Based on:** `rtl/peripherals/uart.sv` and `rtl/bus.sv` reference implementations  
**Status:** Planning

---

## Executive Summary

This document provides a detailed technical implementation plan for adding a **host bus interface module** (`host_bus_interface.sv`) to the RISC-V CPU. This module enables communication between an external host and the system by serializing bus transactions over a generic byte-stream interface.

### Key Features

1. **Bus Slave Interface:** Accepts memory-mapped bus requests from the CPU/system bus
2. **Serialized Communication:** 8-bit TX/RX data channels with flow control for variable latency
3. **Pull Model:** Single transaction at a time; no unsolicited responses from host
4. **Transport Agnostic:** Generic byte stream allows UART, USB, SPI, or custom transport
5. **Blocking Transactions:** Bus requests block until host response is received

### Design Philosophy

The module acts as a **protocol bridge** that:
- Receives bus requests (address, data, size, read/write) from the system
- Serializes these into a compact packet format
- Transmits the packet to an external host via byte-stream TX interface
- Waits for a response packet from the host via byte-stream RX interface
- Deserializes the response and completes the bus transaction

This design is intentionally simple (pull-only, one transaction at a time) to avoid multi-master bus complexities and ensure reliable operation.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Module Interface Specification](#2-module-interface-specification)
3. [Packet Protocol Design](#3-packet-protocol-design)
4. [State Machine Design](#4-state-machine-design)
5. [RTL Implementation](#5-rtl-implementation)
6. [Rust Integration Layer](#6-rust-integration-layer)
7. [Testing Strategy](#7-testing-strategy)
8. [Implementation Checklist](#8-implementation-checklist)
9. [Future Extensions](#9-future-extensions)

---

## 1. Architecture Overview

### 1.1 Block Diagram

```
                              ┌─────────────────────────────────────────────────────────────────┐
                              │                    host_bus_interface.sv                        │
                              │                                                                  │
   ┌──────────────────────────┼────────────────────────────────┐    ┌────────────────────────────┤
   │    Bus Slave Interface   │                                │    │    Host TX/RX Interface    │
   │    (from System Bus)     │                                │    │    (to External Host)      │
   │                          │                                │    │                            │
   │  addr[31:0] ─────────────┼──▶ ┌──────────────────────┐    │    │                            │
   │  wdata[31:0] ────────────┼──▶ │                      │    │    │                            │
   │  rdata[31:0] ◀───────────┼─── │   Request Capture    │    │    │                            │
   │  we ─────────────────────┼──▶ │   & Serializer       │────┼────┼──▶ tx_data[7:0]            │
   │  size[1:0] ──────────────┼──▶ │                      │    │    │                            │
   │  req ────────────────────┼──▶ └──────────┬───────────┘    │    │    tx_valid ───────────────┼──▶
   │  ready ◀─────────────────┼───            │                │    │    tx_ready ◀──────────────┼───
   │                          │               │                │    │                            │
   └──────────────────────────┼───            │                │    │                            │
                              │    ┌──────────▼───────────┐    │    │                            │
                              │    │                      │    │    │    rx_data[7:0] ◀──────────┼───
                              │    │   Response           │◀───┼────┼───                         │
                              │    │   Deserializer       │    │    │    rx_valid ◀──────────────┼───
                              │    │                      │    │    │    rx_ready ───────────────┼──▶
                              │    └──────────────────────┘    │    │                            │
                              │                                │    │                            │
                              │    ┌──────────────────────┐    │    │                            │
                              │    │   Transaction FSM    │    │    │                            │
                              │    │   (State Machine)    │    │    │                            │
                              │    └──────────────────────┘    │    │                            │
                              │                                │    │                            │
                              └────────────────────────────────┴────┴────────────────────────────┘
```

### 1.2 Transaction Flow

1. **Request Phase:**
   - System bus asserts `req` with address, data (for writes), size, and write enable
   - Module captures request and holds `ready` low (blocking the bus)
   - Module serializes request into packet and transmits via TX interface

2. **Wait Phase:**
   - Module waits for response packet from host via RX interface
   - TX interface may be idle during this phase

3. **Response Phase:**
   - Host sends response packet (read data for reads, acknowledgement for writes)
   - Module deserializes response and presents read data on `rdata`
   - Module asserts `ready` to complete the bus transaction

### 1.3 Design Constraints

| Constraint | Description |
|------------|-------------|
| Single Transaction | Only one request in flight at a time |
| Pull Model | Host never sends unsolicited data |
| Blocking | Bus stalls until response received |
| 8-bit Interface | TX/RX data paths are 8 bits wide |
| Flow Control | Valid/ready handshake on TX/RX |
| No Timeout | Module waits indefinitely for response |

---

## 2. Module Interface Specification

### 2.1 Module Ports

```systemverilog
module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // ============================================================
    // Bus Slave Interface (from System Bus)
    // ============================================================
    input  logic [31:0] addr,       // Address for bus transaction
    input  logic [31:0] wdata,      // Write data (for writes)
    output logic [31:0] rdata,      // Read data (for reads)
    input  logic        we,         // Write enable (1 = write, 0 = read)
    input  logic [1:0]  size,       // Access size (00=byte, 01=half, 10=word)
    input  logic        req,        // Request valid
    output logic        ready,      // Transaction complete
    
    // ============================================================
    // Host TX Interface (to External Host)
    // ============================================================
    output logic [7:0]  tx_data,    // Transmit data byte
    output logic        tx_valid,   // Transmit data valid
    input  logic        tx_ready,   // Host ready to accept data
    
    // ============================================================
    // Host RX Interface (from External Host)
    // ============================================================
    input  logic [7:0]  rx_data,    // Receive data byte
    input  logic        rx_valid,   // Receive data valid
    output logic        rx_ready    // Module ready to accept data
);
```

### 2.2 Bus Slave Interface Signals

| Signal | Direction | Width | Description |
|--------|-----------|-------|-------------|
| `addr` | Input | 32 | Full 32-bit address |
| `wdata` | Input | 32 | Write data (ignored for reads) |
| `rdata` | Output | 32 | Read data (valid when `ready` asserted for reads) |
| `we` | Input | 1 | Write enable: 1 = write, 0 = read |
| `size` | Input | 2 | Access size: 00=byte (8-bit), 01=halfword (16-bit), 10=word (32-bit) |
| `req` | Input | 1 | Request valid (starts transaction when asserted) |
| `ready` | Output | 1 | Transaction complete (asserted for one cycle) |

**Bus Handshake Protocol:**
- Transaction starts when `req` is asserted
- Module captures `addr`, `wdata`, `we`, `size` on the cycle `req` goes high
- `ready` stays LOW while transaction is in progress
- `ready` goes HIGH for one cycle when response is received
- For reads, `rdata` is valid when `ready` is asserted

### 2.3 Host TX Interface Signals

| Signal | Direction | Width | Description |
|--------|-----------|-------|-------------|
| `tx_data` | Output | 8 | Byte to transmit to host |
| `tx_valid` | Output | 1 | Data on `tx_data` is valid |
| `tx_ready` | Input | 1 | Host is ready to accept data |

**TX Handshake Protocol:**
- Data transfer occurs when `tx_valid` AND `tx_ready` are both high
- Module can hold `tx_valid` high until `tx_ready` goes high
- One byte transferred per handshake

### 2.4 Host RX Interface Signals

| Signal | Direction | Width | Description |
|--------|-----------|-------|-------------|
| `rx_data` | Input | 8 | Byte received from host |
| `rx_valid` | Input | 1 | Data on `rx_data` is valid |
| `rx_ready` | Output | 1 | Module is ready to accept data |

**RX Handshake Protocol:**
- Data transfer occurs when `rx_valid` AND `rx_ready` are both high
- Host can hold `rx_valid` high until `rx_ready` goes high
- One byte received per handshake

---

## 3. Packet Protocol Design

### 3.1 Request Packet Format

The request packet serializes a bus transaction into a compact byte stream:

```
Request Packet (10 bytes total):
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Command/Flags: [7:4]=reserved, [3:2]=size, [1]=0, [0]=we │
│ Byte 1    │ Address[31:24]                                       │
│ Byte 2    │ Address[23:16]                                       │
│ Byte 3    │ Address[15:8]                                        │
│ Byte 4    │ Address[7:0]                                         │
│ Byte 5    │ Write Data[31:24] (always sent, ignored for reads)   │
│ Byte 6    │ Write Data[23:16]                                    │
│ Byte 7    │ Write Data[15:8]                                     │
│ Byte 8    │ Write Data[7:0]                                      │
│ Byte 9    │ Checksum (XOR of bytes 0-8)                          │
└─────────────────────────────────────────────────────────────────┘
```

**Command/Flags Byte (Byte 0):**

| Bits | Field | Description |
|------|-------|-------------|
| [0] | `we` | Write enable: 1 = write, 0 = read |
| [1] | Reserved | Always 0 |
| [3:2] | `size` | Access size: 00=byte, 01=half, 10=word |
| [7:4] | Reserved | Must be 0 |

**Command Byte Examples:**
- Read, byte size: `0x00` (size=00, we=0)
- Read, word size: `0x08` (size=10, we=0)
- Write, halfword size: `0x05` (size=01, we=1)
- Write, word size: `0x09` (size=10, we=1)

**Design Rationale:**
- Fixed 10-byte packet simplifies parsing (no length field needed)
- Write data always sent (even for reads) to maintain fixed packet size
- Checksum provides basic error detection
- Big-endian byte order for address and data (MSB first)

### 3.2 Response Packet Format

The response packet returns data (for reads) or acknowledgement (for writes):

```
Response Packet (6 bytes total):
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Status: [7:4]=reserved, [3:0]=status_code           │
│ Byte 1    │ Read Data[31:24] (0x00 for writes)                   │
│ Byte 2    │ Read Data[23:16]                                     │
│ Byte 3    │ Read Data[15:8]                                      │
│ Byte 4    │ Read Data[7:0]                                       │
│ Byte 5    │ Checksum (XOR of bytes 0-4)                          │
└─────────────────────────────────────────────────────────────────┘
```

**Status Byte (Byte 0):**

| Value | Name | Description |
|-------|------|-------------|
| 0x00 | `STATUS_OK` | Transaction completed successfully |
| 0x01 | `STATUS_ERROR` | Transaction failed (reserved for future) |

**Design Rationale:**
- Fixed 6-byte response simplifies parsing
- Read data always present (zeros for writes)
- Status code allows for error reporting (future extension)
- Checksum provides basic error detection

### 3.3 Checksum Calculation

Simple XOR checksum of all preceding bytes:

```
checksum = byte[0] XOR byte[1] XOR byte[2] XOR ... XOR byte[N-2]
```

**For Request (10 bytes):**
```
checksum = byte[0] XOR byte[1] XOR ... XOR byte[8]
```

**For Response (6 bytes):**
```
checksum = byte[0] XOR byte[1] XOR ... XOR byte[4]
```

---

## 4. State Machine Design

### 4.1 Main Transaction FSM

```
                           ┌──────────────┐
                           │    IDLE      │◀──────────────────────────┐
                           │              │                           │
                           └──────┬───────┘                           │
                                  │ req && !in_transaction            │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │   CAPTURE    │ Capture addr, wdata,      │
                           │   REQUEST    │ we, size into registers   │
                           └──────┬───────┘                           │
                                  │                                   │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  TX_BYTE_0   │ Transmit command/flags    │
                           │              │                           │
                           └──────┬───────┘                           │
                                  │ tx_valid && tx_ready              │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  TX_BYTE_1   │ Transmit addr[31:24]      │
                           │   ...        │                           │
                           └──────┬───────┘                           │
                                  │                                   │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  TX_BYTE_9   │ Transmit checksum         │
                           │              │                           │
                           └──────┬───────┘                           │
                                  │ tx_valid && tx_ready              │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  RX_BYTE_0   │ Receive status            │
                           │              │                           │
                           └──────┬───────┘                           │
                                  │ rx_valid && rx_ready              │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  RX_BYTE_1   │ Receive rdata[31:24]      │
                           │   ...        │                           │
                           └──────┬───────┘                           │
                                  │                                   │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │  RX_BYTE_5   │ Receive checksum          │
                           │              │                           │
                           └──────┬───────┘                           │
                                  │ rx_valid && rx_ready              │
                                  ▼                                   │
                           ┌──────────────┐                           │
                           │   COMPLETE   │ Assert ready for 1 cycle  │
                           │              │ Present rdata             │
                           └──────┬───────┘                           │
                                  │                                   │
                                  └───────────────────────────────────┘
```

### 4.2 State Encoding

```systemverilog
typedef enum logic [4:0] {
    // Idle state - waiting for request
    STATE_IDLE        = 5'd0,
    
    // Capture request from bus
    STATE_CAPTURE     = 5'd1,
    
    // Transmit request packet (10 bytes)
    STATE_TX_BYTE_0   = 5'd2,   // Command/Flags
    STATE_TX_BYTE_1   = 5'd3,   // Address[31:24]
    STATE_TX_BYTE_2   = 5'd4,   // Address[23:16]
    STATE_TX_BYTE_3   = 5'd5,   // Address[15:8]
    STATE_TX_BYTE_4   = 5'd6,   // Address[7:0]
    STATE_TX_BYTE_5   = 5'd7,   // WData[31:24]
    STATE_TX_BYTE_6   = 5'd8,   // WData[23:16]
    STATE_TX_BYTE_7   = 5'd9,   // WData[15:8]
    STATE_TX_BYTE_8   = 5'd10,  // WData[7:0]
    STATE_TX_BYTE_9   = 5'd11,  // Checksum
    
    // Receive response packet (6 bytes)
    STATE_RX_BYTE_0   = 5'd12,  // Status
    STATE_RX_BYTE_1   = 5'd13,  // RData[31:24]
    STATE_RX_BYTE_2   = 5'd14,  // RData[23:16]
    STATE_RX_BYTE_3   = 5'd15,  // RData[15:8]
    STATE_RX_BYTE_4   = 5'd16,  // RData[7:0]
    STATE_RX_BYTE_5   = 5'd17,  // Checksum
    
    // Complete transaction
    STATE_COMPLETE    = 5'd18
} state_t;
```

### 4.3 State Transition Logic

**Key Transitions:**

1. **IDLE → CAPTURE:** When `req` is asserted and no transaction in progress
2. **CAPTURE → TX_BYTE_0:** Immediately after capturing request
3. **TX_BYTE_N → TX_BYTE_N+1:** When `tx_valid && tx_ready` (byte transferred)
4. **TX_BYTE_9 → RX_BYTE_0:** After last TX byte is sent
5. **RX_BYTE_N → RX_BYTE_N+1:** When `rx_valid && rx_ready` (byte received)
6. **RX_BYTE_5 → COMPLETE:** After checksum received (validation optional)
7. **COMPLETE → IDLE:** After one cycle with `ready` asserted

---

## 5. RTL Implementation

### 5.1 Module Header and Parameters

```systemverilog
// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - 10-byte request packets, 6-byte response packets
//   - Pull-only model: single transaction at a time
//   - XOR checksum for basic error detection
//
// Protocol:
//   Request:  [cmd][addr3][addr2][addr1][addr0][wdata3][wdata2][wdata1][wdata0][cksum]
//   Response: [status][rdata3][rdata2][rdata1][rdata0][cksum]

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus Slave Interface (from System Bus)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,
    
    // Host TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,
    
    // Host RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready
);
```

### 5.2 Internal Signals and Registers

```systemverilog
    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [4:0] {
        STATE_IDLE        = 5'd0,
        STATE_CAPTURE     = 5'd1,
        STATE_TX_BYTE_0   = 5'd2,
        STATE_TX_BYTE_1   = 5'd3,
        STATE_TX_BYTE_2   = 5'd4,
        STATE_TX_BYTE_3   = 5'd5,
        STATE_TX_BYTE_4   = 5'd6,
        STATE_TX_BYTE_5   = 5'd7,
        STATE_TX_BYTE_6   = 5'd8,
        STATE_TX_BYTE_7   = 5'd9,
        STATE_TX_BYTE_8   = 5'd10,
        STATE_TX_BYTE_9   = 5'd11,
        STATE_RX_BYTE_0   = 5'd12,
        STATE_RX_BYTE_1   = 5'd13,
        STATE_RX_BYTE_2   = 5'd14,
        STATE_RX_BYTE_3   = 5'd15,
        STATE_RX_BYTE_4   = 5'd16,
        STATE_RX_BYTE_5   = 5'd17,
        STATE_COMPLETE    = 5'd18
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Request Registers
    // ============================================================
    logic [31:0] cap_addr;      // Captured address
    logic [31:0] cap_wdata;     // Captured write data
    logic        cap_we;        // Captured write enable
    logic [1:0]  cap_size;      // Captured access size
    
    // ============================================================
    // Response Data Registers
    // ============================================================
    logic [7:0]  resp_status;   // Received status byte
    logic [31:0] resp_rdata;    // Received read data
    logic [7:0]  resp_checksum; // Received checksum
    
    // ============================================================
    // Checksum Calculation
    // ============================================================
    logic [7:0]  tx_checksum;   // Running TX checksum
    logic [7:0]  rx_checksum;   // Running RX checksum
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
```

### 5.3 State Machine Logic

```systemverilog
    // ============================================================
    // State Register
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;
        end else begin
            state <= next_state;
        end
    end
    
    // ============================================================
    // Next State Logic
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                if (req) begin
                    next_state = STATE_CAPTURE;
                end
            end
            
            STATE_CAPTURE: begin
                next_state = STATE_TX_BYTE_0;
            end
            
            // TX States: advance when tx_valid && tx_ready
            STATE_TX_BYTE_0: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_1;
            STATE_TX_BYTE_1: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_2;
            STATE_TX_BYTE_2: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_3;
            STATE_TX_BYTE_3: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_4;
            STATE_TX_BYTE_4: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_5;
            STATE_TX_BYTE_5: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_6;
            STATE_TX_BYTE_6: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_7;
            STATE_TX_BYTE_7: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_8;
            STATE_TX_BYTE_8: if (tx_valid && tx_ready) next_state = STATE_TX_BYTE_9;
            STATE_TX_BYTE_9: if (tx_valid && tx_ready) next_state = STATE_RX_BYTE_0;
            
            // RX States: advance when rx_valid && rx_ready
            STATE_RX_BYTE_0: if (rx_valid && rx_ready) next_state = STATE_RX_BYTE_1;
            STATE_RX_BYTE_1: if (rx_valid && rx_ready) next_state = STATE_RX_BYTE_2;
            STATE_RX_BYTE_2: if (rx_valid && rx_ready) next_state = STATE_RX_BYTE_3;
            STATE_RX_BYTE_3: if (rx_valid && rx_ready) next_state = STATE_RX_BYTE_4;
            STATE_RX_BYTE_4: if (rx_valid && rx_ready) next_state = STATE_RX_BYTE_5;
            STATE_RX_BYTE_5: if (rx_valid && rx_ready) next_state = STATE_COMPLETE;
            
            STATE_COMPLETE: begin
                next_state = STATE_IDLE;
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end
```

### 5.4 Request Capture Logic

```systemverilog
    // ============================================================
    // Capture Request on CAPTURE state
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cap_addr  <= 32'h0;
            cap_wdata <= 32'h0;
            cap_we    <= 1'b0;
            cap_size  <= 2'b00;
        end else if (state == STATE_IDLE && req) begin
            // Capture on rising edge of req while idle
            cap_addr  <= addr;
            cap_wdata <= wdata;
            cap_we    <= we;
            cap_size  <= size;
        end
    end
```

### 5.5 TX Data Path

```systemverilog
    // ============================================================
    // TX Data Multiplexer
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            STATE_TX_BYTE_0: tx_byte = {4'b0000, cap_size, 1'b0, cap_we};  // Command
            STATE_TX_BYTE_1: tx_byte = cap_addr[31:24];
            STATE_TX_BYTE_2: tx_byte = cap_addr[23:16];
            STATE_TX_BYTE_3: tx_byte = cap_addr[15:8];
            STATE_TX_BYTE_4: tx_byte = cap_addr[7:0];
            STATE_TX_BYTE_5: tx_byte = cap_wdata[31:24];
            STATE_TX_BYTE_6: tx_byte = cap_wdata[23:16];
            STATE_TX_BYTE_7: tx_byte = cap_wdata[15:8];
            STATE_TX_BYTE_8: tx_byte = cap_wdata[7:0];
            STATE_TX_BYTE_9: tx_byte = tx_checksum;  // Checksum
            default: tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    assign tx_valid = (state >= STATE_TX_BYTE_0) && (state <= STATE_TX_BYTE_9);
    
    // ============================================================
    // TX Checksum Calculation (XOR of bytes 0-8)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            tx_checksum <= 8'h00;
        end else if (state == STATE_CAPTURE) begin
            // Initialize checksum at start of transaction
            tx_checksum <= 8'h00;
        end else if (tx_valid && tx_ready && state != STATE_TX_BYTE_9) begin
            // XOR each transmitted byte (except checksum itself)
            tx_checksum <= tx_checksum ^ tx_byte;
        end
    end
```

### 5.6 RX Data Path

```systemverilog
    // ============================================================
    // RX Ready Signal
    // ============================================================
    assign rx_ready = (state >= STATE_RX_BYTE_0) && (state <= STATE_RX_BYTE_5);
    
    // ============================================================
    // RX Data Capture
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_status   <= 8'h00;
            resp_rdata    <= 32'h0;
            resp_checksum <= 8'h00;
        end else if (rx_valid && rx_ready) begin
            case (state)
                STATE_RX_BYTE_0: resp_status <= rx_data;
                STATE_RX_BYTE_1: resp_rdata[31:24] <= rx_data;
                STATE_RX_BYTE_2: resp_rdata[23:16] <= rx_data;
                STATE_RX_BYTE_3: resp_rdata[15:8]  <= rx_data;
                STATE_RX_BYTE_4: resp_rdata[7:0]   <= rx_data;
                STATE_RX_BYTE_5: resp_checksum     <= rx_data;
                default: ;
            endcase
        end
    end
    
    // ============================================================
    // RX Checksum Calculation (for verification - optional)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_checksum <= 8'h00;
        end else if (state == STATE_TX_BYTE_9 && tx_valid && tx_ready) begin
            // Reset RX checksum when transitioning to RX phase
            rx_checksum <= 8'h00;
        end else if (rx_valid && rx_ready && state != STATE_RX_BYTE_5) begin
            // XOR each received byte (except checksum itself)
            rx_checksum <= rx_checksum ^ rx_data;
        end
    end
```

### 5.7 Bus Response Logic

```systemverilog
    // ============================================================
    // Bus Ready Signal
    // ============================================================
    assign ready = (state == STATE_COMPLETE);
    
    // ============================================================
    // Bus Read Data
    // ============================================================
    assign rdata = resp_rdata;

endmodule
```

### 5.8 Complete Module

The complete module combines all sections above into a single file at `rtl/host_bus_interface.sv`.

---

## 6. Rust Integration Layer

### 6.1 Add Module Definition to riscv_core/src/lib.rs

Add the following to `riscv_core/src/lib.rs`:

```rust
// Define Host Bus Interface module
#[verilog(src = "../rtl/host_bus_interface.sv", name = "host_bus_interface")]
pub struct HostBusInterface;

// Helper function to create a runtime for the Host Bus Interface
pub fn create_host_bus_interface_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["host_bus_interface.sv"])
}
```

### 6.2 Module Not Integrated into Top

**Important Note:** Per the requirements, this module is NOT integrated into `top.sv` or the `cpu-sim` project. The module is provided as a standalone RTL component with its own testbench. Integration will be handled in a future change.

---

## 7. Testing Strategy

### 7.1 Overview

Testing follows the RTL-focused testbench pattern used by other modules in this project (e.g., `uart_test.rs`, `alu_test.rs`). Tests directly instantiate the `host_bus_interface` module via Verilator and exercise its behavior at the signal level.

**Test File Location:** `testbench/tests/host_bus_interface_test.rs`

### 7.2 Test Categories

| Category | Description | Test Count |
|----------|-------------|------------|
| Reset State | Verify initial state after reset | 2 |
| Basic Write | Single write transaction | 3 |
| Basic Read | Single read transaction | 2 |
| Protocol | Packet format and checksums | 2 |
| Flow Control | TX/RX backpressure handling | 3 |
| Edge Cases | Size variations, address patterns | 4 |

### 7.3 Test Infrastructure

```rust
// File: testbench/tests/host_bus_interface_test.rs

use riscv_core::{create_host_bus_interface_runtime, HostBusInterface};

// Clock cycle macro
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

/// Apply reset to the module
fn reset_module(dut: &mut HostBusInterface) {
    dut.rst_n = 0;
    dut.req = 0;
    dut.we = 0;
    dut.addr = 0;
    dut.wdata = 0;
    dut.size = 0;
    dut.tx_ready = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

/// Helper to receive a byte from TX interface
/// Returns (byte_value, success)
fn receive_tx_byte(dut: &mut HostBusInterface, max_cycles: u32) -> Option<u8> {
    for _ in 0..max_cycles {
        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            let byte = dut.tx_data as u8;
            clock_cycle!(dut);
            dut.tx_ready = 0;
            dut.eval();
            return Some(byte);
        }
        clock_cycle!(dut);
    }
    None
}

/// Helper to send a byte to RX interface
fn send_rx_byte(dut: &mut HostBusInterface, byte: u8, max_cycles: u32) -> bool {
    dut.rx_data = byte as u32;
    dut.rx_valid = 1;
    dut.eval();
    
    for _ in 0..max_cycles {
        if dut.rx_ready != 0 {
            clock_cycle!(dut);
            dut.rx_valid = 0;
            dut.eval();
            return true;
        }
        clock_cycle!(dut);
    }
    dut.rx_valid = 0;
    dut.eval();
    false
}

/// Calculate XOR checksum of bytes
fn calculate_checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc ^ b)
}
```

### 7.4 Reset State Tests

```rust
#[test]
fn test_reset_state() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Verify outputs are in expected initial state
    assert_eq!(dut.ready, 0, "ready should be LOW after reset");
    assert_eq!(dut.tx_valid, 0, "tx_valid should be LOW after reset");
    assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW after reset");
    assert_eq!(dut.rdata, 0, "rdata should be 0 after reset");
}

#[test]
fn test_idle_no_transaction() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Run for many cycles without asserting req
    for _ in 0..100 {
        assert_eq!(dut.tx_valid, 0, "tx_valid should stay LOW without request");
        assert_eq!(dut.ready, 0, "ready should stay LOW without request");
        clock_cycle!(dut);
    }
}
```

### 7.5 Basic Write Transaction Tests

```rust
#[test]
fn test_write_transaction_packet_format() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a write transaction
    dut.addr = 0x12345678;
    dut.wdata = 0xDEADBEEF;
    dut.we = 1;
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..10 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }
    
    // Verify packet format
    assert_eq!(tx_packet.len(), 10, "Request packet should be 10 bytes");
    
    // Byte 0: command = {4'b0, size[1:0], 1'b0, we} = {0, 10, 0, 1} = 0x09
    assert_eq!(tx_packet[0], 0x09, "Command byte mismatch");
    
    // Bytes 1-4: Address (big-endian)
    assert_eq!(tx_packet[1], 0x12, "Address[31:24] mismatch");
    assert_eq!(tx_packet[2], 0x34, "Address[23:16] mismatch");
    assert_eq!(tx_packet[3], 0x56, "Address[15:8] mismatch");
    assert_eq!(tx_packet[4], 0x78, "Address[7:0] mismatch");
    
    // Bytes 5-8: Write data (big-endian)
    assert_eq!(tx_packet[5], 0xDE, "WData[31:24] mismatch");
    assert_eq!(tx_packet[6], 0xAD, "WData[23:16] mismatch");
    assert_eq!(tx_packet[7], 0xBE, "WData[15:8] mismatch");
    assert_eq!(tx_packet[8], 0xEF, "WData[7:0] mismatch");
    
    // Byte 9: Checksum
    let expected_checksum = calculate_checksum(&tx_packet[0..9]);
    assert_eq!(tx_packet[9], expected_checksum, "Checksum mismatch");
}

#[test]
fn test_write_transaction_complete() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet (10 bytes)
    for _ in 0..10 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response packet (6 bytes): status=0, rdata=0, checksum
    let response = [0x00, 0x00, 0x00, 0x00, 0x00];
    let checksum = calculate_checksum(&response);
    
    for byte in response.iter() {
        assert!(send_rx_byte(&mut dut, *byte, 100), "Failed to send RX byte");
    }
    assert!(send_rx_byte(&mut dut, checksum, 100), "Failed to send checksum");
    
    // Give a cycle for state machine to complete
    clock_cycle!(dut);
    
    // Verify ready is asserted
    assert_eq!(dut.ready, 1, "ready should be HIGH after response received");
}
```

### 7.6 Basic Read Transaction Tests

```rust
#[test]
fn test_read_transaction_returns_data() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a read transaction
    dut.addr = 0xABCD1234;
    dut.we = 0;  // Read
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet
    for _ in 0..10 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response with read data = 0xCAFEBABE
    let response = [0x00, 0xCA, 0xFE, 0xBA, 0xBE];
    let checksum = calculate_checksum(&response);
    
    for byte in response.iter() {
        assert!(send_rx_byte(&mut dut, *byte, 100), "Failed to send RX byte");
    }
    assert!(send_rx_byte(&mut dut, checksum, 100), "Failed to send checksum");
    
    clock_cycle!(dut);
    
    // Verify read data
    assert_eq!(dut.ready, 1, "ready should be HIGH");
    assert_eq!(dut.rdata, 0xCAFEBABE, "Read data mismatch");
}

#[test]
fn test_read_transaction_command_byte() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a read transaction (we=0)
    dut.addr = 0x00000000;
    dut.we = 0;
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Get first byte (command)
    let cmd_byte = receive_tx_byte(&mut dut, 100).expect("Failed to receive command byte");
    
    // Command byte for read, word size: {4'b0, size=10, 1'b0, we=0} = 0x08
    assert_eq!(cmd_byte, 0x08, "Command byte for read mismatch");
}
```

### 7.7 Flow Control Tests

```rust
#[test]
fn test_tx_backpressure() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start transaction
    dut.addr = 0x11111111;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Wait until tx_valid is asserted
    for _ in 0..10 {
        if dut.tx_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(dut.tx_valid, 1, "tx_valid should be asserted");
    
    // Keep tx_ready LOW for several cycles (backpressure)
    let first_byte = dut.tx_data;
    dut.tx_ready = 0;
    for _ in 0..10 {
        clock_cycle!(dut);
        // tx_valid should remain asserted
        assert_eq!(dut.tx_valid, 1, "tx_valid should stay HIGH during backpressure");
        // tx_data should not change
        assert_eq!(dut.tx_data, first_byte, "tx_data should not change during backpressure");
    }
    
    // Now accept the byte
    dut.tx_ready = 1;
    dut.eval();
    clock_cycle!(dut);
    
    // Data should have advanced
    // (next byte should be different for non-zero address)
}

#[test]
fn test_rx_delayed_valid() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start transaction and drain TX
    dut.addr = 0x00000000;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    for _ in 0..10 {
        receive_tx_byte(&mut dut, 100).expect("TX byte");
    }
    
    // Module should now be waiting for RX
    // rx_ready should be asserted
    for _ in 0..5 {
        clock_cycle!(dut);
    }
    assert_eq!(dut.rx_ready, 1, "rx_ready should be HIGH waiting for response");
    
    // Delay sending response for many cycles
    for _ in 0..50 {
        clock_cycle!(dut);
        assert_eq!(dut.ready, 0, "ready should stay LOW waiting for response");
    }
    
    // Now send response
    let response = [0x00, 0x00, 0x00, 0x00, 0x00];
    let checksum = calculate_checksum(&response);
    for byte in response.iter() {
        send_rx_byte(&mut dut, *byte, 100);
    }
    send_rx_byte(&mut dut, checksum, 100);
    
    clock_cycle!(dut);
    assert_eq!(dut.ready, 1, "ready should be HIGH after delayed response");
}
```

### 7.8 Size Variation Tests

```rust
#[test]
fn test_byte_access_size() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Byte access (size = 00)
    dut.addr = 0x00000000;
    dut.we = 0;
    dut.size = 0b00;  // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    let cmd = receive_tx_byte(&mut dut, 100).expect("Command byte");
    // Command: {4'b0, size=00, 1'b0, we=0} = 0x00
    assert_eq!(cmd, 0x00, "Byte access command byte mismatch");
}

#[test]
fn test_halfword_access_size() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Halfword access (size = 01)
    dut.addr = 0x00000000;
    dut.we = 1;
    dut.size = 0b01;  // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    let cmd = receive_tx_byte(&mut dut, 100).expect("Command byte");
    // Command: {4'b0, size=01, 1'b0, we=1} = 0x05
    assert_eq!(cmd, 0x05, "Halfword write command byte mismatch");
}
```

### 7.9 Additional Test Cases

```rust
#[test]
fn test_write_blocking() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Verify ready stays LOW during TX phase
    for _ in 0..5 {
        assert_eq!(dut.ready, 0, "ready should be LOW during transaction");
        receive_tx_byte(&mut dut, 10);
    }
    
    // Drain remaining TX bytes
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("TX byte");
    }
    
    // Verify ready stays LOW during RX wait phase
    for _ in 0..10 {
        clock_cycle!(dut);
        assert_eq!(dut.ready, 0, "ready should be LOW waiting for response");
    }
}

#[test]
fn test_rx_ready_timing() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Verify rx_ready is LOW in IDLE state
    assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW in IDLE");
    
    // Start transaction
    dut.addr = 0x00000000;
    dut.we = 0;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // During TX phase, rx_ready should be LOW
    for _ in 0..5 {
        assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW during TX phase");
        receive_tx_byte(&mut dut, 100);
    }
    
    // Drain remaining TX
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100);
    }
    
    // After TX complete, rx_ready should go HIGH
    for _ in 0..5 {
        clock_cycle!(dut);
    }
    assert_eq!(dut.rx_ready, 1, "rx_ready should be HIGH waiting for response");
}

#[test]
fn test_all_ones_address() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Test with all-ones address
    dut.addr = 0xFFFFFFFF;
    dut.wdata = 0xFFFFFFFF;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..10 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        }
    }
    
    // Verify address bytes are all 0xFF
    assert_eq!(tx_packet[1], 0xFF, "Address[31:24] should be 0xFF");
    assert_eq!(tx_packet[2], 0xFF, "Address[23:16] should be 0xFF");
    assert_eq!(tx_packet[3], 0xFF, "Address[15:8] should be 0xFF");
    assert_eq!(tx_packet[4], 0xFF, "Address[7:0] should be 0xFF");
    
    // Verify write data bytes are all 0xFF
    assert_eq!(tx_packet[5], 0xFF, "WData[31:24] should be 0xFF");
    assert_eq!(tx_packet[6], 0xFF, "WData[23:16] should be 0xFF");
    assert_eq!(tx_packet[7], 0xFF, "WData[15:8] should be 0xFF");
    assert_eq!(tx_packet[8], 0xFF, "WData[7:0] should be 0xFF");
}

#[test]
fn test_checksum_verification() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Use known values for predictable checksum
    dut.addr = 0x00000001;
    dut.wdata = 0x00000000;
    dut.we = 0;
    dut.size = 0b10;  // Word read
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..10 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        }
    }
    
    // Calculate expected checksum
    let expected_checksum = calculate_checksum(&tx_packet[0..9]);
    
    // Verify checksum matches
    assert_eq!(
        tx_packet[9], expected_checksum,
        "TX checksum mismatch: got 0x{:02x}, expected 0x{:02x}",
        tx_packet[9], expected_checksum
    );
}

#[test]
fn test_consecutive_transactions() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Perform two back-to-back transactions
    for iteration in 0..2 {
        let test_addr = 0x80000000 + (iteration as u32 * 4);
        let test_data = 0xDEAD0000 + iteration as u32;
        
        // Start transaction
        dut.addr = test_addr;
        dut.wdata = test_data;
        dut.we = 1;
        dut.size = 0b10;
        dut.req = 1;
        clock_cycle!(dut);
        dut.req = 0;
        
        // Drain TX
        for _ in 0..10 {
            receive_tx_byte(&mut dut, 100).expect("TX byte");
        }
        
        // Send response
        let response = [0x00, 0x00, 0x00, 0x00, 0x00];
        let checksum = calculate_checksum(&response);
        for byte in response.iter() {
            send_rx_byte(&mut dut, *byte, 100);
        }
        send_rx_byte(&mut dut, checksum, 100);
        
        clock_cycle!(dut);
        
        // Verify completion
        assert_eq!(dut.ready, 1, "Transaction {} should complete", iteration);
        
        // Wait a cycle for state to return to IDLE
        clock_cycle!(dut);
    }
}
```

### 7.10 Test Summary

| Test Name | Category | Description |
|-----------|----------|-------------|
| `test_reset_state` | Reset | Verify outputs after reset |
| `test_idle_no_transaction` | Reset | Verify idle behavior |
| `test_write_transaction_packet_format` | Write | Verify TX packet format |
| `test_write_transaction_complete` | Write | Full write cycle |
| `test_write_blocking` | Write | Verify ready stays low during transaction |
| `test_read_transaction_returns_data` | Read | Verify read data path |
| `test_read_transaction_command_byte` | Read | Verify read command encoding |
| `test_tx_backpressure` | Flow | TX ready backpressure handling |
| `test_rx_delayed_valid` | Flow | Delayed RX response |
| `test_rx_ready_timing` | Flow | RX ready assertion timing |
| `test_byte_access_size` | Edge | Byte size encoding (size=00) |
| `test_halfword_access_size` | Edge | Halfword size encoding (size=01) |
| `test_all_ones_address` | Edge | 0xFFFFFFFF address handling |
| `test_checksum_verification` | Protocol | Verify TX checksum calculation |
| `test_consecutive_transactions` | Sequence | Back-to-back transactions |

---

## 8. Implementation Checklist

### Phase 1: RTL Implementation

- [ ] **Create `rtl/host_bus_interface.sv`**
  - [ ] Module header with parameters and ports
  - [ ] State machine enum definition
  - [ ] Captured request registers
  - [ ] Response data registers
  - [ ] State register with async reset
  - [ ] Next state combinational logic
  - [ ] Request capture logic
  - [ ] TX data multiplexer
  - [ ] TX valid/checksum logic
  - [ ] RX ready signal
  - [ ] RX data capture logic
  - [ ] Bus ready and rdata outputs
  
- [ ] **Verify Verilator lint**
  - [ ] Run `verilator --lint-only rtl/host_bus_interface.sv`
  - [ ] Fix any warnings or errors

### Phase 2: Rust Integration

- [ ] **Update `riscv_core/src/lib.rs`**
  - [ ] Add `HostBusInterface` struct definition with `#[verilog]` attribute
  - [ ] Add `create_host_bus_interface_runtime()` helper function

### Phase 3: Test Implementation

- [ ] **Create `testbench/tests/host_bus_interface_test.rs`**
  - [ ] Test infrastructure (macros, helpers)
  - [ ] Reset state tests
  - [ ] Basic write transaction tests
  - [ ] Basic read transaction tests
  - [ ] Flow control tests
  - [ ] Size variation tests
  - [ ] Protocol/checksum tests

- [ ] **Run all tests**
  - [ ] `cargo test --package testbench`
  - [ ] Verify all tests pass

### Phase 4: Code Quality

- [ ] **Rust formatting and linting**
  - [ ] `cargo fmt`
  - [ ] `cargo clippy --fix --allow-dirty`
  - [ ] `cargo clippy -- -D warnings`
  
- [ ] **SystemVerilog linting**
  - [ ] `verilator --lint-only rtl/host_bus_interface.sv`

### Phase 5: Documentation

- [ ] **Add module documentation**
  - [ ] Header comments in SystemVerilog file
  - [ ] Update AGENTS.md memory map (if applicable, deferred to integration)

---

## 9. Future Extensions

### 9.1 Integration with Top Module (Future Change)

The following integration work is **explicitly out of scope** for this implementation:

- Adding address decoder entry in `rtl/bus.sv`
- Instantiating `host_bus_interface` in `rtl/top.sv`
- Connecting TX/RX signals to external pins or UART
- Updating `cpu-sim` project for host bus communication

### 9.2 Potential Enhancements

| Enhancement | Description | Priority |
|-------------|-------------|----------|
| Timeout | Add optional timeout for response | Medium |
| Error Handling | Handle checksum errors gracefully | Medium |
| Burst Mode | Support multiple transactions per request | Low |
| DMA | Direct memory access from host | Low |
| Interrupt | Generate interrupt on transaction complete | Low |

### 9.3 Transport Options

The generic TX/RX byte stream interface can be connected to:

- **UART:** For serial communication (use existing `uart.sv`)
- **USB FIFO:** For high-speed USB (e.g., FTDI FT232H)
- **SPI Slave:** For SPI-based host communication
- **Custom FIFO:** For simulation or custom transports

---

## Appendix A: Packet Format Quick Reference

### Request Packet (10 bytes)

| Byte | Content | Example (Write word to 0x80001234, data=0xDEADBEEF) |
|------|---------|-----------------------------------------------------|
| 0 | `{4'b0, size[1:0], 1'b0, we}` | 0x09 (size=10, we=1) |
| 1 | `addr[31:24]` | 0x80 |
| 2 | `addr[23:16]` | 0x00 |
| 3 | `addr[15:8]` | 0x12 |
| 4 | `addr[7:0]` | 0x34 |
| 5 | `wdata[31:24]` | 0xDE |
| 6 | `wdata[23:16]` | 0xAD |
| 7 | `wdata[15:8]` | 0xBE |
| 8 | `wdata[7:0]` | 0xEF |
| 9 | `checksum` | XOR of bytes 0-8 |

### Response Packet (6 bytes)

| Byte | Content | Example (OK, rdata=0xCAFEBABE) |
|------|---------|-------------------------------|
| 0 | `status` | 0x00 (STATUS_OK) |
| 1 | `rdata[31:24]` | 0xCA |
| 2 | `rdata[23:16]` | 0xFE |
| 3 | `rdata[15:8]` | 0xBA |
| 4 | `rdata[7:0]` | 0xBE |
| 5 | `checksum` | XOR of bytes 0-4 |

---

## Appendix B: State Machine Diagram (ASCII)

```
      ┌───────────────────────────────────────────────────────────────┐
      │                                                               │
      │  IDLE ──req──▶ CAPTURE ──▶ TX_0 ──▶ TX_1 ──▶ ... ──▶ TX_9   │
      │    ▲                                                     │    │
      │    │                                                     │    │
      │    │   ┌─────────────────────────────────────────────────┘    │
      │    │   ▼                                                      │
      │    └─ COMPLETE ◀── RX_5 ◀── ... ◀── RX_1 ◀── RX_0            │
      │                                                               │
      └───────────────────────────────────────────────────────────────┘
```

---

**Document Version:** 1.0  
**Last Updated:** January 30, 2026

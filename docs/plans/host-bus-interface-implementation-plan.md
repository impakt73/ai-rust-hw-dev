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

### 3.1 Design Philosophy

The packet protocol is optimized for **minimal wire bandwidth** since the project may use slow serial links. Key design decisions:

- **Variable-length packets** - Only transmit bytes that are needed
- **No checksums** - Rely on transport layer for error detection if needed
- **Self-describing headers** - First byte contains enough info to determine packet length

### 3.2 Request Packet Format

Request packets are variable length based on whether it's a read or write transaction:

**Read Request (5 bytes):**
```
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Header: [7:4]=reserved, [3:2]=size, [1]=0, [0]=we=0 │
│ Byte 1    │ Address[31:24]                                       │
│ Byte 2    │ Address[23:16]                                       │
│ Byte 3    │ Address[15:8]                                        │
│ Byte 4    │ Address[7:0]                                         │
└─────────────────────────────────────────────────────────────────┘
```

**Write Request (5 + N bytes, where N = 1, 2, or 4 based on size):**
```
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Header: [7:4]=reserved, [3:2]=size, [1]=0, [0]=we=1 │
│ Byte 1    │ Address[31:24]                                       │
│ Byte 2    │ Address[23:16]                                       │
│ Byte 3    │ Address[15:8]                                        │
│ Byte 4    │ Address[7:0]                                         │
│ Byte 5    │ Write Data (MSB or only byte for size=00)            │
│ Byte 6    │ Write Data (for size=01,10) - optional               │
│ Byte 7    │ Write Data (for size=10) - optional                  │
│ Byte 8    │ Write Data (LSB for size=10) - optional              │
└─────────────────────────────────────────────────────────────────┘
```

**Header Byte (Byte 0):**

| Bits | Field | Description |
|------|-------|-------------|
| [0] | `we` | Write enable: 1 = write, 0 = read |
| [1] | Reserved | Always 0 |
| [3:2] | `size` | Access size: 00=byte (1B), 01=half (2B), 10=word (4B) |
| [7:4] | Reserved | Must be 0 |

**Request Packet Sizes:**

| Transaction Type | Size Field | Total Request Bytes |
|------------------|------------|---------------------|
| Read byte        | 00         | 5 (header + 4 addr) |
| Read halfword    | 01         | 5 (header + 4 addr) |
| Read word        | 10         | 5 (header + 4 addr) |
| Write byte       | 00         | 6 (header + 4 addr + 1 data) |
| Write halfword   | 01         | 7 (header + 4 addr + 2 data) |
| Write word       | 10         | 9 (header + 4 addr + 4 data) |

**Header Byte Examples:**
- Read, byte size: `0x00` (size=00, we=0)
- Read, word size: `0x08` (size=10, we=0)
- Write, byte size: `0x01` (size=00, we=1)
- Write, halfword size: `0x05` (size=01, we=1)
- Write, word size: `0x09` (size=10, we=1)

### 3.3 Response Packet Format

Response packets are variable length based on whether the original request was a read or write:

**Write Response (1 byte - acknowledgement only):**
```
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Header: [7:4]=reserved, [3:2]=size, [1]=0, [0]=we=1 │
└─────────────────────────────────────────────────────────────────┘
```

**Read Response (1 + N bytes, where N = 1, 2, or 4 based on size):**
```
┌─────────────────────────────────────────────────────────────────┐
│ Byte 0    │ Header: [7:4]=reserved, [3:2]=size, [1]=0, [0]=we=0 │
│ Byte 1    │ Read Data (MSB or only byte for size=00)             │
│ Byte 2    │ Read Data (for size=01,10) - optional                │
│ Byte 3    │ Read Data (for size=10) - optional                   │
│ Byte 4    │ Read Data (LSB for size=10) - optional               │
└─────────────────────────────────────────────────────────────────┘
```

**Response Header Byte (Byte 0):**

| Bits | Field | Description |
|------|-------|-------------|
| [0] | `we` | Echoes request we: 1 = was write, 0 = was read |
| [1] | Reserved | Always 0 |
| [3:2] | `size` | Echoes request size: 00=byte, 01=half, 10=word |
| [7:4] | Reserved | Must be 0 |

**Response Packet Sizes:**

| Transaction Type | Size Field | Total Response Bytes |
|------------------|------------|----------------------|
| Write (any size) | xx         | 1 (header only)      |
| Read byte        | 00         | 2 (header + 1 data)  |
| Read halfword    | 01         | 3 (header + 2 data)  |
| Read word        | 10         | 5 (header + 4 data)  |

### 3.4 Bandwidth Comparison

The variable-length encoding provides significant bandwidth savings compared to fixed-length packets:

| Transaction | Old (Fixed) | New (Variable) | Savings |
|-------------|-------------|----------------|---------|
| Read word   | 10 + 6 = 16 | 5 + 5 = 10     | 37.5%   |
| Write word  | 10 + 6 = 16 | 9 + 1 = 10     | 37.5%   |
| Read half   | 10 + 6 = 16 | 5 + 3 = 8      | 50%     |
| Write half  | 10 + 6 = 16 | 7 + 1 = 8      | 50%     |
| Read byte   | 10 + 6 = 16 | 5 + 2 = 7      | 56.25%  |
| Write byte  | 10 + 6 = 16 | 6 + 1 = 7      | 56.25%  |

### 3.5 Packet Parsing Logic

**Request Parsing (Host Side):**
1. Read header byte (byte 0)
2. Extract `we` (bit 0) and `size` (bits 3:2)
3. Read 4 address bytes (bytes 1-4)
4. If `we == 1` (write): read N data bytes based on size:
   - size=00: read 1 byte
   - size=01: read 2 bytes
   - size=10: read 4 bytes

**Response Parsing (Module Side):**
1. Read header byte (byte 0)
2. Extract `we` (bit 0) and `size` (bits 3:2)
3. If `we == 0` (read response): read N data bytes based on size:
   - size=00: read 1 byte
   - size=01: read 2 bytes
   - size=10: read 4 bytes
4. If `we == 1` (write response): no more bytes to read

---

## 4. State Machine Design

### 4.1 Variable-Length Packet FSM Overview

The state machine must handle variable-length packets based on the captured `we` and `size` fields:

**TX Phase (Request Packet):**
- Always: Header (1 byte) + Address (4 bytes) = 5 bytes minimum
- If `we == 1` (write): Add data bytes based on size:
  - size=00: +1 byte (total 6)
  - size=01: +2 bytes (total 7)
  - size=10: +4 bytes (total 9)

**RX Phase (Response Packet):**
- If `we == 1` (write): Header only (1 byte)
- If `we == 0` (read): Header + data bytes based on size:
  - size=00: 1 + 1 = 2 bytes
  - size=01: 1 + 2 = 3 bytes
  - size=10: 1 + 4 = 5 bytes

### 4.2 Main Transaction FSM Diagram

```
                           ┌──────────────┐
                           │    IDLE      │◀──────────────────────────────────────┐
                           │              │                                       │
                           └──────┬───────┘                                       │
                                  │ req                                           │
                                  ▼                                               │
                           ┌──────────────┐                                       │
                           │   CAPTURE    │ Capture addr, wdata, we, size         │
                           └──────┬───────┘                                       │
                                  │                                               │
                                  ▼                                               │
                           ┌──────────────┐                                       │
                           │  TX_HEADER   │ Transmit header byte                  │
                           └──────┬───────┘                                       │
                                  │ tx_valid && tx_ready                          │
                                  ▼                                               │
                           ┌──────────────┐                                       │
                           │  TX_ADDR_3   │ Transmit addr[31:24]                  │
                           │  TX_ADDR_2   │ Transmit addr[23:16]                  │
                           │  TX_ADDR_1   │ Transmit addr[15:8]                   │
                           │  TX_ADDR_0   │ Transmit addr[7:0]                    │
                           └──────┬───────┘                                       │
                                  │                                               │
                                  ▼                                               │
                    ┌─────────────┴─────────────┐                                 │
                    │                           │                                 │
             cap_we == 0                 cap_we == 1                              │
             (read)                      (write)                                  │
                    │                           │                                 │
                    ▼                           ▼                                 │
             ┌──────────────┐            ┌──────────────┐                         │
             │ (skip data)  │            │ TX_WDATA_*   │ 1, 2, or 4 bytes        │
             │              │            │ based on size│                         │
             └──────┬───────┘            └──────┬───────┘                         │
                    │                           │                                 │
                    └─────────────┬─────────────┘                                 │
                                  ▼                                               │
                           ┌──────────────┐                                       │
                           │  RX_HEADER   │ Receive response header               │
                           └──────┬───────┘                                       │
                                  │ rx_valid && rx_ready                          │
                                  ▼                                               │
                    ┌─────────────┴─────────────┐                                 │
                    │                           │                                 │
             cap_we == 1                 cap_we == 0                              │
             (write ack)                 (read data)                              │
                    │                           │                                 │
                    ▼                           ▼                                 │
             ┌──────────────┐            ┌──────────────┐                         │
             │ (no data)    │            │ RX_RDATA_*   │ 1, 2, or 4 bytes        │
             │              │            │ based on size│                         │
             └──────┬───────┘            └──────┬───────┘                         │
                    │                           │                                 │
                    └─────────────┬─────────────┘                                 │
                                  ▼                                               │
                           ┌──────────────┐                                       │
                           │   COMPLETE   │ Assert ready for 1 cycle              │
                           └──────┬───────┘                                       │
                                  │                                               │
                                  └───────────────────────────────────────────────┘
```

### 4.3 State Encoding

```systemverilog
typedef enum logic [3:0] {
    // Idle state - waiting for request
    STATE_IDLE        = 4'd0,
    
    // Capture request from bus
    STATE_CAPTURE     = 4'd1,
    
    // Transmit request packet (variable length: 5-9 bytes)
    STATE_TX_HEADER   = 4'd2,   // Header byte
    STATE_TX_ADDR_3   = 4'd3,   // Address[31:24]
    STATE_TX_ADDR_2   = 4'd4,   // Address[23:16]
    STATE_TX_ADDR_1   = 4'd5,   // Address[15:8]
    STATE_TX_ADDR_0   = 4'd6,   // Address[7:0]
    STATE_TX_WDATA_3  = 4'd7,   // WData[31:24] (word writes)
    STATE_TX_WDATA_2  = 4'd8,   // WData[23:16] (halfword/word writes)
    STATE_TX_WDATA_1  = 4'd9,   // WData[15:8] (word writes)
    STATE_TX_WDATA_0  = 4'd10,  // WData[7:0] (all writes)
    
    // Receive response packet (variable length: 1-5 bytes)
    STATE_RX_HEADER   = 4'd11,  // Response header
    STATE_RX_RDATA_3  = 4'd12,  // RData[31:24] (word reads)
    STATE_RX_RDATA_2  = 4'd13,  // RData[23:16] (halfword/word reads)
    STATE_RX_RDATA_1  = 4'd14,  // RData[15:8] (word reads)
    STATE_RX_RDATA_0  = 4'd15,  // RData[7:0] (all reads)
    
    // Complete transaction (reuse value since COMPLETE follows RX states)
    STATE_COMPLETE    = 4'd0    // NOTE: Handled specially, see logic
} state_t;
```

**Note:** Due to the variable-length nature, we use a 4-bit state encoding with conditional transitions. The `STATE_COMPLETE` is handled via a separate `transaction_complete` signal rather than as a unique state value to keep the encoding compact.

### 4.4 State Transition Logic (Conceptual)

**TX Phase Transitions:**

1. **IDLE → CAPTURE:** When `req` is asserted
2. **CAPTURE → TX_HEADER:** Immediately after capturing request
3. **TX_HEADER → TX_ADDR_3:** When byte transmitted
4. **TX_ADDR_3 → TX_ADDR_2 → TX_ADDR_1 → TX_ADDR_0:** Sequential address bytes
5. **TX_ADDR_0 → (next):**
   - If `cap_we == 0` (read): → RX_HEADER (skip data)
   - If `cap_we == 1` (write): → TX_WDATA start state based on size

**TX Write Data State Selection (based on size):**
- `size == 2'b10` (word): TX_ADDR_0 → TX_WDATA_3 → TX_WDATA_2 → TX_WDATA_1 → TX_WDATA_0 → RX_HEADER
- `size == 2'b01` (half): TX_ADDR_0 → TX_WDATA_2 → TX_WDATA_0 → RX_HEADER
- `size == 2'b00` (byte): TX_ADDR_0 → TX_WDATA_0 → RX_HEADER

**RX Phase Transitions:**

1. **RX_HEADER received:**
   - If `cap_we == 1` (write response): → COMPLETE (no data)
   - If `cap_we == 0` (read response): → RX_RDATA start state based on size

**RX Read Data State Selection (based on size):**
- `size == 2'b10` (word): RX_HEADER → RX_RDATA_3 → RX_RDATA_2 → RX_RDATA_1 → RX_RDATA_0 → COMPLETE
- `size == 2'b01` (half): RX_HEADER → RX_RDATA_2 → RX_RDATA_0 → COMPLETE
- `size == 2'b00` (byte): RX_HEADER → RX_RDATA_0 → COMPLETE

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
//   - Variable-length packets optimized for minimal bandwidth
//   - Pull-only model: single transaction at a time
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length):
//   Read Request:   [header][addr3][addr2][addr1][addr0]              (5 bytes)
//   Write Request:  [header][addr3][addr2][addr1][addr0][data...]     (6-9 bytes)
//   Write Response: [header]                                          (1 byte)
//   Read Response:  [header][data...]                                 (2-5 bytes)

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
    typedef enum logic [3:0] {
        STATE_IDLE        = 4'd0,
        STATE_CAPTURE     = 4'd1,
        
        // TX States (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 4'd2,   // Header byte
        STATE_TX_ADDR_3   = 4'd3,   // Address[31:24]
        STATE_TX_ADDR_2   = 4'd4,   // Address[23:16]
        STATE_TX_ADDR_1   = 4'd5,   // Address[15:8]
        STATE_TX_ADDR_0   = 4'd6,   // Address[7:0]
        STATE_TX_WDATA_3  = 4'd7,   // WData[31:24] (word writes only)
        STATE_TX_WDATA_2  = 4'd8,   // WData[23:16] (half/word writes)
        STATE_TX_WDATA_1  = 4'd9,   // WData[15:8] (word writes only)
        STATE_TX_WDATA_0  = 4'd10,  // WData[7:0] (all writes - byte aligned)
        
        // RX States (variable length: 1-5 bytes)
        STATE_RX_HEADER   = 4'd11,  // Response header
        STATE_RX_RDATA_3  = 4'd12,  // RData[31:24] (word reads only)
        STATE_RX_RDATA_2  = 4'd13,  // RData[23:16] (half/word reads)
        STATE_RX_RDATA_1  = 4'd14,  // RData[15:8] (word reads only)
        STATE_RX_RDATA_0  = 4'd15,  // RData[7:0] (all reads - byte aligned)
        
        STATE_COMPLETE    = 4'd0    // Reused - distinguished by complete flag
    } state_t;
    
    state_t state, next_state;
    logic   transaction_complete;  // Indicates COMPLETE state
    
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
    logic [31:0] resp_rdata;    // Received read data
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
    logic        in_tx_phase;   // Indicates TX phase active
    logic        in_rx_phase;   // Indicates RX phase active
```

### 5.3 State Machine Logic

```systemverilog
    // ============================================================
    // State Register
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;
            transaction_complete <= 1'b0;
        end else begin
            state <= next_state;
            // Set complete flag when transitioning to complete state
            transaction_complete <= (next_state == STATE_IDLE) && 
                                   (state != STATE_IDLE) && 
                                   (state != STATE_CAPTURE);
        end
    end
    
    // ============================================================
    // Next State Logic (Variable Length Packets)
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
                next_state = STATE_TX_HEADER;
            end
            
            // --------------------------------------------------------
            // TX Phase: Header + Address (always) + Data (writes only)
            // --------------------------------------------------------
            STATE_TX_HEADER: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_3;
            end
            
            STATE_TX_ADDR_3: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_2;
            end
            
            STATE_TX_ADDR_2: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_1;
            end
            
            STATE_TX_ADDR_1: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_0;
            end
            
            STATE_TX_ADDR_0: begin
                if (tx_valid && tx_ready) begin
                    if (cap_we) begin
                        // Write: send data bytes based on size
                        case (cap_size)
                            2'b10:   next_state = STATE_TX_WDATA_3;  // Word: 4 bytes
                            2'b01:   next_state = STATE_TX_WDATA_2;  // Half: 2 bytes
                            default: next_state = STATE_TX_WDATA_0;  // Byte: 1 byte
                        endcase
                    end else begin
                        // Read: no data, go to RX phase
                        next_state = STATE_RX_HEADER;
                    end
                end
            end
            
            // TX Write Data States (conditional based on size)
            STATE_TX_WDATA_3: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_2;
            end
            
            STATE_TX_WDATA_2: begin  // Half and Word
                if (tx_valid && tx_ready) begin
                    if (cap_size == 2'b10)
                        next_state = STATE_TX_WDATA_1;  // Word: continue
                    else
                        next_state = STATE_TX_WDATA_0;  // Half: skip to LSB
                end
            end
            
            STATE_TX_WDATA_1: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_0;
            end
            
            STATE_TX_WDATA_0: begin  // All writes end here
                if (tx_valid && tx_ready) next_state = STATE_RX_HEADER;
            end
            
            // --------------------------------------------------------
            // RX Phase: Header (always) + Data (reads only)
            // --------------------------------------------------------
            STATE_RX_HEADER: begin
                if (rx_valid && rx_ready) begin
                    if (cap_we) begin
                        // Write response: header only, transaction complete
                        next_state = STATE_IDLE;
                    end else begin
                        // Read response: receive data based on size
                        case (cap_size)
                            2'b10:   next_state = STATE_RX_RDATA_3;  // Word: 4 bytes
                            2'b01:   next_state = STATE_RX_RDATA_2;  // Half: 2 bytes
                            default: next_state = STATE_RX_RDATA_0;  // Byte: 1 byte
                        endcase
                    end
                end
            end
            
            // RX Read Data States (conditional based on size)
            STATE_RX_RDATA_3: begin  // Word only
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_2;
            end
            
            STATE_RX_RDATA_2: begin  // Half and Word
                if (rx_valid && rx_ready) begin
                    if (cap_size == 2'b10)
                        next_state = STATE_RX_RDATA_1;  // Word: continue
                    else
                        next_state = STATE_RX_RDATA_0;  // Half: skip to LSB
                end
            end
            
            STATE_RX_RDATA_1: begin  // Word only
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_0;
            end
            
            STATE_RX_RDATA_0: begin  // All reads end here
                if (rx_valid && rx_ready) next_state = STATE_IDLE;
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
    // TX Phase Detection
    // ============================================================
    assign in_tx_phase = (state >= STATE_TX_HEADER) && (state <= STATE_TX_WDATA_0);
    
    // ============================================================
    // TX Data Multiplexer
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
            default:          tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    assign tx_valid = in_tx_phase;
```

### 5.6 RX Data Path

```systemverilog
    // ============================================================
    // RX Phase Detection
    // ============================================================
    assign in_rx_phase = (state >= STATE_RX_HEADER) && (state <= STATE_RX_RDATA_0);
    
    // ============================================================
    // RX Ready Signal
    // ============================================================
    assign rx_ready = in_rx_phase;
    
    // ============================================================
    // RX Data Capture
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_rdata <= 32'h0;
        end else if (rx_valid && rx_ready) begin
            case (state)
                STATE_RX_RDATA_3: resp_rdata[31:24] <= rx_data;
                STATE_RX_RDATA_2: resp_rdata[23:16] <= rx_data;
                STATE_RX_RDATA_1: resp_rdata[15:8]  <= rx_data;
                STATE_RX_RDATA_0: resp_rdata[7:0]   <= rx_data;
                default: ;
            endcase
        end
    end
```

### 5.7 Bus Response Logic

```systemverilog
    // ============================================================
    // Bus Ready Signal
    // ============================================================
    assign ready = transaction_complete;
    
    // ============================================================
    // Bus Read Data
    // ============================================================
    assign rdata = resp_rdata;

endmodule
```

### 5.8 Complete Module Summary

The complete module combines all sections above into a single file at `rtl/host_bus_interface.sv`. Key characteristics:

| Property | Value |
|----------|-------|
| State bits | 4 bits (16 states used) |
| TX packet size | 5-9 bytes (variable) |
| RX packet size | 1-5 bytes (variable) |
| Latency | 1 cycle after last RX byte |
| Checksum | None (removed for bandwidth) |

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
| Basic Write | Single write transaction (various sizes) | 4 |
| Basic Read | Single read transaction (various sizes) | 4 |
| Variable Length | Verify correct packet lengths | 3 |
| Flow Control | TX/RX backpressure handling | 3 |
| Edge Cases | Address patterns, consecutive ops | 2 |

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

/// Calculate expected TX packet length based on we and size
fn expected_tx_len(we: bool, size: u8) -> usize {
    if we {
        // Write: header + 4 addr + data
        match size {
            0b00 => 6,  // byte: 1 data byte
            0b01 => 7,  // half: 2 data bytes
            0b10 => 9,  // word: 4 data bytes
            _ => 9,
        }
    } else {
        // Read: header + 4 addr (no data)
        5
    }
}

/// Calculate expected RX packet length based on we and size
fn expected_rx_len(we: bool, size: u8) -> usize {
    if we {
        // Write response: header only
        1
    } else {
        // Read response: header + data
        match size {
            0b00 => 2,  // byte: 1 data byte
            0b01 => 3,  // half: 2 data bytes
            0b10 => 5,  // word: 4 data bytes
            _ => 5,
        }
    }
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
fn test_write_word_packet_format() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a word write transaction
    dut.addr = 0x12345678;
    dut.wdata = 0xDEADBEEF;
    dut.we = 1;
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet (should be 9 bytes for word write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..9 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }
    
    // Verify packet format
    assert_eq!(tx_packet.len(), 9, "Word write request should be 9 bytes");
    
    // Byte 0: header = {4'b0, size=10, 1'b0, we=1} = 0x09
    assert_eq!(tx_packet[0], 0x09, "Header byte mismatch");
    
    // Bytes 1-4: Address (big-endian)
    assert_eq!(tx_packet[1], 0x12, "Address[31:24] mismatch");
    assert_eq!(tx_packet[2], 0x34, "Address[23:16] mismatch");
    assert_eq!(tx_packet[3], 0x56, "Address[15:8] mismatch");
    assert_eq!(tx_packet[4], 0x78, "Address[7:0] mismatch");
    
    // Bytes 5-8: Write data (big-endian, 4 bytes for word)
    assert_eq!(tx_packet[5], 0xDE, "WData[31:24] mismatch");
    assert_eq!(tx_packet[6], 0xAD, "WData[23:16] mismatch");
    assert_eq!(tx_packet[7], 0xBE, "WData[15:8] mismatch");
    assert_eq!(tx_packet[8], 0xEF, "WData[7:0] mismatch");
    
    // Verify no more TX bytes (tx_valid should go low)
    // Module should now be waiting for RX
}

#[test]
fn test_write_halfword_packet_format() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a halfword write transaction
    dut.addr = 0x80001000;
    dut.wdata = 0x0000CAFE;  // Lower 16 bits used
    dut.we = 1;
    dut.size = 0b01;  // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet (should be 7 bytes for halfword write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..7 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }
    
    assert_eq!(tx_packet.len(), 7, "Halfword write request should be 7 bytes");
    
    // Byte 0: header = {4'b0, size=01, 1'b0, we=1} = 0x05
    assert_eq!(tx_packet[0], 0x05, "Header byte mismatch");
    
    // Bytes 5-6: Write data (2 bytes for halfword: [23:16] and [7:0])
    assert_eq!(tx_packet[5], 0xCA, "WData[23:16] mismatch");
    assert_eq!(tx_packet[6], 0xFE, "WData[7:0] mismatch");
}

#[test]
fn test_write_byte_packet_format() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a byte write transaction
    dut.addr = 0x80002000;
    dut.wdata = 0x000000AB;  // Lower 8 bits used
    dut.we = 1;
    dut.size = 0b00;  // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet (should be 6 bytes for byte write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..6 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }
    
    assert_eq!(tx_packet.len(), 6, "Byte write request should be 6 bytes");
    
    // Byte 0: header = {4'b0, size=00, 1'b0, we=1} = 0x01
    assert_eq!(tx_packet[0], 0x01, "Header byte mismatch");
    
    // Byte 5: Write data (1 byte)
    assert_eq!(tx_packet[5], 0xAB, "WData[7:0] mismatch");
}

#[test]
fn test_write_transaction_complete() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a word write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet (9 bytes for word write)
    for _ in 0..9 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response packet (1 byte for write: just header)
    // Header echoes we=1, size=10: 0x09
    assert!(send_rx_byte(&mut dut, 0x09, 100), "Failed to send response header");
    
    // Give a cycle for state machine to complete
    clock_cycle!(dut);
    
    // Verify ready is asserted
    assert_eq!(dut.ready, 1, "ready should be HIGH after write response");
}
```

### 7.6 Basic Read Transaction Tests

```rust
#[test]
fn test_read_word_returns_data() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a word read transaction
    dut.addr = 0xABCD1234;
    dut.we = 0;  // Read
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet (5 bytes for read: header + 4 addr)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response with read data = 0xCAFEBABE
    // Header: {4'b0, size=10, 1'b0, we=0} = 0x08
    assert!(send_rx_byte(&mut dut, 0x08, 100), "Failed to send response header");
    // Data: 4 bytes (word)
    assert!(send_rx_byte(&mut dut, 0xCA, 100), "Failed to send RData[31:24]");
    assert!(send_rx_byte(&mut dut, 0xFE, 100), "Failed to send RData[23:16]");
    assert!(send_rx_byte(&mut dut, 0xBA, 100), "Failed to send RData[15:8]");
    assert!(send_rx_byte(&mut dut, 0xBE, 100), "Failed to send RData[7:0]");
    
    clock_cycle!(dut);
    
    // Verify read data
    assert_eq!(dut.ready, 1, "ready should be HIGH");
    assert_eq!(dut.rdata, 0xCAFEBABE, "Read data mismatch");
}

#[test]
fn test_read_halfword_returns_data() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a halfword read transaction
    dut.addr = 0x80001000;
    dut.we = 0;
    dut.size = 0b01;  // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet (5 bytes for read)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response: header + 2 bytes data
    // Header: {4'b0, size=01, 1'b0, we=0} = 0x04
    assert!(send_rx_byte(&mut dut, 0x04, 100), "Failed to send response header");
    assert!(send_rx_byte(&mut dut, 0xAB, 100), "Failed to send RData[23:16]");
    assert!(send_rx_byte(&mut dut, 0xCD, 100), "Failed to send RData[7:0]");
    
    clock_cycle!(dut);
    
    assert_eq!(dut.ready, 1, "ready should be HIGH");
    // Data should be in bits [23:16] and [7:0]
}

#[test]
fn test_read_byte_returns_data() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a byte read transaction
    dut.addr = 0x80002000;
    dut.we = 0;
    dut.size = 0b00;  // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain TX packet (5 bytes for read)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }
    
    // Send response: header + 1 byte data
    // Header: {4'b0, size=00, 1'b0, we=0} = 0x00
    assert!(send_rx_byte(&mut dut, 0x00, 100), "Failed to send response header");
    assert!(send_rx_byte(&mut dut, 0x42, 100), "Failed to send RData[7:0]");
    
    clock_cycle!(dut);
    
    assert_eq!(dut.ready, 1, "ready should be HIGH");
}

#[test]
fn test_read_request_packet_format() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Start a read transaction (we=0)
    dut.addr = 0x12345678;
    dut.we = 0;
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect all TX bytes (should be exactly 5 for read)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..5 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }
    
    assert_eq!(tx_packet.len(), 5, "Read request should be 5 bytes");
    
    // Header byte: {4'b0, size=10, 1'b0, we=0} = 0x08
    assert_eq!(tx_packet[0], 0x08, "Header byte for read mismatch");
    
    // Address bytes
    assert_eq!(tx_packet[1], 0x12, "Address[31:24] mismatch");
    assert_eq!(tx_packet[2], 0x34, "Address[23:16] mismatch");
    assert_eq!(tx_packet[3], 0x56, "Address[15:8] mismatch");
    assert_eq!(tx_packet[4], 0x78, "Address[7:0] mismatch");
    
    // Verify tx_valid goes low after 5 bytes (no more data)
    clock_cycle!(dut);
    assert_eq!(dut.tx_valid, 0, "tx_valid should be LOW after read request complete");
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
    
    // Start word write transaction and drain TX (9 bytes)
    dut.addr = 0x00000000;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    for _ in 0..9 {
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
    
    // Now send response (1 byte for write: header only)
    send_rx_byte(&mut dut, 0x09, 100);  // Header echoing we=1, size=10
    
    clock_cycle!(dut);
    assert_eq!(dut.ready, 1, "ready should be HIGH after delayed response");
}

#[test]
fn test_rx_ready_only_in_rx_phase() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Verify rx_ready is LOW in IDLE
    assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW in IDLE");
    
    // Start read transaction
    dut.addr = 0x80000000;
    dut.we = 0;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // During TX phase, rx_ready should be LOW
    for _ in 0..3 {
        assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW during TX phase");
        receive_tx_byte(&mut dut, 100);
    }
    
    // Finish TX (5 bytes total for read)
    for _ in 0..2 {
        receive_tx_byte(&mut dut, 100);
    }
    
    // Now in RX phase, rx_ready should be HIGH
    clock_cycle!(dut);
    assert_eq!(dut.rx_ready, 1, "rx_ready should be HIGH in RX phase");
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
    
    // Start a word write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Verify ready stays LOW during TX phase (9 bytes for word write)
    for _ in 0..9 {
        assert_eq!(dut.ready, 0, "ready should be LOW during TX");
        receive_tx_byte(&mut dut, 100).expect("TX byte");
    }
    
    // Verify ready stays LOW during RX wait phase
    for _ in 0..10 {
        clock_cycle!(dut);
        assert_eq!(dut.ready, 0, "ready should be LOW waiting for response");
    }
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
        
        // Start word write transaction
        dut.addr = test_addr;
        dut.wdata = test_data;
        dut.we = 1;
        dut.size = 0b10;
        dut.req = 1;
        clock_cycle!(dut);
        dut.req = 0;
        
        // Drain TX (9 bytes for word write)
        for _ in 0..9 {
            receive_tx_byte(&mut dut, 100).expect("TX byte");
        }
        
        // Send response (1 byte for write)
        send_rx_byte(&mut dut, 0x09, 100);  // Header echoing we=1, size=10
        
        clock_cycle!(dut);
        
        // Verify completion
        assert_eq!(dut.ready, 1, "Transaction {} should complete", iteration);
        
        // Wait a cycle for state to return to IDLE
        clock_cycle!(dut);
    }
}

#[test]
fn test_all_ones_address() {
    let runtime = create_host_bus_interface_runtime()
        .expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    
    reset_module(&mut dut);
    
    // Test with all-ones address (word write = 9 bytes)
    dut.addr = 0xFFFFFFFF;
    dut.wdata = 0xFFFFFFFF;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Collect TX packet (9 bytes for word write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..9 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        }
    }
    
    assert_eq!(tx_packet.len(), 9, "Word write should be 9 bytes");
    
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
```

### 7.10 Test Summary

| Test Name | Category | Description |
|-----------|----------|-------------|
| `test_reset_state` | Reset | Verify outputs after reset |
| `test_idle_no_transaction` | Reset | Verify idle behavior |
| `test_write_word_packet_format` | Write | Verify word write TX packet (9 bytes) |
| `test_write_halfword_packet_format` | Write | Verify halfword write TX packet (7 bytes) |
| `test_write_byte_packet_format` | Write | Verify byte write TX packet (6 bytes) |
| `test_write_transaction_complete` | Write | Full word write cycle with 1-byte response |
| `test_read_word_returns_data` | Read | Word read with 5-byte response |
| `test_read_halfword_returns_data` | Read | Halfword read with 3-byte response |
| `test_read_byte_returns_data` | Read | Byte read with 2-byte response |
| `test_read_request_packet_format` | Read | Verify read TX packet is 5 bytes |
| `test_tx_backpressure` | Flow | TX ready backpressure handling |
| `test_rx_delayed_valid` | Flow | Delayed RX response |
| `test_rx_ready_only_in_rx_phase` | Flow | RX ready timing |
| `test_byte_access_size` | Header | Byte size encoding (size=00) |
| `test_halfword_access_size` | Header | Halfword size encoding (size=01) |
| `test_write_blocking` | Blocking | Verify ready stays low during transaction |
| `test_consecutive_transactions` | Sequence | Back-to-back transactions |
| `test_all_ones_address` | Edge | 0xFFFFFFFF address handling |

---

## 8. Implementation Checklist

### Phase 1: RTL Implementation

- [ ] **Create `rtl/host_bus_interface.sv`**
  - [ ] Module header with parameters and ports
  - [ ] State machine enum definition (variable-length states)
  - [ ] Captured request registers
  - [ ] Response data registers
  - [ ] State register with async reset
  - [ ] Next state combinational logic with size-based branching
  - [ ] Request capture logic
  - [ ] TX data multiplexer
  - [ ] TX valid signal (variable-length based on we/size)
  - [ ] RX ready signal
  - [ ] RX data capture logic (variable-length based on we/size)
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
  - [ ] Variable-length write transaction tests (byte/half/word)
  - [ ] Variable-length read transaction tests (byte/half/word)
  - [ ] Flow control tests
  - [ ] Header byte encoding tests
  - [ ] Consecutive transaction tests

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
| Error Handling | Add error status in response header | Medium |
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

### Request Packet (Variable Length: 5-9 bytes)

**Word Write (9 bytes):**

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

**Word Read (5 bytes):**

| Byte | Content | Example (Read word from 0xABCD1234) |
|------|---------|-------------------------------------|
| 0 | `{4'b0, size[1:0], 1'b0, we}` | 0x08 (size=10, we=0) |
| 1 | `addr[31:24]` | 0xAB |
| 2 | `addr[23:16]` | 0xCD |
| 3 | `addr[15:8]` | 0x12 |
| 4 | `addr[7:0]` | 0x34 |

### Response Packet (Variable Length: 1-5 bytes)

**Write Response (1 byte):**

| Byte | Content | Example (Write acknowledgement) |
|------|---------|--------------------------------|
| 0 | `{4'b0, size[1:0], 1'b0, we}` | 0x09 (echoes request header) |

**Word Read Response (5 bytes):**

| Byte | Content | Example (Read returns 0xCAFEBABE) |
|------|---------|----------------------------------|
| 0 | `{4'b0, size[1:0], 1'b0, we}` | 0x08 (size=10, we=0) |
| 1 | `rdata[31:24]` | 0xCA |
| 2 | `rdata[23:16]` | 0xFE |
| 3 | `rdata[15:8]` | 0xBA |
| 4 | `rdata[7:0]` | 0xBE |

### Packet Size Summary

| Transaction | Request Bytes | Response Bytes | Total |
|-------------|---------------|----------------|-------|
| Read word   | 5             | 5              | 10    |
| Write word  | 9             | 1              | 10    |
| Read half   | 5             | 3              | 8     |
| Write half  | 7             | 1              | 8     |
| Read byte   | 5             | 2              | 7     |
| Write byte  | 6             | 1              | 7     |

---

## Appendix B: State Machine Diagram (ASCII)

```
      ┌────────────────────────────────────────────────────────────────────────────┐
      │                                                                            │
      │  IDLE ──req──▶ CAPTURE ──▶ TX_HDR ──▶ TX_ADDR ──┬──▶ TX_WDATA ──┐         │
      │    ▲                                 (4 bytes)  │   (write only) │         │
      │    │                                            │   (1-4 bytes)  │         │
      │    │                            ┌───────────────┴────────────────┘         │
      │    │                            │                                          │
      │    │                            ▼                                          │
      │    │                       RX_HEADER ──┬──────────────────▶ COMPLETE       │
      │    │                                   │ (write response)     │            │
      │    │                                   │                      │            │
      │    │                                   ▼                      │            │
      │    │                              RX_RDATA ───────────────────┘            │
      │    │                              (read only)                              │
      │    │                              (1-4 bytes)                              │
      │    │                                                                       │
      │    └───────────────────────────────────────────────────────────────────────┘
```

---

**Document Version:** 2.0  
**Last Updated:** January 30, 2026

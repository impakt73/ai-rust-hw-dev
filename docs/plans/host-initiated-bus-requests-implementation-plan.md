# Host-Initiated Bus Requests Implementation Plan

## Bi-Directional Host Bus Communication with Multi-Master Arbitration

**Author:** GitHub Copilot Hardware-Software Integration Agent  
**Date:** February 1, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Prerequisites:** Existing `host_bus_interface.sv` implementation (CPU-initiated transactions only)  
**Status:** Planning

---

## Executive Summary

This document provides a detailed technical implementation plan for **upgrading the host bus communication system** to support **host-initiated bus requests**. The current system only supports CPU-initiated transactions (CPU → Host); this upgrade adds the reverse direction (Host → CPU/RTL peripherals) while maintaining system stability and preventing bus hangs.

### Key Features

1. **Bi-Directional Communication:** Support both CPU→Host and Host→CPU transactions
2. **Multi-Master Arbitration:** Priority arbiter with Host having priority over CPU
3. **Single-Pending Transaction Rule:** Both sides maintain one transaction in flight
4. **Address Range Validation:** Prevent routing loops (Host→Host, CPU→CPU)
5. **Hang Prevention:** Comprehensive edge case handling with timeouts
6. **Comprehensive Testing:** RTL testbench tests and cpu-sim integration tests

### Design Philosophy

The system follows a **simple, reliable** design:
- **One pending transaction at a time** on each side (Host side and Target/FPGA side)
- **Priority arbitration** with Host taking precedence over CPU
- **Address range validation** to prevent routing loops
- **Clear error reporting** for invalid transactions

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Protocol Design](#2-protocol-design)
3. [Bus Arbitration Design](#3-bus-arbitration-design)
4. [Edge Cases and Hang Prevention](#4-edge-cases-and-hang-prevention)
5. [RTL Implementation](#5-rtl-implementation)
6. [Rust Integration Layer](#6-rust-integration-layer)
7. [Testing Strategy](#7-testing-strategy)
8. [Implementation Checklist](#8-implementation-checklist)

---

## 1. Architecture Overview

### 1.1 Current Architecture (CPU-Initiated Only)

```
┌─────────────┐          ┌─────────────────────┐          ┌──────────────────┐
│             │          │                     │          │                  │
│    CPU      │◀────────▶│     System Bus      │◀────────▶│ RTL Peripherals  │
│             │          │   (Single Master)   │          │ (LED, Clock,     │
│             │          │                     │          │  UART)           │
└─────────────┘          └──────────┬──────────┘          └──────────────────┘
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │ host_bus_interface  │
                         │ (TX: CPU→Host)      │◀───────▶ Host (Rust Simulator)
                         │ (RX: Host→CPU)      │
                         └─────────────────────┘
```

**Current Limitations:**
- Only CPU can initiate bus transactions to the host
- Host can only respond to CPU requests
- Host cannot directly access RTL peripherals (LED, Clock, UART)

### 1.2 Proposed Architecture (Bi-Directional)

```
┌─────────────┐          ┌─────────────────────┐          ┌──────────────────┐
│             │          │                     │          │                  │
│    CPU      │◀────────▶│   Priority Arbiter  │◀────────▶│ RTL Peripherals  │
│             │          │   + System Bus      │          │ (LED, Clock,     │
│             │          │   (Multi-Master)    │          │  UART)           │
└─────────────┘          └──────────┬──────────┘          └──────────────────┘
       │                            │                              ▲
       │                            │                              │
       │                  ┌─────────┴─────────┐                    │
       │                  │                   │                    │
       │                  ▼                   ▼                    │
       │        ┌─────────────────┐  ┌─────────────────┐          │
       │        │ CPU→Host Path   │  │ Host→RTL Path   │          │
       │        │ (External Mem)  │  │ (RTL Periph)    │◀─────────┘
       │        └────────┬────────┘  └────────┬────────┘
       │                 │                    │
       │                 ▼                    ▼
       │        ┌─────────────────────────────────────┐
       │        │      host_bus_interface             │
       │        │  (Upgraded: Bi-Directional)         │
       │        │                                     │
       │        │  TX: CPU→Host requests/responses    │◀────────┐
       │        │  RX: Host→CPU requests/responses    │         │
       └───────▶│                                     │◀────────┤
                └─────────────────────────────────────┘         │
                                    ▲                           │
                                    │                           │
                                    ▼                           ▼
                         ┌─────────────────────────────────────────┐
                         │            Host (Rust Simulator)        │
                         │                                         │
                         │  - send_bus_request() / recv_response() │
                         │  - Handles CPU external memory requests │
                         │  - DRAM, Video, Audio, FIFO, SimControl │
                         └─────────────────────────────────────────┘
```

### 1.3 Address Space Ownership

| Address Range | Owner | Description |
|---------------|-------|-------------|
| `0x10000000 - 0x100000FF` | Host (Rust) | SimControl |
| `0x20000000 - 0x2000000F` | Host (Rust) | Video |
| `0x30000000 - 0x3000000F` | Host (Rust) | Audio |
| `0x40000000 - 0x40000007` | Host (Rust) | FIFO |
| `0x50000000 - 0x5FFFFFFF` | RTL (FPGA) | LED, Clock, UART peripherals |
| `0x80000000 - 0xFFFFFFFF` | Host (Rust) | DRAM |

**Note:** The regions between Host peripheral ranges (e.g., 0x10000100 to 0x1FFFFFFF) are currently reserved/unassigned.

**Routing Rules:**
- **CPU requests to `0x50000000 - 0x5FFFFFFF`:** Handled locally by RTL bus (no serialization)
- **CPU requests to other addresses:** Serialized to Host via host_bus_interface
- **Host requests to `0x50000000 - 0x5FFFFFFF`:** Processed by RTL bus (valid)
- **Host requests to other addresses:** **ERROR** - would route back to Host (loop)

---

## 2. Protocol Design

### 2.1 Existing Protocol (CPU → Host, unchanged)

```
Direction: FPGA → Host (TX channel)
Read Request:   [header][addr0][addr1][addr2][addr3]              (5 bytes)
Write Request:  [header][addr0][addr1][addr2][addr3][data...]     (6-9 bytes)

Direction: Host → FPGA (RX channel)
Write Response: [ack]                                             (1 byte, 0x00)
Read Response:  [data...]                                         (1-4 bytes)

Header format: {4'b0000, size[1:0], 1'b0, we}
```

### 2.2 New Protocol (Host → FPGA Requests)

The Host can now **initiate** bus requests to the FPGA side. We use **the same packet format** but in the reverse direction on different logical channels.

```
Direction: Host → FPGA (RX channel) - NEW: Host-Initiated Requests
Read Request:   [header][addr0][addr1][addr2][addr3]              (5 bytes)
Write Request:  [header][addr0][addr1][addr2][addr3][data...]     (6-9 bytes)

Direction: FPGA → Host (TX channel) - NEW: FPGA Responses
Write Response: [ack]                                             (1 byte, 0x00)
Read Response:  [data...]                                         (1-4 bytes)
Error Response: [error_code]                                      (1 byte, 0xFF = address error)

Header format: {4'b0000, size[1:0], 1'b0, we}
```

### 2.3 Channel Multiplexing

The existing TX/RX channels are **shared** for both directions. A **direction bit** in an extended header byte distinguishes request vs response packets.

**Extended Header Format (1 byte):**
```
Bits [7:4]: Packet type
  0000 = CPU-initiated request (CPU → Host, existing)
  0001 = Host response to CPU request (Host → FPGA, existing)
  0010 = Host-initiated request (Host → FPGA, NEW)
  0011 = FPGA response to Host request (FPGA → Host, NEW)
  1111 = Error response

Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
Bit  [1]:   Reserved (0)
Bit  [0]:   we (1=write, 0=read)
```

**Alternative Approach (Context-Based Detection) - NOT RECOMMENDED:**

> **⚠️ WARNING:** This approach is **NOT VIABLE** due to the simultaneous request edge case described below.

An alternative would be to use **context** to determine packet meaning:
- The Host knows when it's sending a request vs responding
- The FPGA knows when it's sending a request vs responding
- A simple state machine on each side tracks whether it expects a request or response

**Why This Doesn't Work:** When both sides send a request simultaneously, the receiving side cannot distinguish whether incoming data is:
- A new request from the other side, OR
- A response to its own outstanding request

The extended header format with explicit packet type bits (0010 for host-initiated requests) is **mandatory** to resolve this ambiguity.

### 2.4 Simultaneous Request Handling (Critical Edge Case)

**Problem:** When both Host and FPGA send requests at the same time:
1. FPGA sends CPU-initiated request (packet type 0000) to Host
2. Host sends Host-initiated request (packet type 0010) to FPGA
3. Both sides are now waiting for a response, but receiving a request instead

**Solution:** Each side must be capable of **buffering one incoming request** while waiting for a response to its outstanding request:

1. **Dual-Buffer Requirement:** Both sides must maintain:
   - A pending **outgoing request** waiting for response
   - A buffered **incoming request** that arrived while waiting

2. **Processing Order Rule:** When a side has an outstanding request:
   - It MUST **read and buffer** any incoming request (identified by packet type bits)
   - It MUST **NOT process** the buffered request until its own response arrives
   - After receiving the response, it processes the buffered request

3. **Why This Prevents Deadlock:**
   - Each side can only have ONE outstanding request at a time (existing rule)
   - Therefore, at most ONE request can be buffered on each side
   - Response to an outstanding request is never blocked by an unprocessed incoming request
   - After the response arrives, the buffered request is processed

**FPGA-Side Constraint:**
When the FPGA has an outstanding CPU-initiated request:
- The bus is occupied by the CPU
- An incoming Host request CANNOT be processed (arbiter blocks it)
- The Host request MUST be buffered until the CPU transaction completes
- Once the CPU transaction completes, the buffered Host request gains bus access

**Protocol Flow (Simultaneous Requests):**

```
Time    FPGA Side                          Host Side
────────────────────────────────────────────────────────────────────
T1      Send CPU request (type 0000)       Send Host request (type 0010)
T2      Receive Host request → BUFFER      Receive CPU request → BUFFER
        (cannot process: bus busy)         (must wait for own response)
T3      Waiting for CPU response...        Process buffered CPU request
T4      Receive CPU response               Send CPU response
        Process buffered Host request      Waiting for Host response...
T5      Send Host response                 Receive Host response
T6      IDLE                               IDLE
```

### 2.5 Transaction State Machines

**Host Side State Machine:**

```
                    ┌───────────────────┐
                    │                   │
         ┌─────────▶│       IDLE        │◀─────────┐
         │          │                   │          │
         │          └─────────┬─────────┘          │
         │                    │                    │
         │        ┌───────────┴───────────┐        │
         │        │                       │        │
         │        ▼                       ▼        │
    ┌────┴────────────┐           ┌────────────────┴────┐
    │ PROCESSING_CPU  │           │ SENDING_HOST_REQ    │
    │ REQUEST         │           │                     │
    │ (recv CPU req,  │           │ (send host request, │
    │  access bus,    │           │  wait for response) │
    │  send response) │           │                     │
    └─────────────────┘           └─────────────────────┘
```

**FPGA Side State Machine (host_bus_interface):**

```
                    ┌───────────────────┐
                    │                   │
         ┌─────────▶│       IDLE        │◀─────────┐
         │          │                   │          │
         │          └─────────┬─────────┘          │
         │                    │                    │
         │        ┌───────────┴───────────┐        │
         │        │                       │        │
         │        ▼                       ▼        │
    ┌────┴────────────┐           ┌────────────────┴────┐
    │ TX_CPU_REQUEST  │           │ RX_HOST_REQUEST     │
    │                 │           │                     │
    │ (serialize CPU  │           │ (receive host req,  │
    │  request, wait  │           │  access RTL bus,    │
    │  for response)  │           │  send response)     │
    └─────────────────┘           └─────────────────────┘
```

### 2.6 Single Pending Transaction Rule

**Critical Design Decision:** To keep complexity low and prevent ordering issues:

1. **Host Side:** Can have at most ONE **outstanding request** at a time
   - May send ONE host-initiated request and wait for response
   - While waiting, MUST buffer any incoming CPU-initiated request
   - After receiving response, processes the buffered request (if any)

2. **FPGA Side:** Can have at most ONE **outstanding request** at a time
   - May send ONE CPU-initiated request and wait for response
   - While waiting, MUST buffer any incoming Host-initiated request (identified by packet type 0010)
   - After receiving response, processes the buffered request (if any)

**Implementation:**
- **Host (Rust):** Tracks pending state using the existing `HostBusHostState` enum (state machine) and the `current_host_request: Option<HostBusRequest>` field. When `current_host_request` is `Some` and state is waiting for response, the host has an outstanding request.
- **FPGA (`host_bus_interface`):** Maintains current active transaction direction in the FSM state, plus adds an optional buffered host request register for the simultaneous request scenario.
- Before sending a new request, each side checks its current state to determine if it's already waiting for a response.

**Key Invariants:**
- At most ONE outstanding request per side
- At most ONE buffered incoming request per side
- Responses are never blocked (always processed immediately)
- Buffered requests are processed in FIFO order after response received

---

## 3. Bus Arbitration Design

### 3.1 Multi-Master Requirements

With Host-initiated requests, the system becomes **multi-master**:
- **Master 1:** CPU (existing)
- **Master 2:** Host (via host_bus_interface, new)

Both masters may want to access the RTL bus simultaneously. We need an arbiter.

### 3.2 Priority Arbiter Design

**Design Choice:** Simple **fixed-priority arbiter** with Host having higher priority.

**Rationale:**
- Host requests are typically infrequent (test/debug operations)
- When Host does request, it's often time-sensitive (simulation control)
- CPU can wait briefly; Host gives it back quickly

**Arbiter Logic:**

```systemverilog
// Priority Arbiter: Host > CPU
// host_req has priority over cpu_req

always_comb begin
    // Default: no grant
    cpu_grant = 1'b0;
    host_grant = 1'b0;
    
    if (host_req && !host_active_txn) begin
        // Host has priority
        host_grant = 1'b1;
    end else if (cpu_req && !cpu_active_txn) begin
        // CPU gets bus if Host isn't requesting
        cpu_grant = 1'b1;
    end
end
```

### 3.3 Bus Arbitration Module

**New Module: `bus_arbiter.sv`**

```systemverilog
module bus_arbiter (
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU Master Interface
    input  logic [31:0] cpu_addr,
    input  logic [31:0] cpu_wdata,
    output logic [31:0] cpu_rdata,
    input  logic        cpu_we,
    input  logic [1:0]  cpu_size,
    input  logic        cpu_req,
    output logic        cpu_ready,
    
    // Host Master Interface (from host_bus_interface)
    input  logic [31:0] host_addr,
    input  logic [31:0] host_wdata,
    output logic [31:0] host_rdata,
    input  logic        host_we,
    input  logic [1:0]  host_size,
    input  logic        host_req,
    output logic        host_ready,
    
    // Slave Interface (to bus.sv)
    output logic [31:0] bus_addr,
    output logic [31:0] bus_wdata,
    input  logic [31:0] bus_rdata,
    output logic        bus_we,
    output logic [1:0]  bus_size,
    output logic        bus_req,
    input  logic        bus_ready
);
```

### 3.4 Integration with Existing Bus

**Modified `top.sv` Hierarchy:**

```
┌────────────────────────────────────────────────────────────────────┐
│                            top.sv                                  │
│                                                                    │
│  ┌──────────────┐    ┌──────────────────┐    ┌─────────────────┐  │
│  │              │    │                  │    │                 │  │
│  │     CPU      │───▶│   bus_arbiter    │───▶│    bus.sv       │  │
│  │              │    │   (priority)     │    │ (address decode)│  │
│  └──────────────┘    │                  │    │                 │  │
│                      │   Host────▶      │    └────────┬────────┘  │
│                      └────────▲─────────┘             │           │
│                               │                       ▼           │
│                      ┌────────┴─────────┐    ┌─────────────────┐  │
│                      │ host_bus_if      │    │ RTL Peripherals │  │
│                      │ (upgraded)       │◀──▶│ LED, Clock,     │  │
│                      └──────────────────┘    │ UART            │  │
│                               ▲              └─────────────────┘  │
│                               │                                   │
└───────────────────────────────┼───────────────────────────────────┘
                                │
                                ▼
                    Host TX/RX (Rust Simulator)
```

---

## 4. Edge Cases and Hang Prevention

### 4.1 Potential Hang Scenarios

| Scenario | Description | Cause | Prevention |
|----------|-------------|-------|------------|
| **Host→Host Loop** | Host sends request to address owned by Host | Address range error | Validate address before sending |
| **CPU→CPU Loop** | CPU sends request to RTL peripheral | Should work (local) | N/A - this is valid |
| **Simultaneous Requests** | Both CPU and Host request at same time | Both sides waiting for response while receiving request | Packet type header distinguishes request from response; buffer incoming request until own response received (see Section 2.4) |
| **Response Mismatch** | Response received when not expected | Protocol error | State machine validation + packet type bits |
| **TX Backpressure Deadlock** | TX full while waiting for RX | Buffer exhaustion | Separate TX/RX handling |
| **Host Disappears** | Host stops responding mid-transaction | Software crash | Optional timeout mechanism |
| **Bus Contention on Buffered Request** | FPGA receives Host request while CPU transaction active | Bus arbiter blocking | Buffer Host request, process after CPU transaction completes |

### 4.2 Address Validation (Preventing Routing Loops)

**Critical Rule:** Transactions must not route back to their origin.

**Host-Initiated Request Validation:**

```rust
// In Rust simulator: validate before sending
fn validate_host_request_address(addr: u32) -> Result<(), BusError> {
    // Host-initiated requests MUST target RTL peripheral space
    // They CANNOT target Host-owned addresses (would route back)
    if addr >= RTL_PERIPH_BASE && addr < RTL_PERIPH_LIMIT {
        Ok(())  // Valid: targeting RTL peripherals
    } else {
        Err(BusError::InvalidAddressRange {
            addr,
            reason: "Host-initiated request targets non-RTL address (would loop back to host)"
        })
    }
}
```

**CPU-Initiated Request Validation:**

```systemverilog
// In host_bus_interface: validate before processing CPU request
// CPU requests to RTL peripheral range should NOT reach host_bus_interface
// (they should be handled locally by bus.sv)
// If they do reach here, it's an error in bus.sv routing

// In bus.sv: ensure RTL peripheral requests don't reach ext_mem port
// This is already implemented correctly - RTL peripherals are local
```

### 4.3 State Machine Error Recovery

**FPGA Side (`host_bus_interface.sv`):**

```systemverilog
// Error states for recovery
typedef enum logic [2:0] {
    ERR_NONE           = 3'd0,
    ERR_INVALID_ADDR   = 3'd1,  // Host request to invalid address
    ERR_TIMEOUT        = 3'd2,  // Optional: response timeout
    ERR_PROTOCOL       = 3'd3   // Unexpected packet received
} error_t;

// When error detected:
// 1. Send error response to Host
// 2. Return to IDLE state
// 3. Log error for debugging (via $display in simulation)
```

**Host Side (Rust Simulator):**

```rust
/// Error codes from FPGA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaError {
    InvalidAddress = 0xFF,
    Timeout = 0xFE,
    ProtocolError = 0xFD,
}

/// Handle error response from FPGA
fn handle_error_response(&mut self, error_code: u8) -> Result<(), String> {
    let error = match error_code {
        0xFF => FpgaError::InvalidAddress,
        0xFE => FpgaError::Timeout,
        _ => FpgaError::ProtocolError,
    };
    Err(format!("FPGA returned error: {:?}", error))
}
```

### 4.4 Optional Timeout Mechanism

**Design Decision:** Timeouts are **optional** for initial implementation.

**Rationale:**
- In simulation, Host is always responsive (Rust code)
- Timeouts add complexity and potential false positives
- Can be added later if needed for FPGA deployment

**If Implemented:**

```systemverilog
// In host_bus_interface.sv
parameter TIMEOUT_CYCLES = 32'd1_000_000;  // ~20ms at 50MHz

logic [31:0] timeout_counter;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        timeout_counter <= '0;
    end else if (state == WAITING_FOR_RESPONSE) begin
        if (timeout_counter >= TIMEOUT_CYCLES) begin
            // Timeout! Return error response
            next_state <= ERROR_RECOVERY;
        end else begin
            timeout_counter <= timeout_counter + 1;
        end
    end else begin
        timeout_counter <= '0;
    end
end
```

### 4.5 Backpressure Handling

**TX Backpressure:**
- Host TX buffer full → TX ready goes low
- FPGA holds data, waits for ready
- State machine stays in current state until handshake completes

**RX Backpressure:**
- FPGA RX buffer full → RX ready goes low
- Host holds data, waits for ready
- No deadlock because TX/RX are independent channels

**Prevention of TX/RX Deadlock:**
- TX and RX are handled by independent state machines
- Never wait for TX space when processing RX (or vice versa)
- Each direction has its own backpressure handling

---

## 5. RTL Implementation

### 5.1 Modified Files

| File | Changes |
|------|---------|
| `rtl/bus_arbiter.sv` | **NEW:** Priority arbiter module |
| `rtl/host_bus_interface.sv` | **MODIFIED:** Add Host→FPGA request handling |
| `rtl/bus.sv` | **MODIFIED:** Minor interface changes for arbiter |
| `rtl/top.sv` | **MODIFIED:** Instantiate arbiter, wire up Host master |

### 5.2 New Module: `bus_arbiter.sv`

```systemverilog
// Bus Arbiter Module
// Implements fixed-priority arbitration between CPU and Host masters
// Priority: Host > CPU
//
// Features:
// - Registered outputs for timing closure
// - Hold grant until transaction completes (ready asserted)
// - No combinational loops

module bus_arbiter (
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU Master Interface
    input  logic [31:0] cpu_addr,
    input  logic [31:0] cpu_wdata,
    output logic [31:0] cpu_rdata,
    input  logic        cpu_we,
    input  logic [1:0]  cpu_size,
    input  logic        cpu_req,
    output logic        cpu_ready,
    
    // Host Master Interface (from host_bus_interface)
    input  logic [31:0] host_addr,
    input  logic [31:0] host_wdata,
    output logic [31:0] host_rdata,
    input  logic        host_we,
    input  logic [1:0]  host_size,
    input  logic        host_req,
    output logic        host_ready,
    
    // Slave Interface (to bus.sv)
    output logic [31:0] bus_addr,
    output logic [31:0] bus_wdata,
    input  logic [31:0] bus_rdata,
    output logic        bus_we,
    output logic [1:0]  bus_size,
    output logic        bus_req,
    input  logic        bus_ready
);

    // ============================================================
    // Arbiter State
    // ============================================================
    typedef enum logic [1:0] {
        ARB_IDLE      = 2'd0,
        ARB_CPU_GRANT = 2'd1,
        ARB_HOST_GRANT = 2'd2
    } arb_state_t;
    
    arb_state_t state, next_state;
    
    // ============================================================
    // State Machine
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= ARB_IDLE;
        end else begin
            state <= next_state;
        end
    end
    
    always_comb begin
        next_state = state;
        
        case (state)
            ARB_IDLE: begin
                // Priority: Host > CPU
                if (host_req) begin
                    next_state = ARB_HOST_GRANT;
                end else if (cpu_req) begin
                    next_state = ARB_CPU_GRANT;
                end
            end
            
            ARB_CPU_GRANT: begin
                if (bus_ready) begin
                    // Transaction complete
                    // Check if host is waiting (preempt for next transaction)
                    if (host_req) begin
                        next_state = ARB_HOST_GRANT;
                    end else if (!cpu_req) begin
                        next_state = ARB_IDLE;
                    end
                    // else stay in CPU_GRANT for consecutive CPU transactions
                end
            end
            
            ARB_HOST_GRANT: begin
                if (bus_ready) begin
                    // Transaction complete
                    if (host_req) begin
                        // Host has more requests
                        next_state = ARB_HOST_GRANT;
                    end else if (cpu_req) begin
                        next_state = ARB_CPU_GRANT;
                    end else begin
                        next_state = ARB_IDLE;
                    end
                end
            end
            
            default: next_state = ARB_IDLE;
        endcase
    end
    
    // ============================================================
    // Multiplexer Logic
    // ============================================================
    always_comb begin
        // Default: idle
        bus_addr  = '0;
        bus_wdata = '0;
        bus_we    = 1'b0;
        bus_size  = 2'b00;
        bus_req   = 1'b0;
        
        cpu_rdata  = '0;
        cpu_ready  = 1'b0;
        host_rdata = '0;
        host_ready = 1'b0;
        
        case (state)
            ARB_CPU_GRANT: begin
                bus_addr  = cpu_addr;
                bus_wdata = cpu_wdata;
                bus_we    = cpu_we;
                bus_size  = cpu_size;
                bus_req   = cpu_req;
                cpu_rdata = bus_rdata;
                cpu_ready = bus_ready;
            end
            
            ARB_HOST_GRANT: begin
                bus_addr  = host_addr;
                bus_wdata = host_wdata;
                bus_we    = host_we;
                bus_size  = host_size;
                bus_req   = host_req;
                host_rdata = bus_rdata;
                host_ready = bus_ready;
            end
            
            default: ;
        endcase
    end

endmodule
```

### 5.3 Modified `host_bus_interface.sv`

Add new states and logic for Host→FPGA request processing:

```systemverilog
// Extended State Machine (additions shown)
typedef enum logic [5:0] {
    // Existing states for CPU→Host (unchanged)
    STATE_IDLE        = 6'd0,
    STATE_CAPTURE     = 6'd1,
    STATE_TX_HEADER   = 6'd2,
    // ... existing TX states ...
    STATE_RX_ACK      = 6'd11,
    // ... existing RX states ...
    STATE_COMPLETE    = 6'd16,
    
    // NEW: States for Host→FPGA requests
    STATE_HOST_RX_HEADER    = 6'd20,  // Receive host request header
    STATE_HOST_RX_ADDR_0    = 6'd21,  // Receive address bytes
    STATE_HOST_RX_ADDR_1    = 6'd22,
    STATE_HOST_RX_ADDR_2    = 6'd23,
    STATE_HOST_RX_ADDR_3    = 6'd24,
    STATE_HOST_RX_WDATA_0   = 6'd25,  // Receive write data bytes
    STATE_HOST_RX_WDATA_1   = 6'd26,
    STATE_HOST_RX_WDATA_2   = 6'd27,
    STATE_HOST_RX_WDATA_3   = 6'd28,
    STATE_HOST_BUS_REQ      = 6'd29,  // Issue request to bus arbiter
    STATE_HOST_BUS_WAIT     = 6'd30,  // Wait for bus response
    STATE_HOST_TX_ACK       = 6'd31,  // Send write ack to host
    STATE_HOST_TX_RDATA_0   = 6'd32,  // Send read data to host
    STATE_HOST_TX_RDATA_1   = 6'd33,
    STATE_HOST_TX_RDATA_2   = 6'd34,
    STATE_HOST_TX_RDATA_3   = 6'd35,
    STATE_HOST_COMPLETE     = 6'd36,
    STATE_HOST_ERROR        = 6'd37   // Error handling
} state_t;

// New ports for Host→FPGA requests
// Bus Master Interface (to Arbiter)
output logic [31:0] host_bus_addr,
output logic [31:0] host_bus_wdata,
input  logic [31:0] host_bus_rdata,
output logic        host_bus_we,
output logic [1:0]  host_bus_size,
output logic        host_bus_req,
input  logic        host_bus_ready,

// New registers for host-initiated transactions
logic [31:0] host_cap_addr;
logic [31:0] host_cap_wdata;
logic        host_cap_we;
logic [1:0]  host_cap_size;
logic [31:0] host_resp_rdata;
```

### 5.4 Priority in IDLE State

```systemverilog
// In IDLE state, check for both CPU requests and Host requests
// Host-initiated requests have priority (checked first on RX)
STATE_IDLE: begin
    // First: Check if Host is sending a request (RX has valid data)
    if (rx_valid && is_host_initiated_request_header(rx_data)) begin
        // Host-initiated request incoming (packet type 0010)
        host_cap_we   <= (rx_data & 8'h01) != 0;
        host_cap_size <= (rx_data >> 2) & 2'b11;
        next_state <= STATE_HOST_RX_ADDR_0;
    end
    // Second: Check if CPU is requesting (existing behavior)
    else if (req) begin
        next_state <= STATE_CAPTURE;
    end
end

// Helper function to identify host-initiated request header
// Host-initiated request headers use packet type 0010 in bits [7:4],
// per the Extended Header Format specification. This distinguishes them from:
// - CPU-initiated requests (type 0000, sent by FPGA, not received)
// - Host responses to CPU requests (type 0001)
// - FPGA responses to Host requests (type 0011)
function logic is_host_initiated_request_header(input logic [7:0] data);
    // Check for packet type 0010 in the upper nibble
    return (data[7:4] == 4'b0010);
endfunction
```

### 5.5 STATE_HOST_ERROR Implementation

```systemverilog
// Error handling state - sends error response to host
STATE_HOST_ERROR: begin
    // Send error response byte (0xFF = invalid address)
    tx_data  <= 8'hFF;
    tx_valid <= 1'b1;
    
    if (tx_ready) begin
        // Error response sent, return to IDLE
        next_state <= STATE_IDLE;
    end
end
```

### 5.6 Address Validation in RTL

```systemverilog
// Validate host request address before issuing to bus
// Host can ONLY access RTL peripheral space (0x5000_0000 - 0x5FFF_FFFF)
localparam RTL_PERIPH_BASE  = 32'h5000_0000;
localparam RTL_PERIPH_LIMIT = 32'h6000_0000;

logic host_addr_valid;
assign host_addr_valid = (host_cap_addr >= RTL_PERIPH_BASE) && 
                         (host_cap_addr < RTL_PERIPH_LIMIT);

// In STATE_HOST_BUS_REQ:
if (host_addr_valid) begin
    host_bus_req <= 1'b1;
    next_state <= STATE_HOST_BUS_WAIT;
end else begin
    // Invalid address - send error response
    next_state <= STATE_HOST_ERROR;
end
```

---

## 6. Rust Integration Layer

### 6.1 Modified Files

| File | Changes |
|------|---------|
| `cpu-sim/src/sim.rs` | **MODIFIED:** Add host request/response queues, state machine |
| `cpu-sim/src/lib.rs` | **MODIFIED:** Export new types |

### 6.2 New Types

```rust
/// Direction of a bus transaction relative to the host
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionDirection {
    /// CPU initiated request to host (existing)
    CpuToHost,
    /// Host initiated request to FPGA (new)
    HostToFpga,
}

/// Host-initiated bus request
#[derive(Debug, Clone)]
pub struct HostBusRequest {
    /// Address to access
    pub addr: u32,
    /// Write data (for writes, ignored for reads)
    pub wdata: u32,
    /// Access size (0=byte, 1=half, 2=word)
    pub size: u8,
    /// Write enable
    pub we: bool,
}

/// Response to a host-initiated bus request
#[derive(Debug, Clone)]
pub enum HostBusResponse {
    /// Successful read with data
    ReadData(u32),
    /// Successful write acknowledgement
    WriteAck,
    /// Error response
    Error(FpgaError),
}

/// Error codes from FPGA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaError {
    /// Request targeted invalid address (would route back to host)
    InvalidAddress,
    /// Request timed out
    Timeout,
    /// Protocol error
    ProtocolError,
}
```

### 6.3 SimulatorView Interface Extensions

```rust
impl<'a> SimulatorView<'a> {
    // ... existing methods ...
    
    /// Queue a host-initiated bus request
    /// 
    /// The request will be sent to the FPGA on subsequent step() calls.
    /// Use receive_bus_response() to get the result.
    /// 
    /// # Arguments
    /// * `request` - The bus request to send
    /// 
    /// # Errors
    /// * Returns error if a request is already pending (one at a time)
    /// * Returns error if address is outside RTL peripheral range
    pub fn send_bus_request(&mut self, request: HostBusRequest) -> Result<(), String> {
        // Validate address range
        if request.addr < RTL_PERIPH_BASE || request.addr >= RTL_PERIPH_LIMIT {
            return Err(format!(
                "Host-initiated request address 0x{:08x} is outside RTL peripheral range \
                 (0x{:08x} - 0x{:08x}). Host cannot access addresses that route back to host.",
                request.addr, RTL_PERIPH_BASE, RTL_PERIPH_LIMIT
            ));
        }
        
        // Check if a request is already pending
        if self.host_request_pending() {
            return Err("A host-initiated request is already pending. \
                        Wait for response before sending another.".to_string());
        }
        
        // Queue the request
        self.host_bus_request_queue.push_back(request);
        Ok(())
    }
    
    /// Receive response to a previously sent host-initiated bus request
    /// 
    /// # Returns
    /// * `Some(response)` - Response received
    /// * `None` - No response available yet
    pub fn receive_bus_response(&mut self) -> Option<HostBusResponse> {
        self.host_bus_response_queue.pop_front()
    }
    
    /// Check if a host-initiated request is pending
    pub fn host_request_pending(&self) -> bool {
        !self.host_bus_request_queue.is_empty() || 
        self.host_bus_state != HostBusHostState::Idle
    }
}
```

### 6.4 Extended Host Bus State Machine

```rust
/// Host-side state machine for host-initiated requests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusHostState {
    /// Idle - ready to send or receive
    Idle,
    /// Sending host request header
    TxHeader,
    /// Sending address bytes
    TxAddr { byte_idx: u8 },
    /// Sending write data bytes
    TxWdata { byte_idx: u8 },
    /// Waiting for response
    RxWaitingAckOrData,
    /// Receiving read data bytes
    RxRdata { byte_idx: u8 },
}

/// Handle host-initiated bus requests
fn handle_host_bus_host_requests(&mut self) {
    match self.host_bus_host_state {
        HostBusHostState::Idle => {
            // If we have a queued request and no CPU transaction pending, send it
            if let Some(request) = self.host_bus_request_queue.front().cloned() {
                // Check we're not in the middle of processing a CPU request
                if self.host_bus_state == HostBusState::Idle {
                    // Start sending host request
                    self.current_host_request = Some(request.clone());
                    self.host_bus_host_state = HostBusHostState::TxHeader;
                }
            }
        }
        
        HostBusHostState::TxHeader => {
            let request = self.current_host_request.as_ref().unwrap();
            // Send header with packet type 0010 (host-initiated request)
            let header = 0x20  // Packet type 0b0010 = host-initiated request (Host → FPGA)
                       | ((request.size as u8 & 0x03) << 2)
                       | (if request.we { 0x01 } else { 0x00 });
            
            if self.try_send_rx_byte(header) {
                self.host_bus_host_state = HostBusHostState::TxAddr { byte_idx: 0 };
            }
        }
        
        // ... remaining states follow same pattern as CPU→Host ...
    }
}
```

---

## 7. Testing Strategy

### 7.1 RTL Testbench Tests (testbench/tests/)

Create new test file: `testbench/tests/host_bus_interface_bidirectional_test.rs`

#### 7.1.1 Test Categories

| Category | Tests | Description |
|----------|-------|-------------|
| **Basic Host Requests** | 6 | Host→FPGA read/write for byte/half/word |
| **Address Validation** | 4 | Valid RTL addresses, invalid addresses |
| **Arbiter Priority** | 3 | Host priority, CPU preemption, fairness |
| **Protocol Edge Cases** | 5 | Backpressure, interleaved requests |
| **Simultaneous Requests** | 4 | Both sides send request simultaneously, buffering, response ordering |
| **Error Handling** | 4 | Invalid address response, recovery |

#### 7.1.2 Sample Test: Host Read Word

```rust
#[test]
fn test_host_initiated_read_word() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);
    
    // Simulate host sending a read request for LED register
    let target_addr: u32 = 0x50000000;  // LED_BASE
    
    // Send host request header: {4'b0010, size=10, 1'b0, we=0} = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");
    
    // Send address (little-endian)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    
    // Module should now issue bus request
    clock_cycle!(dut);
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_addr, target_addr, "host_bus_addr should match");
    assert_eq!(dut.host_bus_we, 0, "host_bus_we should be 0 for read");
    
    // Provide bus response (simulated LED value)
    dut.host_bus_rdata = 0x000000AA;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;
    
    // Receive response (4 bytes for word, little-endian)
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    let b1 = receive_tx_byte(&mut dut, 100).expect("rdata[15:8]");
    let b2 = receive_tx_byte(&mut dut, 100).expect("rdata[23:16]");
    let b3 = receive_tx_byte(&mut dut, 100).expect("rdata[31:24]");
    
    let rdata = (b3 as u32) << 24 | (b2 as u32) << 16 | (b1 as u32) << 8 | (b0 as u32);
    assert_eq!(rdata, 0x000000AA, "Read data should match LED value");
}
```

#### 7.1.3 Sample Test: Invalid Address Error

```rust
#[test]
fn test_host_request_invalid_address_error() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);
    
    // Send host request to DRAM address (invalid - would loop back to host)
    let invalid_addr: u32 = 0x80000000;  // DRAM_BASE
    
    // Send request header (0x28 = Host-initiated word read request)
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");
    
    // Send invalid address (little-endian)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x80, 100), "addr[31:24]");
    
    // Module should send error response without issuing bus request
    let response = receive_tx_byte(&mut dut, 100).expect("Error response");
    assert_eq!(response, 0xFF, "Should receive error code 0xFF");
    
    // Verify bus was NOT accessed
    // (host_bus_req should never have been asserted)
}
```

#### 7.1.4 Sample Test: Simultaneous Requests

```rust
#[test]
fn test_simultaneous_requests_fpga_buffers_host_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);
    
    // Step 1: CPU initiates request (FPGA sends to Host)
    dut.addr = 0x80000000;  // DRAM address
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;  // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;
    
    // Drain header byte from TX
    let header = receive_tx_byte(&mut dut, 100).expect("CPU request header");
    assert_eq!(header & 0xF0, 0x00, "CPU request should have packet type 0000");
    
    // Step 2: WHILE CPU request is outstanding, Host sends a request
    // This simulates the simultaneous request scenario
    // Send host request header (0x28 = Host-initiated word read)
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send host header");
    
    // Step 3: FPGA should buffer the host request (not process yet)
    // because bus is busy with CPU transaction
    // Continue sending host request address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    
    // Step 4: Complete CPU request by providing response from Host
    // Drain remaining TX bytes for CPU request (address bytes)
    for _ in 0..4 {
        receive_tx_byte(&mut dut, 100).expect("CPU address byte");
    }
    for _ in 0..4 {
        receive_tx_byte(&mut dut, 100).expect("CPU wdata byte");
    }
    
    // Send CPU response (write ack)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "CPU write ack");
    
    // Step 5: Now CPU transaction is complete, FPGA should process buffered Host request
    // Verify host_bus_req is now asserted for the buffered Host request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted after CPU txn completes");
    assert_eq!(dut.host_bus_addr, 0x50000000, "Should process buffered Host request");
    
    // Complete the Host request
    dut.host_bus_rdata = 0xAA;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;
    
    // Receive Host response
    let resp = receive_tx_byte(&mut dut, 100).expect("Host response byte 0");
    assert_eq!(resp, 0xAA, "Host should receive correct LED value");
}
```

### 7.2 CPU-Sim Integration Tests

Create new test file: `cpu-sim/tests/test_host_bus_requests.rs`

#### 7.2.1 Test Categories

| Category | Tests | Description |
|----------|-------|-------------|
| **Basic API** | 4 | send_bus_request, receive_bus_response |
| **LED Integration** | 3 | Read/write LED via host requests |
| **Mixed Traffic** | 4 | CPU + Host concurrent access |
| **Simultaneous Requests** | 3 | Request buffering, response ordering |
| **Error Handling** | 3 | Invalid addresses, pending check |

#### 7.2.2 End-to-End LED Test (from problem statement)

```rust
/// End-to-end test verifying bi-directional host bus communication
/// 
/// Test sequence:
/// 1. CPU writes value to LED device
/// 2. Verify LED value via host bus read request
/// 3. Host writes new value to LED via bus request
/// 4. CPU reads LED value and verifies
/// 5. Exit with success code via tohost
#[test]
fn test_host_bus_end_to_end_led() {
    init_test_logger();
    
    const START_ADDR: u32 = 0x8000_0000;
    const LED_BASE: u32 = 0x5000_0000;
    
    // Step 1: CPU writes 0xAA to LED
    // Step 4: CPU reads LED (should be 0x55), verifies, then exits
    let instructions = vec![
        // Phase 1: CPU writes 0xAA to LED
        lui(15, LED_BASE),         // x15 = LED base
        addi(14, 0, 0xAA),         // x14 = 0xAA
        sw(15, 14, 0),             // Write 0xAA to LED
        
        // Signal to host that write is complete (write 1 to scratch location)
        lui(13, 0x80000000),       // x13 = DRAM base
        addi(12, 0, 1),            // x12 = 1
        sw(13, 12, 0x100),         // Write 1 to 0x80000100 (signal)
        
        // Phase 2: Wait for host to signal back (poll location 0x80000104)
        lw(11, 13, 0x104),         // Read signal from host
        beq(11, 0, -4),            // Loop if zero
        
        // Phase 3: CPU reads LED (should be 0x55 after host write)
        lw(10, 15, 0),             // Read LED into x10
        andi(10, 10, 0xFF),        // Mask to 8 bits
        
        // Verify LED value is 0x55
        addi(9, 0, 0x55),          // x9 = expected value
        bne(10, 9, 8),             // Skip success if not equal (jump to fail)
        
        // Success path
        lui(8, 0x10000000),        // x8 = tohost base
        addi(7, 0, 1),             // x7 = success code
        sw(8, 7, 0),               // Write success to tohost
        jal(0, 0),                 // Halt (infinite loop)
        
        // Fail path (jumped to if LED != 0x55)
        lui(8, 0x10000000),        // x8 = tohost base
        addi(7, 0, 2),             // x7 = failure code
        sw(8, 7, 0),               // Write failure to tohost
        jal(0, 0),                 // Halt
    ];
    
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();
    
    let test_state = Arc::new(Mutex::new(TestState {
        phase: 0,
        host_read_result: None,
        host_write_sent: false,
    }));
    let test_state_callback = test_state.clone();
    
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        // Instruction complete callback - handles host bus operations
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            
            match state.phase {
                0 => {
                    // Check if CPU signaled phase 1 complete
                    let signal = sim.read_word(0x80000100);
                    if signal == 1 {
                        // Phase 2a: Send host read request
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0,
                            size: 2, // word
                            we: false,
                        };
                        sim.send_bus_request(request).expect("Failed to send read request");
                        state.phase = 1;
                    }
                }
                1 => {
                    // Phase 2b: Check for read response
                    if let Some(HostBusResponse::ReadData(value)) = sim.receive_bus_response() {
                        assert_eq!(value & 0xFF, 0xAA, "Host read should see 0xAA");
                        state.host_read_result = Some(value);
                        
                        // Phase 3a: Send host write request (0x55)
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0x55,
                            size: 2, // word
                            we: true,
                        };
                        sim.send_bus_request(request).expect("Failed to send write request");
                        state.phase = 2;
                    }
                }
                2 => {
                    // Phase 3b: Check for write response
                    if let Some(HostBusResponse::WriteAck) = sim.receive_bus_response() {
                        state.host_write_sent = true;
                        
                        // Signal CPU to continue
                        sim.write_word(0x80000104, 1);
                        state.phase = 3;
                    }
                }
                _ => {}
            }
        }),
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
    
    let final_state = test_state.lock().unwrap();
    assert!(final_state.host_read_result.is_some(), "Host should have read LED");
    assert!(final_state.host_write_sent, "Host should have written LED");
    assert_eq!(result.tohost_value, Some(1), "Program should exit with success");
}

struct TestState {
    phase: u32,
    host_read_result: Option<u32>,
    host_write_sent: bool,
}
```

### 7.3 Test Matrix Summary

| Test Type | Count | Location |
|-----------|-------|----------|
| RTL Arbiter Tests | 8 | `testbench/tests/bus_arbiter_test.rs` |
| RTL Host Request Tests | 19 | `testbench/tests/host_bus_interface_bidirectional_test.rs` |
| CPU-Sim Integration Tests | 15 | `cpu-sim/tests/test_host_bus_requests.rs` |
| **Total New Tests** | **42** | |

---

## 8. Implementation Checklist

### Phase 1: RTL Changes

- [ ] Create `rtl/bus_arbiter.sv` module
  - [ ] Implement priority arbiter (Host > CPU)
  - [ ] Add state machine for grant holding
  - [ ] Add verilator lint compliance
  
- [ ] Modify `rtl/host_bus_interface.sv`
  - [ ] Add Host→FPGA request state machine
  - [ ] Add bus master interface ports
  - [ ] Add address validation logic
  - [ ] Add error response handling (STATE_HOST_ERROR)
  - [ ] Update protocol header format (packet type bits in upper nibble)
  - [ ] Add buffered host request register for simultaneous request handling
  - [ ] Implement request buffering when CPU transaction is active
  
- [ ] Modify `rtl/bus.sv`
  - [ ] Add second master interface for arbiter output
  - [ ] Ensure address decode works for both masters
  
- [ ] Modify `rtl/top.sv`
  - [ ] Instantiate bus_arbiter
  - [ ] Wire CPU through arbiter
  - [ ] Wire host_bus_interface master port through arbiter
  - [ ] Connect arbiter output to bus.sv

### Phase 2: Rust Changes

- [ ] Modify `cpu-sim/src/sim.rs`
  - [ ] Add `HostBusRequest` and `HostBusResponse` types
  - [ ] Add host request/response queues
  - [ ] Add `HostBusHostState` state machine
  - [ ] Implement `send_bus_request()` method
  - [ ] Implement `receive_bus_response()` method
  - [ ] Add address validation for host requests
  - [ ] Update `handle_host_bus_interface()` for bidirectional traffic
  - [ ] Add buffered CPU request handling for simultaneous request scenario
  - [ ] Implement packet type detection (upper nibble) to distinguish requests from responses
  
- [ ] Modify `cpu-sim/src/lib.rs`
  - [ ] Export new types
  
- [ ] Update `riscv_shared/src/bus.rs`
  - [ ] Add error code constants

### Phase 3: Testing

- [ ] Create RTL tests
  - [ ] `testbench/tests/bus_arbiter_test.rs`
  - [ ] `testbench/tests/host_bus_interface_bidirectional_test.rs`
  
- [ ] Create CPU-Sim tests
  - [ ] `cpu-sim/tests/test_host_bus_requests.rs`
  - [ ] End-to-end LED test
  
- [ ] Update `riscv_core` if needed
  - [ ] Add new model bindings for bus_arbiter if tested standalone

### Phase 4: Verification

- [ ] Run all existing tests (ensure no regression)
- [ ] Run new tests
- [ ] Verify FPGA synthesis still works (`cd fpga && make`)
- [ ] Run verilator lint on all modified RTL
- [ ] Run cargo clippy on all Rust code
- [ ] Run cargo fmt on all Rust code

### Phase 5: Documentation

- [ ] Update `AGENTS.md` with new memory map info
- [ ] Update `docs/README.md` if needed
- [ ] Add inline documentation to new code
- [ ] Mark this plan document as 'Implemented' and move to archive (preserve design rationale)

---

## Appendix A: Detailed Timing Diagrams

### A.1 Host-Initiated Read Transaction

```
                    1     2     3     4     5     6     7     8     9    10    11    12
clk            ─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────

rx_valid       _____|▀▀▀▀▀▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____
rx_data        XXXXX|hdr  |XXXXX|adr0 |XXXXX|adr1 |XXXXX|adr2 |XXXXX|adr3 |XXXXX
rx_ready       ▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀|_____|▀▀▀▀▀

                                                                      |<- bus access ->|
host_bus_req   _____________________________________________________________|▀▀▀▀▀▀▀▀▀▀▀|
host_bus_addr  XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|valid addr  |
host_bus_ready ____________________________________________________________|_____|▀▀▀▀▀|
host_bus_rdata XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|data |

tx_valid       ___________________________________________________________________________|▀▀▀▀
tx_data        XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|d0
tx_ready       ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
```

### A.2 Arbiter Priority Resolution

```
                    1     2     3     4     5     6     7     8
clk            ─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────

cpu_req        ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀|_____|▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
host_req       _____|▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀|_____|_____________

arb_state      |IDLE|CPU  |HOST |HOST |HOST |CPU  |CPU  |IDLE
                    |GRANT|GRANT|GRANT|GRANT|GRANT|GRANT|

cpu_grant      _____|▀▀▀▀▀|_____|_____|_____|▀▀▀▀▀▀▀▀▀▀▀|_____
host_grant     ___________|▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀|_____|_____|_____

bus_ready      _____|▀▀▀▀▀|_____|▀▀▀▀▀|▀▀▀▀▀|_____|▀▀▀▀▀|_____
```

---

## Appendix B: Error Code Reference

| Code | Name | Description | Recovery |
|------|------|-------------|----------|
| 0x00 | ACK | Write successful | N/A |
| 0xFF | INVALID_ADDRESS | Request targeted non-RTL address | Request rejected, return to IDLE |
| 0xFE | TIMEOUT | Response timeout (optional) | Request aborted, return to IDLE |
| 0xFD | PROTOCOL_ERROR | Unexpected packet format | Reset state machine, return to IDLE |

---

## Appendix C: Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Arbiter deadlock | Low | High | Thorough testing of edge cases |
| Address validation bypass | Low | Medium | Review by second engineer |
| Protocol mismatch | Medium | Medium | Shared header constants between RTL/Rust |
| Timing regression | Low | Medium | FPGA synthesis verification |
| Test coverage gaps | Medium | Low | Comprehensive test matrix |

---

*End of Implementation Plan*

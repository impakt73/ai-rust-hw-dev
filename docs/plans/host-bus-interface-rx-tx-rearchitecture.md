# Host Bus Interface RX/TX Re-Architecture Plan

## 1. Overview and Goals

### 1.1 Problem Statement

The current `host_bus_interface.sv` implementation has evolved organically, resulting in:
- **Monolithic FSM complexity**: A single large FSM (20+ states) handles both CPU-initiated and host-initiated transaction paths, TX and RX phases, and buffering logic.
- **Tight coupling**: The `host_rx_buffer.sv` module provides dual buffering (response + request) but its interface is tightly coupled to the main FSM, making state transitions difficult to reason about.
- **Backpressure ambiguity**: Ready/valid handshakes between components are implicit in state transitions, making it hard to verify correct backpressure behavior.
- **Maintenance burden**: Adding new features (e.g., error handling, extended packet types) requires modifying deeply nested FSM logic.

### 1.2 Goals

This re-architecture aims to:

1. **Modularity**: Split communication into independent `host_bus_rx` and `host_bus_tx` modules with clean, symmetric interfaces.
2. **Simplified FSMs**: Each module has a focused FSM handling only its direction of data flow.
3. **Explicit handshakes**: Use ready/valid semantics consistently across all module boundaries.
4. **Unified buffering**: Integrate `host_rx_buffer` functionality directly into `host_bus_rx`, eliminating a layer of indirection.
5. **Simplified assumptions**: Leverage the fact that CPU and host request paths are now mutually exclusive (no simultaneous buffering needed).
6. **Maintainability**: Enable future enhancements (DMA, error correction, flow control extensions) without refactoring.

### 1.3 Scope

**In Scope:**
- Create new `host_bus_rx.sv` module (absorbs `host_rx_buffer` functionality)
- Create new `host_bus_tx.sv` module
- Refactor `host_bus_interface.sv` to orchestrate RX/TX modules
- Remove `host_rx_buffer.sv` module
- Update testbench for modular testing

**Out of Scope:**
- Changes to packet protocol format (header, sizes, endianness)
- Changes to external UART/transport layer
- Changes to system bus arbiter or memory map
- Performance optimizations (pipelining, multi-beat bursts)

---

## 2. Current State Pain Points

### 2.1 Complexity Metrics

```
host_bus_interface.sv:  ~467 lines, 20 states, 3 major code paths
host_rx_buffer.sv:      ~411 lines, 13 states, dual buffering logic
Total:                  ~878 lines, interleaved control flow
```

### 2.2 Specific Issues

1. **FSM State Explosion**:
   - CPU-initiated TX: 9 states (header + addr + data)
   - CPU-initiated RX: 1 wait state + buffer dependency
   - Host-initiated TX: 5 states (header + data)
   - Host-initiated RX: 13 states in separate module
   - State transitions depend on runtime packet size (byte/half/word)

2. **Dual Buffering Overhead**:
   - `host_rx_buffer` maintains separate response and request buffers
   - Assumption: CPU and host requests can arrive simultaneously
   - Reality (current use): Only one path active at a time due to bus arbiter mutual exclusion
   - Result: Wasted registers and complex valid/consumed handshakes

3. **Unclear Backpressure**:
   - `rx_ready` logic: `is_receiving_packet || can_accept_new_packet`
   - Depends on two buffer valid flags and current FSM state
   - Difficult to prove correctness in formal verification

4. **Tight Coupling**:
   - `host_bus_interface` directly accesses buffer outputs (`buf_resp_rdata`, `buf_req_addr`)
   - `buf_req_consumed` signal computed from FSM state and bus master handshake
   - Changes to buffer logic require coordinated changes to main FSM

5. **Limited Testability**:
   - Cannot test RX path independently from TX path
   - Cannot isolate packet parsing from bus master/slave interactions
   - Difficult to inject errors or test corner cases

---

## 3. Target Architecture

### 3.1 Module Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│                   host_bus_interface.sv                     │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Packet Type Router & Orchestrator                     │ │
│  │  - Classifies packets (request vs response)            │ │
│  │  - Routes to appropriate path (CPU vs Host)            │ │
│  │  - Handles priority arbitration                        │ │
│  └───────┬─────────────────────────────┬──────────────────┘ │
│          │                             │                    │
│  ┌───────▼───────────┐         ┌───────▼──────────┐        │
│  │  host_bus_rx.sv   │         │  host_bus_tx.sv  │        │
│  │  ┌──────────────┐ │         │  ┌─────────────┐ │        │
│  │  │ RX FSM       │ │         │  │ TX FSM      │ │        │
│  │  │ - Parse hdr  │ │         │  │ - Build pkt │ │        │
│  │  │ - Accum data │ │         │  │ - Serialize │ │        │
│  │  │ - Buffer pkt │ │         │  │ - Mux bytes │ │        │
│  │  └──────────────┘ │         │  └─────────────┘ │        │
│  └───────────────────┘         └──────────────────┘        │
└──────┬──────────────────────────────────┬─────────────────┘
       │                                  │
  ┌────▼────┐                        ┌────▼────┐
  │ RX byte │                        │ TX byte │
  │ stream  │                        │ stream  │
  │(UART/...)                        │(UART/...)
  └─────────┘                        └─────────┘
```

### 3.2 Module Responsibilities

#### 3.2.1 `host_bus_rx`
**Purpose**: Receive and parse incoming byte stream, buffer complete packets, provide structured outputs.

**Responsibilities**:
- Accept byte stream via ready/valid handshake
- Parse packet header to extract type/size/we
- Accumulate address bytes (little-endian) for request packets
- Accumulate data bytes (little-endian) for write requests and read responses
- Buffer complete packets until consumed
- Provide separate outputs for response packets (type 0001) and request packets (type 0010)
- Assert backpressure when buffer is full

**Key Assumption**: Only one packet type buffered at a time (response OR request, not both).

#### 3.2.2 `host_bus_tx`
**Purpose**: Accept structured bus transactions, serialize to byte stream.

**Responsibilities**:
- Accept CPU-initiated requests via bus slave interface
- Accept host-initiated responses via dedicated interface
- Serialize packets to byte stream (header + addr + data)
- Implement ready/valid handshake on byte stream output
- Handle variable-length packets based on size field
- Prioritize CPU-initiated requests over host responses

**Key Feature**: Dual input ports with explicit priority arbitration.

#### 3.2.3 `host_bus_interface` (Orchestrator)
**Purpose**: Top-level module that wires RX/TX modules and implements routing logic.

**Responsibilities**:
- Instantiate `host_bus_rx` and `host_bus_tx`
- Route RX responses to CPU slave interface `rdata` output
- Route RX requests to host bus master interface
- Route CPU slave requests to TX module
- Route host bus master responses to TX module
- Implement consumed/handshake signaling between components
- No FSM required (simple combinational routing + edge-triggered handshakes)

---

## 4. Detailed Interface Specifications

### 4.1 `host_bus_rx` Interface

```systemverilog
module host_bus_rx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // ============================================================
    // RX Byte Stream Interface (from UART/transport)
    // ============================================================
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // ============================================================
    // Response Packet Output (Type 0001: Host → CPU)
    // Valid when complete response packet has been received
    // ============================================================
    output logic        resp_valid,       // Complete packet ready
    output logic        resp_we,          // Write enable (echoed from req)
    output logic [1:0]  resp_size,        // Access size (00=byte, 01=half, 10=word)
    output logic [31:0] resp_rdata,       // Read data (0 for writes)
    input  logic        resp_consumed,    // Pulse: consumer has latched data
    
    // ============================================================
    // Request Packet Output (Type 0010: Host → Target)
    // Valid when complete request packet has been received
    // ============================================================
    output logic        req_valid,        // Complete packet ready
    output logic        req_we,           // Write enable
    output logic [1:0]  req_size,         // Access size
    output logic [31:0] req_addr,         // Address
    output logic [31:0] req_wdata,        // Write data (0 for reads)
    input  logic        req_consumed      // Pulse: consumer has latched data
);
```

#### 4.1.1 Protocol Rules

**RX Handshake**:
- Rule 1: Data transfer occurs when `rx_valid && rx_ready` on rising edge of `clk`
- Rule 2: `rx_ready` may be lowered mid-packet if buffer fills (backpressure)
- Rule 3: Sender must hold `rx_data` stable while `rx_valid=1` and `rx_ready=0`

**Output Buffering**:
- Rule 4: `resp_valid` asserted when final byte of type 0001 packet received
- Rule 5: `req_valid` asserted when final byte of type 0010 packet received
- Rule 6: Only ONE of {`resp_valid`, `req_valid`} may be high at a time
- Rule 7: Valid remains high until corresponding `consumed` pulse received
- Rule 8: Output data must remain stable while `valid=1`

**Backpressure**:
- Rule 9: `rx_ready=0` when buffer is full (packet complete, not yet consumed)
- Rule 10: `rx_ready=1` when buffer is empty OR actively receiving packet
- Rule 11: Once packet reception starts, RX must not stall (accept all bytes)

**Error Handling**:
- Rule 12: Unknown packet types (not 0001 or 0010) are silently dropped
- Rule 13: No timeout mechanism (relies on transport layer)

---

### 4.2 `host_bus_tx` Interface

```systemverilog
module host_bus_tx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // ============================================================
    // TX Byte Stream Interface (to UART/transport)
    // ============================================================
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,
    
    // ============================================================
    // CPU Request Input (Type 0000: CPU → Host)
    // Bus slave interface from CPU-side transactions
    // ============================================================
    input  logic [31:0] cpu_req_addr,
    input  logic [31:0] cpu_req_wdata,
    input  logic        cpu_req_we,
    input  logic [1:0]  cpu_req_size,
    input  logic        cpu_req_valid,    // Request present
    output logic        cpu_req_ready,    // TX module ready to accept
    
    // ============================================================
    // Host Response Input (Type 0011: Target → Host)
    // Response data for host-initiated requests
    // ============================================================
    input  logic [31:0] host_resp_rdata,
    input  logic        host_resp_we,     // Echoed from original request
    input  logic [1:0]  host_resp_size,   // Echoed from original request
    input  logic        host_resp_valid,  // Response present
    output logic        host_resp_ready   // TX module ready to accept
);
```

#### 4.2.1 Protocol Rules

**TX Handshake**:
- Rule 1: Data transfer occurs when `tx_valid && tx_ready` on rising edge of `clk`
- Rule 2: `tx_data` must remain stable while `tx_valid=1` and `tx_ready=0`
- Rule 3: `tx_valid` may be lowered and re-asserted between packet bytes

**Input Arbitration**:
- Rule 4: CPU requests have priority over host responses
- Rule 5: When `cpu_req_valid=1`, CPU request is accepted (captured) when `cpu_req_ready=1`
- Rule 6: When `host_resp_valid=1` AND `cpu_req_valid=0`, host response is accepted
- Rule 7: Input handshakes are single-cycle pulses (ready asserted for 1 cycle)

**Packet Transmission**:
- Rule 8: Once packet transmission starts, all bytes sent consecutively (no gaps if `tx_ready=1`)
- Rule 9: If `tx_ready=0`, TX module holds current byte until ready
- Rule 10: Packet length determined by `size` and `we` fields:
  - Read request: 5 bytes (header + 4 addr)
  - Write byte: 6 bytes (header + 4 addr + 1 data)
  - Write half: 7 bytes (header + 4 addr + 2 data)
  - Write word: 9 bytes (header + 4 addr + 4 data)
  - Host read response: 2-5 bytes (header + 1-4 data)
  - Host write response: 1 byte (header only)

**Ready Signaling**:
- Rule 11: `cpu_req_ready=1` when FSM is IDLE (no active transmission)
- Rule 12: `host_resp_ready=1` when FSM is IDLE AND `cpu_req_valid=0`

---

## 5. `host_bus_interface` Orchestration

### 5.1 Packet Type Classification

The orchestrator does not parse packets directly; it routes data from RX/TX modules:

```
RX Module Output          Direction      Destination
─────────────────────────────────────────────────────────
resp_valid=1              Host → CPU     CPU slave rdata
req_valid=1               Host → Target  Bus master interface

TX Module Input           Direction      Source
─────────────────────────────────────────────────────────
cpu_req_valid=1           CPU → Host     CPU slave interface
host_resp_valid=1         Target → Host  Bus master rdata
```

### 5.2 Routing Logic (Combinational)

```systemverilog
// ============================================================
// CPU Slave Interface (CPU → Host TX path)
// ============================================================
assign tx.cpu_req_addr   = addr;
assign tx.cpu_req_wdata  = wdata;
assign tx.cpu_req_we     = we;
assign tx.cpu_req_size   = size;
assign tx.cpu_req_valid  = req;        // Direct passthrough
assign ready             = tx.cpu_req_ready;  // Direct passthrough

// ============================================================
// CPU Slave Read Data (Host → CPU RX path)
// ============================================================
assign rdata = rx.resp_rdata;          // Direct passthrough
// Note: resp_consumed pulse generated when CPU transaction completes

// ============================================================
// Bus Master Interface (Host → Target RX path, Target → Host TX path)
// ============================================================
assign host_bus_addr  = rx.req_addr;
assign host_bus_wdata = rx.req_wdata;
assign host_bus_we    = rx.req_we;
assign host_bus_size  = rx.req_size;
assign host_bus_req   = rx.req_valid;  // Direct passthrough

assign tx.host_resp_rdata = host_bus_rdata;
assign tx.host_resp_we    = rx.req_we;    // Echo from original request
assign tx.host_resp_size  = rx.req_size;  // Echo from original request
// Note: host_resp_valid generated when bus master handshake completes
```

### 5.3 Handshake Completion Logic (Sequential)

```systemverilog
// ============================================================
// CPU-initiated transaction completion
// ============================================================
logic cpu_transaction_complete;
assign cpu_transaction_complete = req && ready;  // Single-cycle pulse

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx.resp_consumed <= 1'b0;
    end else begin
        // Consume RX response when CPU transaction completes
        rx.resp_consumed <= cpu_transaction_complete && rx.resp_valid;
    end
end

// ============================================================
// Host-initiated transaction completion
// ============================================================
logic bus_master_handshake_complete;
assign bus_master_handshake_complete = host_bus_req && host_bus_ready;

logic host_resp_pending;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        host_resp_pending    <= 1'b0;
        rx.req_consumed      <= 1'b0;
        tx.host_resp_valid   <= 1'b0;
    end else begin
        // Step 1: Bus master completes → capture response, mark pending
        if (bus_master_handshake_complete && !host_resp_pending) begin
            host_resp_pending <= 1'b1;
            rx.req_consumed   <= 1'b1;
        end else begin
            rx.req_consumed   <= 1'b0;  // Single-cycle pulse
        end
        
        // Step 2: TX module ready → send response
        if (host_resp_pending && !tx.host_resp_valid) begin
            tx.host_resp_valid <= 1'b1;
        end
        
        // Step 3: TX module accepts → clear pending
        if (tx.host_resp_valid && tx.host_resp_ready) begin
            tx.host_resp_valid <= 1'b0;
            host_resp_pending  <= 1'b0;
        end
    end
end
```

### 5.4 Priority and Mutual Exclusion

**Mutual Exclusion Guarantee**:
- By design: CPU-initiated and host-initiated paths use different address spaces
- System bus arbiter ensures only one master active at a time
- RX module buffers only ONE packet type at a time (response XOR request)
- TX module arbitration gives priority to CPU requests

**Priority Rules**:
1. **RX side**: First-come-first-served (limited by buffer capacity)
2. **TX side**: CPU requests > host responses (implemented in TX FSM)
3. **Overall**: No starvation risk due to mutual exclusion of paths

---

## 6. Ready/Valid Handshake Rules

### 6.1 Canonical Ready/Valid Protocol

All interfaces follow standard AXI-style ready/valid handshaking:

```
Initiator                    Target
──────────                   ──────
Assert valid ────────────────▶
Hold data stable             │
                             │ (May take N cycles to assert ready)
                             │
          ◀────────────────── Assert ready
Transfer occurs on rising edge of clk when (valid && ready)
```

**Rules**:
1. Once `valid` is asserted, it must remain asserted until handshake completes
2. `data` must remain stable while `valid=1` and `ready=0`
3. `ready` may be asserted before, during, or after `valid` (no ordering requirement)
4. Transfer occurs on RISING edge of clock when both `valid=1` and `ready=1`
5. Either signal may be combinationally dependent on the other (no deadlock if rules followed)

### 6.2 Module-Specific Handshake Behavior

#### 6.2.1 `host_bus_rx` RX Stream

```
State: IDLE
  rx_ready = 1 (buffer empty, ready to receive)
  
State: RECEIVING_HEADER
  rx_ready = 1 (must accept header to determine packet type)
  
State: RECEIVING_DATA (packet type known, buffer free)
  rx_ready = 1 (continue accepting bytes for current packet)
  
State: PACKET_COMPLETE (valid=1, waiting for consumed pulse)
  rx_ready = 0 (buffer full, assert backpressure)
```

**Critical Invariant**: Once packet reception starts, `rx_ready` remains high until packet complete.

#### 6.2.2 `host_bus_tx` TX Stream

```
State: IDLE
  tx_valid = 0
  cpu_req_ready = 1 (ready to accept new request)
  host_resp_ready = 1 (if no CPU request pending)
  
State: TX_ACTIVE (transmitting packet bytes)
  tx_valid = 1
  tx_data = current_byte
  cpu_req_ready = 0
  host_resp_ready = 0
  
  On each (tx_valid && tx_ready):
    Advance to next byte OR return to IDLE if packet complete
```

#### 6.2.3 Bus Slave Interface (CPU → TX)

```
CPU asserts:    req=1, addr, wdata, we, size
TX responds:    ready=1 (single cycle, when accepted)
CPU interprets: Transaction complete, may proceed with next request

Timing:
  Cycle N:   CPU: req=1, TX: ready=0 (busy transmitting previous packet)
  Cycle N+1: CPU: req=1, TX: ready=0 (still busy)
  Cycle N+2: CPU: req=1, TX: ready=1 (accepted, latched into TX module)
  Cycle N+3: CPU: req=0, TX: ready=0 (transmission in progress)
```

#### 6.2.4 Bus Master Interface (RX → Target)

```
RX asserts:     req_valid=1, req_addr, req_wdata, req_we, req_size
Arbiter/Target: host_bus_ready=1 (when arbiter grants access)
RX interprets:  Transaction submitted, await completion

Note: req_valid is mapped directly to host_bus_req
      req_consumed pulse generated by orchestrator on handshake completion
```

### 6.3 Backpressure Propagation

**Scenario 1: RX byte stream backpressure**
```
UART/Transport → host_bus_rx.rx_ready=0 (buffer full)
   ↓
Transport layer stalls (implementation-dependent)
   ↓
Host application blocks on write() call
```

**Scenario 2: TX byte stream backpressure**
```
host_bus_tx.tx_valid=1, tx_ready=0 (UART transmit FIFO full)
   ↓
TX FSM stalls in current state, holds tx_data stable
   ↓
cpu_req_ready=0 (not accepting new requests)
   ↓
CPU bus transaction stalls (req=1, ready=0)
```

**Scenario 3: Bus master backpressure**
```
host_bus_req=1, host_bus_ready=0 (arbiter busy, or target peripheral slow)
   ↓
Orchestrator does not pulse req_consumed
   ↓
RX module keeps req_valid=1, rx_ready=0
   ↓
RX byte stream backpressure (Scenario 1)
```

---

## 7. FSM Strategy and Data Path Details

### 7.1 `host_bus_rx` FSM

#### 7.1.1 State Diagram

```
                         ┌─────────────┐
                         │    IDLE     │ (rx_ready=1, valid=0)
                         └──────┬──────┘
                                │
                 rx_valid=1 (header byte received)
                                │
                         ┌──────▼──────────────┐
                         │  CLASSIFY_PACKET   │
                         │  (decode header)    │
                         └──────┬──────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
     Type 0001 (Response)              Type 0010 (Request)
                │                               │
                │                               │
      ┌─────────▼──────────┐          ┌────────▼─────────┐
      │  RX_RESP_DATA_0    │          │  RX_REQ_ADDR_0   │
      │  (byte 0 of data)  │          │  (byte 0 of addr)│
      └─────────┬──────────┘          └────────┬─────────┘
                │                               │
        (size-dependent)                 (always 4 bytes)
                │                               │
      ┌─────────▼──────────┐          ┌────────▼─────────┐
      │  RX_RESP_DATA_1-3  │          │  RX_REQ_ADDR_1-3 │
      │  (half/word)       │          └────────┬─────────┘
      └─────────┬──────────┘                   │
                │                              │
                │                     ┌────────▼─────────┐
                │                     │ RX_REQ_WDATA_0-3 │
                │                     │ (if we=1)        │
                │                     └────────┬─────────┘
                │                              │
                └──────────────┬───────────────┘
                               │
                      ┌────────▼──────────┐
                      │  PACKET_COMPLETE  │ (valid=1, rx_ready=0)
                      └────────┬──────────┘
                               │
                    consumed=1 (pulse from orchestrator)
                               │
                      ┌────────▼──────────┐
                      │      IDLE         │
                      └───────────────────┘
```

#### 7.1.2 State Descriptions

| State              | rx_ready | resp_valid | req_valid | Next State Condition                  |
|--------------------|----------|------------|-----------|---------------------------------------|
| IDLE               | 1        | 0          | 0         | rx_valid=1 → CLASSIFY                 |
| CLASSIFY           | 1        | 0          | 0         | type=0001 → RX_RESP_DATA_0            |
|                    |          |            |           | type=0010 → RX_REQ_ADDR_0             |
|                    |          |            |           | type=other → IDLE (drop)              |
| RX_RESP_DATA_0     | 1        | 0          | 0         | size=byte → RESP_COMPLETE             |
|                    |          |            |           | size=half/word → RX_RESP_DATA_1       |
| RX_RESP_DATA_1-3   | 1        | 0          | 0         | (size-dependent transitions)          |
| RESP_COMPLETE      | 0        | 1          | 0         | resp_consumed → IDLE                  |
| RX_REQ_ADDR_0-3    | 1        | 0          | 0         | ADDR_3 + we=1 → RX_REQ_WDATA_0        |
|                    |          |            |           | ADDR_3 + we=0 → REQ_COMPLETE          |
| RX_REQ_WDATA_0-3   | 1        | 0          | 0         | (size-dependent transitions)          |
| REQ_COMPLETE       | 0        | 0          | 1         | req_consumed → IDLE                   |

#### 7.1.3 Data Path Registers

```systemverilog
// Buffered packet storage
logic        resp_buf_we;
logic [1:0]  resp_buf_size;
logic [31:0] resp_buf_rdata;

logic        req_buf_we;
logic [1:0]  req_buf_size;
logic [31:0] req_buf_addr;
logic [31:0] req_buf_wdata;

// Temporary header capture (used during CLASSIFY state)
logic [3:0]  temp_packet_type;
logic        temp_we;
logic [1:0]  temp_size;

// Byte accumulator (builds up multi-byte fields)
logic [31:0] byte_accumulator;
logic [2:0]  byte_counter;  // 0-3 for addr, 0-3 for data
```

#### 7.1.4 Byte Accumulation Logic

Little-endian accumulation (LSB first):

```systemverilog
always_ff @(posedge clk) begin
    if (rx_valid && rx_ready) begin
        case (state)
            RX_RESP_DATA_0, RX_REQ_ADDR_0, RX_REQ_WDATA_0:
                byte_accumulator[7:0] <= rx_data;
            
            RX_RESP_DATA_1, RX_REQ_ADDR_1, RX_REQ_WDATA_1:
                byte_accumulator[15:8] <= rx_data;
            
            RX_RESP_DATA_2, RX_REQ_ADDR_2, RX_REQ_WDATA_2:
                byte_accumulator[23:16] <= rx_data;
            
            RX_RESP_DATA_3, RX_REQ_ADDR_3, RX_REQ_WDATA_3:
                byte_accumulator[31:24] <= rx_data;
        endcase
    end
end
```

---

### 7.2 `host_bus_tx` FSM

#### 7.2.1 State Diagram

```
                    ┌─────────────┐
                    │    IDLE     │ (tx_valid=0, ready signals=1)
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
     cpu_req_valid=1           host_resp_valid=1
              │                         │ (only if cpu_req_valid=0)
              │                         │
    ┌─────────▼──────────┐   ┌─────────▼──────────┐
    │  TX_CPU_HEADER     │   │  TX_HOST_HEADER    │
    │  (type 0000)       │   │  (type 0011)       │
    └─────────┬──────────┘   └─────────┬──────────┘
              │                         │
              │                ┌────────┴─────────┐
              │                │                  │
              │           we=1 (write)       we=0 (read)
              │                │                  │
              │         ┌──────▼──────┐    ┌──────▼──────┐
              │         │    IDLE     │    │TX_HOST_DATA │
              │         │  (1 byte)   │    │  (1-4 bytes)│
              │         └─────────────┘    └──────┬──────┘
              │                                   │
              │                            ┌──────▼──────┐
              │                            │    IDLE     │
              │                            └─────────────┘
    ┌─────────▼──────────┐
    │  TX_CPU_ADDR_0-3   │ (4 bytes, little-endian)
    └─────────┬──────────┘
              │
    ┌─────────▼──────────┐
    │ TX_CPU_WDATA_0-3   │ (0-4 bytes, if we=1)
    └─────────┬──────────┘
              │
    ┌─────────▼──────────┐
    │      IDLE          │
    └────────────────────┘
```

#### 7.2.2 State Descriptions

| State              | tx_valid | cpu_req_ready | host_resp_ready | Next State Condition          |
|--------------------|----------|---------------|-----------------|-------------------------------|
| IDLE               | 0        | 1             | !cpu_req_valid  | cpu_req_valid=1 → TX_CPU_HDR  |
|                    |          |               |                 | host_resp_valid=1 → TX_HOST_HDR|
| TX_CPU_HEADER      | 1        | 0             | 0               | handshake → TX_CPU_ADDR_0     |
| TX_CPU_ADDR_0-3    | 1        | 0             | 0               | ADDR_3 + we=1 → TX_CPU_WDATA_0|
|                    |          |               |                 | ADDR_3 + we=0 → IDLE          |
| TX_CPU_WDATA_0-3   | 1        | 0             | 0               | (size-dependent transitions)  |
| TX_HOST_HEADER     | 1        | 0             | 0               | we=1 → IDLE                   |
|                    |          |               |                 | we=0 → TX_HOST_DATA_0         |
| TX_HOST_DATA_0-3   | 1        | 0             | 0               | (size-dependent transitions)  |

#### 7.2.3 Data Path Registers

```systemverilog
// Captured CPU request
logic [31:0] cpu_req_addr_reg;
logic [31:0] cpu_req_wdata_reg;
logic        cpu_req_we_reg;
logic [1:0]  cpu_req_size_reg;

// Captured host response
logic [31:0] host_resp_rdata_reg;
logic        host_resp_we_reg;
logic [1:0]  host_resp_size_reg;

// Packet type flag (set during capture, cleared on completion)
logic is_cpu_packet;   // 1=CPU request (type 0000), 0=host response (type 0011)
```

#### 7.2.4 Byte Serialization Mux

```systemverilog
always_comb begin
    tx_data = 8'h00;
    
    case (state)
        TX_CPU_HEADER:    tx_data = {4'b0000, cpu_req_size_reg, 1'b0, cpu_req_we_reg};
        TX_CPU_ADDR_0:    tx_data = cpu_req_addr_reg[7:0];
        TX_CPU_ADDR_1:    tx_data = cpu_req_addr_reg[15:8];
        TX_CPU_ADDR_2:    tx_data = cpu_req_addr_reg[23:16];
        TX_CPU_ADDR_3:    tx_data = cpu_req_addr_reg[31:24];
        TX_CPU_WDATA_0:   tx_data = cpu_req_wdata_reg[7:0];
        TX_CPU_WDATA_1:   tx_data = cpu_req_wdata_reg[15:8];
        TX_CPU_WDATA_2:   tx_data = cpu_req_wdata_reg[23:16];
        TX_CPU_WDATA_3:   tx_data = cpu_req_wdata_reg[31:24];
        
        TX_HOST_HEADER:   tx_data = {4'b0011, host_resp_size_reg, 1'b0, host_resp_we_reg};
        TX_HOST_DATA_0:   tx_data = host_resp_rdata_reg[7:0];
        TX_HOST_DATA_1:   tx_data = host_resp_rdata_reg[15:8];
        TX_HOST_DATA_2:   tx_data = host_resp_rdata_reg[23:16];
        TX_HOST_DATA_3:   tx_data = host_resp_rdata_reg[31:24];
        
        default:          tx_data = 8'h00;
    endcase
end
```

---

## 8. Migration Plan

### 8.1 Phase 1: Create New Modules (No Integration)

**Goal**: Implement and test `host_bus_rx` and `host_bus_tx` in isolation.

**Tasks**:
1. Create `rtl/io/host_bus_rx.sv`:
   - Implement FSM per Section 7.1
   - Implement byte accumulation logic
   - Implement ready/valid handshaking
   - Add simulation-only assertions for protocol checks

2. Create `rtl/io/host_bus_tx.sv`:
   - Implement FSM per Section 7.2
   - Implement byte serialization logic
   - Implement priority arbitration (CPU > host)
   - Add simulation-only assertions

3. Create standalone testbenches:
   - `testbench/host_bus_rx_tb.sv`:
     - Test case 1: Receive type 0001 response (byte/half/word reads)
     - Test case 2: Receive type 0010 request (read/write, all sizes)
     - Test case 3: Backpressure (buffer full, rx_ready=0)
     - Test case 4: Unknown packet type (dropped)
   
   - `testbench/host_bus_tx_tb.sv`:
     - Test case 1: Transmit CPU request (read/write, all sizes)
     - Test case 2: Transmit host response (read/write, all sizes)
     - Test case 3: Priority arbitration (CPU request pre-empts host response)
     - Test case 4: TX backpressure (tx_ready=0 stalls FSM)

**Acceptance Criteria**:
- [ ] All testbench test cases pass
- [ ] Verilator lint clean (no warnings)
- [ ] Assertions check ready/valid protocol compliance
- [ ] Waveform inspection confirms correct byte ordering (little-endian)

**Estimated Effort**: 3-4 days

---

### 8.2 Phase 2: Refactor `host_bus_interface` (Orchestrator Only)

**Goal**: Replace existing FSM with orchestration logic, integrate new modules.

**Tasks**:
1. Create backup: `rtl/io/host_bus_interface_v1.sv` (preserve original)

2. Modify `rtl/io/host_bus_interface.sv`:
   - Remove all FSM states (keep module ports unchanged)
   - Remove `host_rx_buffer` instantiation
   - Instantiate `host_bus_rx` module
   - Instantiate `host_bus_tx` module
   - Implement orchestration logic per Section 5

3. File-level changes:
   ```
   REMOVE: rtl/io/host_rx_buffer.sv (deprecated)
   MODIFY: rtl/io/host_bus_interface.sv (refactor)
   ADD:    rtl/io/host_bus_rx.sv (new)
   ADD:    rtl/io/host_bus_tx.sv (new)
   ```

4. Update build scripts:
   - Remove `host_rx_buffer.sv` from source lists
   - Add `host_bus_rx.sv` and `host_bus_tx.sv`

**Acceptance Criteria**:
- [ ] Top-level module interface unchanged (drop-in replacement)
- [ ] Verilator lint clean
- [ ] Simulation builds without errors

**Estimated Effort**: 1-2 days

---

### 8.3 Phase 3: Functional Verification

**Goal**: Verify end-to-end behavior matches original implementation.

**Tasks**:
1. Update existing testbench `testbench/host_bus_interface_tb.sv`:
   - Re-run all test cases from original implementation
   - Verify identical behavior (byte-for-byte packet comparison)

2. Create new system-level tests:
   - `sim-tests/host_bus_refactor_validation.rs`:
     - Test CPU-initiated transactions (read/write, all sizes)
     - Test host-initiated transactions (read/write, all sizes)
     - Test simultaneous requests (verify mutual exclusion)
     - Test backpressure scenarios (stall at each interface)
     - Compare against golden reference (captured from v1)

3. Waveform comparison:
   - Capture VCD from v1 (original) for reference workload
   - Capture VCD from v2 (refactored) for same workload
   - Compare key signals: tx_data, rx_data, ready, valid, req, addr, wdata, rdata

**Acceptance Criteria**:
- [ ] All original test cases pass
- [ ] New test cases pass
- [ ] No functional regressions detected
- [ ] Waveform comparison shows equivalent behavior

**Estimated Effort**: 2-3 days

---

### 8.4 Phase 4: Cleanup and Documentation

**Goal**: Remove deprecated code, update documentation.

**Tasks**:
1. Remove deprecated files:
   - Delete `rtl/io/host_rx_buffer.sv`
   - Delete `rtl/io/host_bus_interface_v1.sv` (backup)

2. Update documentation:
   - Update `rtl/io/README.md` (if exists) with new architecture
   - Update `docs/host_bus_protocol.md` (if exists) with module descriptions
   - Add inline comments to orchestrator logic in `host_bus_interface.sv`

3. Update CI/CD:
   - Verify all CI linting steps pass (Verilator, sv-parser)
   - Verify synthesis targets build (if applicable)
   - Verify FPGA bitstream generation (if applicable)

**Acceptance Criteria**:
- [ ] No references to deprecated modules in codebase
- [ ] Documentation reflects new architecture
- [ ] CI pipeline green

**Estimated Effort**: 1 day

---

### 8.5 File-by-File Change Summary

| File                             | Action   | Description                                      |
|----------------------------------|----------|--------------------------------------------------|
| `rtl/io/host_bus_rx.sv`          | CREATE   | New RX module with integrated buffering          |
| `rtl/io/host_bus_tx.sv`          | CREATE   | New TX module with dual input ports              |
| `rtl/io/host_bus_interface.sv`   | REFACTOR | Remove FSM, add orchestration logic              |
| `rtl/io/host_rx_buffer.sv`       | DELETE   | Deprecated (functionality moved to host_bus_rx)  |
| `testbench/host_bus_rx_tb.sv`    | CREATE   | Standalone testbench for RX module               |
| `testbench/host_bus_tx_tb.sv`    | CREATE   | Standalone testbench for TX module               |
| `testbench/host_bus_interface_tb.sv` | UPDATE | Add regression tests for refactored version  |
| `sim-tests/host_bus_*.rs`        | CREATE   | Rust-based system-level validation tests         |

---

## 9. Verification and Test Plan

### 9.1 Unit Tests (RTL Simulation)

#### 9.1.1 `host_bus_rx_tb.sv`

**Test 1: Type 0001 Response Packet Reception**
- Send: `[0x10][0xAA][0xBB][0xCC][0xDD]` (word read response)
- Assert: `resp_valid=1`, `resp_we=0`, `resp_size=2'b10`, `resp_rdata=32'hDDCCBBAA`

**Test 2: Type 0010 Request Packet Reception**
- Send: `[0x25][0x04][0x03][0x02][0x01][0x77]` (byte write request to 0x01020304)
- Assert: `req_valid=1`, `req_we=1`, `req_size=2'b00`, `req_addr=32'h01020304`, `req_wdata=32'h00000077`

**Test 3: Backpressure**
- Send response packet without pulsing `resp_consumed`
- Assert: `rx_ready=0` after packet complete
- Send second packet → assert: NOT accepted (rx_ready=0)
- Pulse `resp_consumed`
- Assert: `rx_ready=1`, second packet now accepted

**Test 4: Unknown Packet Type**
- Send: `[0xF0][0x00]...` (type 1111, unknown)
- Assert: Packet silently dropped, FSM returns to IDLE
- Assert: `resp_valid=0`, `req_valid=0`

**Test 5: Variable-Length Packets**
- Send byte read response: `[0x10][0xAA]` (2 bytes)
- Send half read response: `[0x14][0xAA][0xBB]` (3 bytes)
- Send word read response: `[0x18][0xAA][0xBB][0xCC][0xDD]` (5 bytes)
- Assert: Correct data captured, FSM completes at correct byte count

#### 9.1.2 `host_bus_tx_tb.sv`

**Test 1: CPU Request Transmission**
- Drive: `cpu_req_valid=1`, addr=0x12345678, wdata=0xAABBCCDD, we=1, size=2'b10
- Assert: `cpu_req_ready=1` for 1 cycle (capture)
- Monitor: TX stream produces `[0x08][0x78][0x56][0x34][0x12][0xDD][0xCC][0xBB][0xAA]`

**Test 2: Host Response Transmission**
- Drive: `host_resp_valid=1`, rdata=0x11223344, we=0, size=2'b10
- Assert: `host_resp_ready=1` for 1 cycle (capture)
- Monitor: TX stream produces `[0x38][0x44][0x33][0x22][0x11]`

**Test 3: Priority Arbitration**
- Drive: `host_resp_valid=1` (waiting for TX)
- Drive: `cpu_req_valid=1` (higher priority)
- Assert: CPU request serviced first
- Assert: Host response serviced after CPU packet complete

**Test 4: TX Backpressure**
- Drive: `cpu_req_valid=1`, `tx_ready=0`
- Assert: `cpu_req_ready=1` (request accepted despite TX stall)
- Assert: FSM stalls at first byte (tx_valid=1, tx_data=header)
- Drive: `tx_ready=1`
- Assert: FSM proceeds through all bytes

### 9.2 Integration Tests (System-Level Simulation)

#### 9.2.1 Rust-Based Tests (`sim-tests/`)

**Test 1: CPU Read Transaction**
```rust
// Scenario: CPU reads from host-mapped address
cpu.read_word(0x80001000);  // Triggers type 0000 packet
// Monitor: host receives [0x08][0x00][0x10][0x00][0x80]
host.send_response(0xDEADBEEF);  // Type 0001 packet
// Assert: cpu transaction completes with rdata=0xDEADBEEF
```

**Test 2: Host Write Transaction**
```rust
// Scenario: Host writes to target peripheral
host.write_byte(0x50001234, 0xAA);  // Triggers type 0010 packet
// Monitor: target receives write request
// Monitor: host receives type 0011 response (write ack)
// Assert: No deadlock, transaction completes
```

**Test 3: Simultaneous Requests (Mutual Exclusion)**
```rust
// Scenario: CPU and host both attempt transactions
let cpu_future = cpu.read_word(0x80000000);
let host_future = host.write_word(0x50000000, 0x12345678);
// Assert: Both complete successfully (sequentialized by arbiter)
// Assert: No data corruption or packet interleaving
```

**Test 4: Backpressure Propagation**
```rust
// Scenario: TX UART FIFO full
uart.set_tx_fifo_size(4);  // Limit to 4 bytes
cpu.write_word(0x80000000, 0xAABBCCDD);  // 9-byte packet
// Monitor: cpu.ready=0 after 4 bytes transmitted
// Monitor: cpu.ready=1 after FIFO drains
// Assert: Transaction eventually completes
```

### 9.3 Lint and Synthesis Checks

**Verilator Lint**:
```bash
verilator --lint-only -Wall rtl/io/host_bus_rx.sv
verilator --lint-only -Wall rtl/io/host_bus_tx.sv
verilator --lint-only -Wall rtl/io/host_bus_interface.sv
```
Expected: 0 warnings, 0 errors

**Yosys Synthesis Check**:
```bash
yosys -p "read_verilog -sv rtl/io/host_bus_rx.sv; synth -top host_bus_rx"
yosys -p "read_verilog -sv rtl/io/host_bus_tx.sv; synth -top host_bus_tx"
```
Expected: No combinational loops, no latches (except intended state registers)

### 9.4 Regression Testing

**Baseline Capture**:
1. Run full test suite against `host_bus_interface_v1.sv` (original)
2. Capture VCD files for all test cases
3. Extract packet byte sequences as golden reference

**Regression Check**:
1. Run identical test suite against refactored `host_bus_interface.sv`
2. Compare VCD signals: `tx_data`, `rx_data`, `ready`, `valid`
3. Compare packet byte sequences against golden reference

**Pass Criteria**: Byte-for-byte identical packet sequences for all test cases

---

## 10. Risks and Mitigations

### 10.1 Risk: Ready/Valid Protocol Deadlock

**Description**: Improper ready/valid dependencies could cause combinational loops or deadlock.

**Likelihood**: Medium  
**Impact**: High (system hang)

**Mitigation**:
- Follow canonical ready/valid rules (Section 6.1)
- Use registered ready signals (no combinational dependencies on valid)
- Add simulation assertions to detect protocol violations:
  ```systemverilog
  assert property (@(posedge clk) $rose(valid) |=> valid throughout !ready [*])
      else $error("Valid de-asserted before handshake");
  ```
- Formal verification of handshake protocol (optional)

---

### 10.2 Risk: Functional Regression

**Description**: Refactored design behaves differently from original, breaking existing software.

**Likelihood**: Medium  
**Impact**: High (requires rollback)

**Mitigation**:
- Preserve original `host_bus_interface_v1.sv` as backup
- Run comprehensive regression suite (Section 9.4)
- Use packet-level comparison (not just cycle-accurate)
- Staged deployment: simulate → FPGA test → production

---

### 10.3 Risk: Timing Closure Failure (FPGA)

**Description**: New design has longer combinational paths, fails timing at target frequency.

**Likelihood**: Low (similar logic depth to original)  
**Impact**: Medium (requires optimization)

**Mitigation**:
- Keep critical paths registered (no long combinational chains)
- RX path: Register output valid/data (already planned)
- TX path: Register captured inputs (already planned)
- If timing fails: Add pipeline stage in byte mux (1 cycle latency)

**Contingency**: If timing cannot be met, revert to original design.

---

### 10.4 Risk: Increased Resource Usage

**Description**: Separate RX/TX modules use more LUTs/FFs than monolithic design.

**Likelihood**: Low (removing dual buffering offsets added logic)  
**Impact**: Low (host_bus_interface is small fraction of total FPGA)

**Expected Change**:
- RX module: +50 LUTs, +64 FFs (vs. host_rx_buffer)
- TX module: +80 LUTs, +96 FFs (new)
- Orchestrator: -120 LUTs, -64 FFs (FSM removed)
- **Net**: ~+10 LUTs, +96 FFs (acceptable)

**Mitigation**:
- Monitor synthesis reports during Phase 2
- If resources exceed budget: optimize byte mux logic (share between states)

---

### 10.5 Risk: Incomplete Test Coverage

**Description**: New edge cases not covered by existing tests, bugs discovered post-deployment.

**Likelihood**: Medium  
**Impact**: Medium (requires patch)

**Mitigation**:
- Code coverage analysis on testbenches (aim for >95% state/transition coverage)
- Add targeted tests for corner cases:
  - Back-to-back packets (no idle cycles between)
  - Interleaved backpressure (tx_ready toggles during packet)
  - Reset during mid-packet (verify clean recovery)
- Fuzz testing: Generate random valid/invalid packet sequences

---

## 11. Acceptance Criteria and Checklist

### 11.1 Functional Correctness

- [ ] **F1**: All `host_bus_rx_tb` test cases pass (Section 9.1.1)
- [ ] **F2**: All `host_bus_tx_tb` test cases pass (Section 9.1.2)
- [ ] **F3**: All integration tests pass (Section 9.2.1)
- [ ] **F4**: Regression suite shows byte-for-byte identical packet output (Section 9.4)
- [ ] **F5**: No functional differences detected in waveform comparison

### 11.2 Code Quality

- [ ] **Q1**: Verilator lint clean (0 warnings, 0 errors)
- [ ] **Q2**: Synthesis checks pass (no latches, no combinational loops)
- [ ] **Q3**: Inline comments explain non-obvious logic
- [ ] **Q4**: Module interfaces documented with signal descriptions
- [ ] **Q5**: Assertions added for ready/valid protocol checks

### 11.3 Test Coverage

- [ ] **T1**: RTL code coverage >95% (state coverage, transition coverage)
- [ ] **T2**: All packet types tested (0000, 0001, 0010, 0011)
- [ ] **T3**: All size variants tested (byte, half, word)
- [ ] **T4**: Backpressure tested at all interfaces (rx, tx, bus master, bus slave)
- [ ] **T5**: Unknown packet type handling tested (dropped gracefully)

### 11.4 Documentation

- [ ] **D1**: This plan document finalized and reviewed
- [ ] **D2**: Module-level comments updated in all modified files
- [ ] **D3**: Architecture diagram added to `docs/` (Section 3.1)
- [ ] **D4**: Migration guide added for future developers

### 11.5 Integration

- [ ] **I1**: All source files build without errors
- [ ] **I2**: Deprecated files removed from repository
- [ ] **I3**: CI/CD pipeline green (all stages pass)
- [ ] **I4**: FPGA bitstream builds successfully (if applicable)
- [ ] **I5**: No timing violations reported (if applicable)

### 11.6 Risk Mitigation

- [ ] **R1**: Original design backed up as `host_bus_interface_v1.sv`
- [ ] **R2**: Rollback procedure documented
- [ ] **R3**: Resource usage within budget (FPGA utilization report)
- [ ] **R4**: Timing reports reviewed (if applicable)
- [ ] **R5**: No deadlock scenarios detected in simulation

---

## 12. Appendix: Protocol Packet Examples

### 12.1 CPU Read Request (Type 0000)

```
Packet: CPU reads word from address 0x80001000

Byte stream (little-endian):
  [0]  0x08   Header: {type=0000, size=10 (word), res=0, we=0}
  [1]  0x00   Address[7:0]
  [2]  0x10   Address[15:8]
  [3]  0x00   Address[23:16]
  [4]  0x80   Address[31:24]

Total: 5 bytes
```

### 12.2 Host Response to CPU (Type 0001)

```
Packet: Host returns word data 0xDEADBEEF

Byte stream (little-endian):
  [0]  0x18   Header: {type=0001, size=10 (word), res=0, we=0}
  [1]  0xEF   Data[7:0]
  [2]  0xBE   Data[15:8]
  [3]  0xAD   Data[23:16]
  [4]  0xDE   Data[31:24]

Total: 5 bytes
```

### 12.3 Host-Initiated Write (Type 0010)

```
Packet: Host writes half-word 0x1234 to address 0x50001000

Byte stream (little-endian):
  [0]  0x25   Header: {type=0010, size=01 (half), res=0, we=1}
  [1]  0x00   Address[7:0]
  [2]  0x10   Address[15:8]
  [3]  0x00   Address[23:16]
  [4]  0x50   Address[31:24]
  [5]  0x34   Data[7:0]
  [6]  0x12   Data[15:8]

Total: 7 bytes
```

### 12.4 FPGA Response to Host (Type 0011)

```
Packet: FPGA acknowledges write (no data)

Byte stream:
  [0]  0x35   Header: {type=0011, size=01 (half), res=0, we=1}

Total: 1 byte
```

---

## 13. Revision History

| Version | Date       | Author | Changes                              |
|---------|------------|--------|--------------------------------------|
| 1.0     | 2025-01-XX | AI     | Initial plan document                |

---

**End of Document**

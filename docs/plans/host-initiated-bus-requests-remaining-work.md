# Host-Initiated Bus Requests - Remaining Implementation Work

**Author:** AI Documentation Review  
**Date:** February 2, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Status:** Active Implementation Plan  
**Parent Document:** `host-initiated-bus-requests-implementation-plan.md`

---

> **⚠️ IMPORTANT NOTE FOR AI AGENTS:**  
> This is a **high-context, detailed implementation plan**. To preserve your context budget, **DO NOT read the original parent plan** (`host-initiated-bus-requests-implementation-plan.md`) unless strictly necessary for resolving ambiguities. This document contains all the implementation details needed for the remaining work.

---

## Executive Summary

This document details the **remaining implementation work** for host-initiated bus requests feature. The foundational infrastructure has been completed (bus arbiter, wiring), but the core bidirectional communication logic in both RTL and Rust remains to be implemented.

### What's Already Implemented ✅

1. **RTL Infrastructure:**
   - `bus_arbiter.sv` module (complete with priority arbitration: Host > CPU)
   - Bus arbiter wired into `top.sv` correctly
   - `host_bus_interface.sv` has bus master interface ports defined (currently unused)
   - Extended header format with packet type bits (0000, 0001) for CPU→Host direction

2. **Tests:**
   - `testbench/tests/bus_arbiter_test.rs` (standalone arbiter test)
   - `testbench/tests/host_bus_interface_test.rs` (CPU→Host direction only)

### What Remains to be Implemented ❌

1. **RTL Changes:**
   - Refactor to dual-FSM architecture (separate RX and TX state machines)
   - RX FSM with request/response buffer assembly logic
   - TX FSM for handling CPU requests and host responses
   - Address validation for host requests (RTL peripheral range only)
   - Error response states and logic
   - **Critical Fix:** Implement parallel RX/TX operation (see Required Changes below)

2. **Rust Changes:**
   - `send_bus_request()` method in `cpu-sim/src/sim.rs`
   - `receive_bus_response()` method in `cpu-sim/src/sim.rs`
   - Host bus state machine for bidirectional traffic
   - Non-blocking RX handling to prevent deadlocks
   - Address validation for host requests
   - New types: `HostBusRequest`, `HostBusResponse`, `FpgaError`

3. **Testing:**
   - Bidirectional test case for `host_bus_interface_test.rs`
   - CPU-sim integration tests for host-initiated requests
   - End-to-end test (Host reads/writes LED peripheral)

---

## Required Changes: Dual-FSM Architecture for Full-Duplex Operation

### Problem Description

The current `host_bus_interface.sv` requires fundamental architectural changes to support true full-duplex bidirectional communication:

1. **Single-FSM Limitation:** The current design uses a single state machine that handles both TX (CPU→Host) and RX (Host→CPU) paths sequentially. This prevents parallel operation and creates deadlock scenarios.

2. **Missing RX Buffering:** The current design has no buffering for incoming host-initiated requests or CPU responses. Without buffering:
   - Incoming host request bytes are lost if they arrive while the FPGA is transmitting a CPU request
   - Incoming CPU response bytes are lost if they arrive while processing a host request
   - UART has no flow control in real FPGA, so the host cannot pause transmission
   - The design cannot support true full-duplex operation

### Current Code (Incorrect)

```systemverilog
// Single FSM handling both TX and RX sequentially
STATE_IDLE: begin
    if (req) begin
        next_state = STATE_CAPTURE;  // Only handles CPU requests
    end
end
```

### Required Fix (Summary)

**Dual-FSM Architecture:**

The design must be refactored into **two independent state machines**:

1. **RX FSM** (Host → FPGA direction):
   - Continuously accepts incoming bytes from UART unless **both** pending request and pending response buffers are full
   - Assembles bytes into separate request/response buffers based on packet header
   - When complete request buffer ready: asserts `rx_request_available`
   - When complete response buffer ready: asserts `rx_response_available`
   - **Buffer Priority:** Response must be processed first when both buffered (unlocks arbitration, then processes buffered request)

2. **TX FSM** (FPGA → Host direction):
   - Handles CPU-initiated requests (reads `req` signal)
   - Handles host-initiated responses (reads `rx_request_available`)
   - Arbitrates between CPU and host response sources
   - Transmits packets independently of RX FSM

**Key Behavior:**
- `rx_request` buffer feeds bus master signals (`host_bus_req`, `host_bus_addr`, etc.)
- `rx_response` buffer resolves outgoing host requests waiting for response (from CPU transactions)
- Both FSMs operate in parallel, enabling true full-duplex communication

**Full implementation details are provided in Task 1 and the "Dual-FSM Architecture" section below.**

---

## Detailed Implementation Tasks

### Task 1: RTL Dual-FSM Architecture

**File:** `rtl/host_bus_interface.sv`

#### 1.1 Add Dual State Machines

Replace the single state machine with two independent FSMs:

```systemverilog
// RX FSM: Assembles incoming packets into request/response buffers
typedef enum logic [3:0] {
    RX_IDLE          = 4'd0,   // Waiting for packet header
    RX_REQ_ADDR_0    = 4'd1,   // Assembling host request: addr[7:0]
    RX_REQ_ADDR_1    = 4'd2,   // addr[15:8]
    RX_REQ_ADDR_2    = 4'd3,   // addr[23:16]
    RX_REQ_ADDR_3    = 4'd4,   // addr[31:24]
    RX_REQ_WDATA_0   = 4'd5,   // wdata[7:0] (writes only)
    RX_REQ_WDATA_1   = 4'd6,   // wdata[15:8]
    RX_REQ_WDATA_2   = 4'd7,   // wdata[23:16]
    RX_REQ_WDATA_3   = 4'd8,   // wdata[31:24]
    RX_RESP_RDATA_0  = 4'd9,   // Assembling CPU response: rdata[7:0]
    RX_RESP_RDATA_1  = 4'd10,  // rdata[15:8]
    RX_RESP_RDATA_2  = 4'd11,  // rdata[23:16]
    RX_RESP_RDATA_3  = 4'd12   // rdata[31:24]
} rx_state_t;

// TX FSM: Transmits CPU requests and host responses
typedef enum logic [4:0] {
    TX_IDLE          = 5'd0,   // Waiting for CPU req or host response ready
    TX_CPU_HEADER    = 5'd1,   // Send CPU request header (type 0000)
    TX_CPU_ADDR_0    = 5'd2,   // addr[7:0]
    TX_CPU_ADDR_1    = 5'd3,   // addr[15:8]
    TX_CPU_ADDR_2    = 5'd4,   // addr[23:16]
    TX_CPU_ADDR_3    = 5'd5,   // addr[31:24]
    TX_CPU_WDATA_0   = 5'd6,   // wdata[7:0]
    TX_CPU_WDATA_1   = 5'd7,   // wdata[15:8]
    TX_CPU_WDATA_2   = 5'd8,   // wdata[23:16]
    TX_CPU_WDATA_3   = 5'd9,   // wdata[31:24]
    TX_CPU_RX_HEADER = 5'd10,  // Wait for CPU response header
    TX_CPU_RX_RDATA  = 5'd11,  // Wait for CPU response data (handled by RX FSM)
    TX_HOST_HEADER   = 5'd12,  // Send host response header (type 0011)
    TX_HOST_RDATA_0  = 5'd13,  // Send rdata[7:0] (reads)
    TX_HOST_RDATA_1  = 5'd14,  // rdata[15:8]
    TX_HOST_RDATA_2  = 5'd15,  // rdata[23:16]
    TX_HOST_RDATA_3  = 5'd16,  // rdata[31:24]
    TX_HOST_ERROR    = 5'd17,  // Send error header (type 1111)
    TX_HOST_ERROR_CODE = 5'd18 // Send error code byte
} tx_state_t;

rx_state_t rx_state, rx_next_state;
tx_state_t tx_state, tx_next_state;
```

#### 1.2 Add Request/Response Buffers

Add registers for pending request and response buffers:

```systemverilog
// Request buffer (assembled by RX FSM from Host→FPGA packets, type 0010)
logic [31:0] req_buf_addr;
logic [31:0] req_buf_wdata;
logic        req_buf_we;
logic [1:0]  req_buf_size;
logic        req_buf_valid;       // Buffer full and ready to process

// Response buffer (assembled by RX FSM from CPU response packets, type 0001/0011)
logic [31:0] resp_buf_rdata;
logic        resp_buf_we;
logic [1:0]  resp_buf_size;
logic        resp_buf_valid;      // Buffer full and ready to process

// Host response data (from bus transaction)
logic [31:0] host_resp_rdata;
logic        host_resp_error;

// Buffer management
logic        req_buf_consume;     // Clear req_buf_valid when processed
logic        resp_buf_consume;    // Clear resp_buf_valid when processed
```

**Buffer control logic:**

```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        req_buf_valid  <= 1'b0;
        resp_buf_valid <= 1'b0;
    end else begin
        // Set valid when RX FSM completes buffer assembly
        if (rx_state == RX_REQ_WDATA_3 || 
            (rx_state == RX_REQ_ADDR_3 && !req_buf_we)) begin
            req_buf_valid <= 1'b1;
        end else if (req_buf_consume) begin
            req_buf_valid <= 1'b0;
        end
        
        if (rx_state == RX_RESP_RDATA_3 ||
            (rx_state == RX_RESP_RDATA_0 && resp_buf_size == 2'b00)) begin
            resp_buf_valid <= 1'b1;
        end else if (resp_buf_consume) begin
            resp_buf_valid <= 1'b0;
        end
    end
end
```

#### 1.3 Implement RX FSM Logic

**RX FSM:** Continuously accepts incoming bytes and assembles into request/response buffers:

```systemverilog
// RX ready signal: accept bytes unless both buffers are full
assign rx_ready = !(req_buf_valid && resp_buf_valid);

always_comb begin
    rx_next_state = rx_state;
    
    case (rx_state)
        RX_IDLE: begin
            if (rx_valid && rx_ready) begin
                // Decode packet type from header
                case (rx_data[7:4])
                    4'b0010: begin  // Host→FPGA request
                        if (!req_buf_valid) begin  // Only if buffer available
                            req_buf_we   = rx_data[0];
                            req_buf_size = rx_data[3:2];
                            rx_next_state = RX_REQ_ADDR_0;
                        end
                    end
                    4'b0001, 4'b0011: begin  // CPU response (write ack or read data)
                        if (!resp_buf_valid) begin  // Only if buffer available
                            resp_buf_we   = rx_data[0];
                            resp_buf_size = rx_data[3:2];
                            if (rx_data[0]) begin
                                // Write ack: header only, complete immediately
                                resp_buf_valid = 1'b1;
                            end else begin
                                // Read response: expect data bytes
                                rx_next_state = RX_RESP_RDATA_0;
                            end
                        end
                    end
                endcase
            end
        end
        
        // Request buffer assembly (see section 1.4 for full implementation)
        // Response buffer assembly (see section 1.5 for full implementation)
    endcase
end
```

**Important:** RX FSM runs **independently** of TX FSM. It continuously accepts bytes as long as at least one buffer is available.

#### 1.4 Implement RX FSM Request Buffer Assembly

RX FSM assembles incoming bytes into request buffer:

```systemverilog
// Request buffer assembly states (within RX FSM always_comb block)
RX_REQ_ADDR_0: begin
    if (rx_valid && rx_ready) begin
        req_buf_addr[7:0] = rx_data;
        rx_next_state = RX_REQ_ADDR_1;
    end
end

RX_REQ_ADDR_1: begin
    if (rx_valid && rx_ready) begin
        req_buf_addr[15:8] = rx_data;
        rx_next_state = RX_REQ_ADDR_2;
    end
end

RX_REQ_ADDR_2: begin
    if (rx_valid && rx_ready) begin
        req_buf_addr[23:16] = rx_data;
        rx_next_state = RX_REQ_ADDR_3;
    end
end

RX_REQ_ADDR_3: begin
    if (rx_valid && rx_ready) begin
        req_buf_addr[31:24] = rx_data;
        if (req_buf_we) begin
            rx_next_state = RX_REQ_WDATA_0;  // Write: get data
        end else begin
            // Read: buffer complete, set req_buf_valid
            rx_next_state = RX_IDLE;
        end
    end
end

// Write data assembly (little-endian)
RX_REQ_WDATA_0: begin
    if (rx_valid && rx_ready) begin
        req_buf_wdata[7:0] = rx_data;
        if (req_buf_size == 2'b00) begin
            rx_next_state = RX_IDLE;  // Byte: done, set req_buf_valid
        end else begin
            rx_next_state = RX_REQ_WDATA_1;
        end
    end
end

RX_REQ_WDATA_1: begin
    if (rx_valid && rx_ready) begin
        req_buf_wdata[15:8] = rx_data;
        if (req_buf_size == 2'b01) begin
            rx_next_state = RX_IDLE;  // Half: done
        end else begin
            rx_next_state = RX_REQ_WDATA_2;
        end
    end
end

RX_REQ_WDATA_2: begin
    if (rx_valid && rx_ready) begin
        req_buf_wdata[23:16] = rx_data;
        rx_next_state = RX_REQ_WDATA_3;
    end
end

RX_REQ_WDATA_3: begin
    if (rx_valid && rx_ready) begin
        req_buf_wdata[31:24] = rx_data;
        rx_next_state = RX_IDLE;  // Buffer complete, set req_buf_valid
    end
end
```

#### 1.5 Implement RX FSM Response Buffer Assembly

RX FSM assembles incoming CPU response bytes into response buffer:

```systemverilog
// Response buffer assembly states (within RX FSM always_comb block)
RX_RESP_RDATA_0: begin
    if (rx_valid && rx_ready) begin
        resp_buf_rdata[7:0] = rx_data;
        if (resp_buf_size == 2'b00) begin
            rx_next_state = RX_IDLE;  // Byte: done, set resp_buf_valid
        end else begin
            rx_next_state = RX_RESP_RDATA_1;
        end
    end
end

RX_RESP_RDATA_1: begin
    if (rx_valid && rx_ready) begin
        resp_buf_rdata[15:8] = rx_data;
        if (resp_buf_size == 2'b01) begin
            rx_next_state = RX_IDLE;  // Half: done
        end else begin
            rx_next_state = RX_RESP_RDATA_2;
        end
    end
end

RX_RESP_RDATA_2: begin
    if (rx_valid && rx_ready) begin
        resp_buf_rdata[23:16] = rx_data;
        rx_next_state = RX_RESP_RDATA_3;
    end
end

RX_RESP_RDATA_3: begin
    if (rx_valid && rx_ready) begin
        resp_buf_rdata[31:24] = rx_data;
        rx_next_state = RX_IDLE;  // Buffer complete, set resp_buf_valid
    end
end
```

#### 1.6 Implement TX FSM Logic

**TX FSM:** Handles CPU requests and host responses independently of RX FSM:

```systemverilog
// Address range constants for validation
localparam RTL_PERIPH_BASE  = 32'h5000_0000;
localparam RTL_PERIPH_LIMIT = 32'h6000_0000;

always_comb begin
    tx_next_state = tx_state;
    req_buf_consume = 1'b0;
    resp_buf_consume = 1'b0;
    
    case (tx_state)
        TX_IDLE: begin
            // Priority 1: Process buffered response (unlocks arbitration)
            if (resp_buf_valid) begin
                resp_buf_consume = 1'b1;
                tx_next_state = TX_IDLE;  // Response consumed, back to idle
            end
            // Priority 2: Process buffered host request
            else if (req_buf_valid) begin
                // Validate address
                if (req_buf_addr >= RTL_PERIPH_BASE && 
                    req_buf_addr < RTL_PERIPH_LIMIT) begin
                    // Valid: issue bus request, wait for completion
                    tx_next_state = TX_HOST_BUS_WAIT;
                end else begin
                    // Invalid: send error
                    tx_next_state = TX_HOST_ERROR;
                end
            end
            // Priority 3: CPU-initiated request
            else if (req) begin
                tx_next_state = TX_CPU_HEADER;
            end
        end
        
        // CPU request transmission (see section 1.7)
        // Host response transmission (see section 1.8)
    endcase
end

// Bus request handling state
TX_HOST_BUS_WAIT: begin
    if (host_bus_ready) begin
        // Transaction complete
        if (!req_buf_we) begin
            host_resp_rdata = host_bus_rdata;  // Capture read data
        end
        host_resp_error = 1'b0;
        req_buf_consume = 1'b1;
        tx_next_state = TX_HOST_HEADER;  // Send response
    end
end
```

#### 1.7 Implement TX FSM CPU Request Path

TX FSM handles CPU-initiated requests (existing functionality):

```systemverilog
// CPU request states (within TX FSM always_comb block)
TX_CPU_HEADER: begin
    if (tx_valid && tx_ready) begin
        tx_next_state = TX_CPU_ADDR_0;
    end
end

TX_CPU_ADDR_0, TX_CPU_ADDR_1, TX_CPU_ADDR_2: begin
    if (tx_valid && tx_ready) begin
        tx_next_state = tx_state + 1;  // Sequential progression
    end
end

TX_CPU_ADDR_3: begin
    if (tx_valid && tx_ready) begin
        if (cpu_cap_we) begin
            tx_next_state = TX_CPU_WDATA_0;
        end else begin
            tx_next_state = TX_CPU_RX_HEADER;  // Wait for response
        end
    end
end

TX_CPU_WDATA_0, TX_CPU_WDATA_1, TX_CPU_WDATA_2: begin
    if (tx_valid && tx_ready) begin
        // Check size to determine next state
        // ... (similar to existing logic)
    end
end

TX_CPU_RX_HEADER: begin
    // Wait for RX FSM to assemble response in resp_buf
    if (resp_buf_valid) begin
        resp_buf_consume = 1'b1;
        tx_next_state = TX_IDLE;  // CPU transaction complete
    end
end
```

#### 1.8 Implement TX FSM Host Response Path

TX FSM sends host responses after bus transaction:

```systemverilog
// Host response states (within TX FSM always_comb block)
TX_HOST_HEADER: begin
    if (tx_valid && tx_ready) begin
        if (req_buf_we) begin
            tx_next_state = TX_IDLE;  // Write ack: header only
        end else begin
            tx_next_state = TX_HOST_RDATA_0;  // Read: send data
        end
    end
end

TX_HOST_RDATA_0: begin
    if (tx_valid && tx_ready) begin
        if (req_buf_size == 2'b00) begin
            tx_next_state = TX_IDLE;
        end else begin
            tx_next_state = TX_HOST_RDATA_1;
        end
    end
end

TX_HOST_RDATA_1, TX_HOST_RDATA_2, TX_HOST_RDATA_3: begin
    if (tx_valid && tx_ready) begin
        // Sequential progression with size checks
        // ... (similar to CPU data transmission)
    end
end

// Error response states
TX_HOST_ERROR: begin
    if (tx_valid && tx_ready) begin
        req_buf_consume = 1'b1;
        tx_next_state = TX_HOST_ERROR_CODE;
    end
end

TX_HOST_ERROR_CODE: begin
    if (tx_valid && tx_ready) begin
        tx_next_state = TX_IDLE;
    end
end
```

#### 1.9 Update TX Data Multiplexer and Control Signals

TX data multiplexer based on TX FSM state:

```systemverilog
always_comb begin
    tx_byte = 8'h00;
    
    case (tx_state)
        // CPU→Host TX states
        TX_CPU_HEADER:  tx_byte = {4'b0000, cpu_cap_size, 1'b0, cpu_cap_we};
        TX_CPU_ADDR_0:  tx_byte = cpu_cap_addr[7:0];
        TX_CPU_ADDR_1:  tx_byte = cpu_cap_addr[15:8];
        TX_CPU_ADDR_2:  tx_byte = cpu_cap_addr[23:16];
        TX_CPU_ADDR_3:  tx_byte = cpu_cap_addr[31:24];
        TX_CPU_WDATA_0: tx_byte = cpu_cap_wdata[7:0];
        TX_CPU_WDATA_1: tx_byte = cpu_cap_wdata[15:8];
        TX_CPU_WDATA_2: tx_byte = cpu_cap_wdata[23:16];
        TX_CPU_WDATA_3: tx_byte = cpu_cap_wdata[31:24];
        
        // Host response TX states
        TX_HOST_HEADER:  tx_byte = {4'b0011, req_buf_size, 1'b0, req_buf_we};
        TX_HOST_RDATA_0: tx_byte = host_resp_rdata[7:0];
        TX_HOST_RDATA_1: tx_byte = host_resp_rdata[15:8];
        TX_HOST_RDATA_2: tx_byte = host_resp_rdata[23:16];
        TX_HOST_RDATA_3: tx_byte = host_resp_rdata[31:24];
        
        // Error response TX states
        TX_HOST_ERROR:      tx_byte = {4'b1111, req_buf_size, 1'b0, req_buf_we};
        TX_HOST_ERROR_CODE: tx_byte = 8'hFF;  // Error code: invalid address
        
        default: tx_byte = 8'h00;
    endcase
end

// TX valid: asserted during transmission states
assign tx_valid = (tx_state >= TX_CPU_HEADER && tx_state <= TX_CPU_WDATA_3) ||
                  (tx_state >= TX_HOST_HEADER && tx_state <= TX_HOST_RDATA_3) ||
                  (tx_state == TX_HOST_ERROR) ||
                  (tx_state == TX_HOST_ERROR_CODE);

// RX ready: accept bytes unless both buffers full (see section 1.3)
assign rx_ready = !(req_buf_valid && resp_buf_valid);
```

#### 1.10 Update Bus Master Interface Outputs

Connect the buffered host request to the bus master interface:

```systemverilog
// Bus Master Interface (Host→RTL path)
assign host_bus_addr  = req_buf_addr;
assign host_bus_wdata = req_buf_wdata;
assign host_bus_we    = req_buf_we;
assign host_bus_size  = req_buf_size;

// Assert request only in TX_HOST_BUS_WAIT state
assign host_bus_req = (tx_state == TX_HOST_BUS_WAIT);
```

---

### Task 2: Rust Implementation

**File:** `cpu-sim/src/sim.rs`

#### 2.1 Add New Types

```rust
/// Direction of a bus transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionDirection {
    /// CPU-initiated request to host (existing)
    CpuToHost,
    /// Host-initiated request to FPGA (new)
    HostToFpga,
}

/// Host-initiated bus request
#[derive(Debug, Clone)]
pub struct HostBusRequest {
    pub addr: u32,
    pub wdata: u32,
    pub size: u8,    // 0=byte, 1=half, 2=word
    pub we: bool,
}

/// Response to a host-initiated bus request
#[derive(Debug, Clone)]
pub enum HostBusResponse {
    /// Successful read with data
    ReadData(u32),
    /// Successful write acknowledgement
    WriteAck,
    /// Error response from FPGA
    Error(FpgaError),
}

/// Error codes from FPGA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaError {
    InvalidAddress,
    Timeout,
    ProtocolError,
}
```

#### 2.2 Add State Machine and Queues

In the `SimulatorView` struct:

```rust
/// Host bus state machine for bidirectional communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusHostState {
    Idle,
    TxHeader,
    TxAddr { byte_idx: u8 },
    TxWdata { byte_idx: u8 },
    RxWaitingHeaderOrIncoming,  // Waiting for response, but also processing incoming CPU requests
    RxRdata { byte_idx: u8 },
}

// Add to SimulatorView:
host_bus_request_queue: VecDeque<HostBusRequest>,
host_bus_response_queue: VecDeque<HostBusResponse>,
host_bus_host_state: HostBusHostState,
host_cap_request: Option<HostBusRequest>,
host_resp_rdata: u32,
```

#### 2.3 Implement send_bus_request()

```rust
impl<'a> SimulatorView<'a> {
    /// Queue a host-initiated bus request
    pub fn send_bus_request(&mut self, request: HostBusRequest) -> Result<(), String> {
        // Validate address range
        const RTL_PERIPH_BASE: u32 = 0x5000_0000;
        const RTL_PERIPH_LIMIT: u32 = 0x6000_0000;
        
        if request.addr < RTL_PERIPH_BASE || request.addr >= RTL_PERIPH_LIMIT {
            return Err(format!(
                "Host request address 0x{:08x} outside RTL peripheral range (0x{:08x}-0x{:08x})",
                request.addr, RTL_PERIPH_BASE, RTL_PERIPH_LIMIT
            ));
        }
        
        // Check if request already pending
        if self.host_request_pending() {
            return Err("Host request already pending".to_string());
        }
        
        // Queue the request
        self.host_bus_request_queue.push_back(request);
        Ok(())
    }
    
    /// Check if a host request is pending (waiting for response)
    pub fn host_request_pending(&self) -> bool {
        matches!(
            self.host_bus_host_state,
            HostBusHostState::RxWaitingHeaderOrIncoming | HostBusHostState::RxRdata { .. }
        )
    }
    
    /// Receive response to a host-initiated request
    pub fn receive_bus_response(&mut self) -> Option<HostBusResponse> {
        self.host_bus_response_queue.pop_front()
    }
}
```

#### 2.4 Update handle_host_bus_interface()

**Critical:** The Rust side must handle **bidirectional traffic simultaneously**. This means:
- While waiting for a host response, continue processing incoming CPU requests
- Use packet type bits to distinguish request from response

```rust
fn handle_host_bus_interface(&mut self) {
    // Process TX (to FPGA) if not busy
    self.handle_host_tx();
    
    // Process RX (from FPGA) - ALWAYS check, even if waiting for response
    self.handle_host_rx();
}

fn handle_host_tx(&mut self) {
    // Existing CPU→Host TX logic (unchanged)
    // ...
    
    // NEW: Host→FPGA TX logic
    match self.host_bus_host_state {
        HostBusHostState::Idle => {
            // Start sending if request queued
            if let Some(req) = self.host_bus_request_queue.pop_front() {
                self.host_cap_request = Some(req);
                self.host_bus_host_state = HostBusHostState::TxHeader;
            }
        }
        HostBusHostState::TxHeader => {
            let req = self.host_cap_request.as_ref().unwrap();
            let header = (0b0010 << 4) | (req.size << 2) | (req.we as u8);  // Packet type 0010
            if self.try_send_rx_byte(header) {
                self.host_bus_host_state = HostBusHostState::TxAddr { byte_idx: 0 };
            }
        }
        HostBusHostState::TxAddr { byte_idx } => {
            let req = self.host_cap_request.as_ref().unwrap();
            let byte = ((req.addr >> (byte_idx * 8)) & 0xFF) as u8;
            if self.try_send_rx_byte(byte) {
                if byte_idx == 3 {
                    if req.we {
                        self.host_bus_host_state = HostBusHostState::TxWdata { byte_idx: 0 };
                    } else {
                        self.host_bus_host_state = HostBusHostState::RxWaitingHeaderOrIncoming;
                    }
                } else {
                    self.host_bus_host_state = HostBusHostState::TxAddr { byte_idx: byte_idx + 1 };
                }
            }
        }
        HostBusHostState::TxWdata { byte_idx } => {
            let req = self.host_cap_request.as_ref().unwrap();
            let byte = ((req.wdata >> (byte_idx * 8)) & 0xFF) as u8;
            let num_bytes = 1 << req.size;
            if self.try_send_rx_byte(byte) {
                if byte_idx + 1 >= num_bytes {
                    self.host_bus_host_state = HostBusHostState::RxWaitingHeaderOrIncoming;
                } else {
                    self.host_bus_host_state = HostBusHostState::TxWdata { byte_idx: byte_idx + 1 };
                }
            }
        }
        _ => {}
    }
}

fn handle_host_rx(&mut self) {
    // Check if data available from FPGA
    if let Some(byte) = self.try_read_tx_byte() {
        let packet_type = (byte >> 4) & 0x0F;
        
        match packet_type {
            0b0000 => {
                // CPU-initiated request (existing logic)
                self.handle_cpu_request_header(byte);
            }
            0b0011 => {
                // FPGA response to host request
                self.handle_fpga_response_header(byte);
            }
            0b1111 => {
                // Error response
                self.handle_fpga_error_response(byte);
            }
            _ => {
                eprintln!("Unexpected packet type: {:04b}", packet_type);
            }
        }
    }
}

fn handle_fpga_response_header(&mut self, header: u8) {
    let we = (header & 0x01) != 0;
    let size = (header >> 2) & 0x03;
    
    if we {
        // Write ack: just header, response complete
        self.host_bus_response_queue.push_back(HostBusResponse::WriteAck);
        self.host_cap_request = None;
        self.host_bus_host_state = HostBusHostState::Idle;
    } else {
        // Read response: expect data bytes
        self.host_resp_rdata = 0;
        self.host_bus_host_state = HostBusHostState::RxRdata { byte_idx: 0 };
    }
}

fn handle_fpga_error_response(&mut self, header: u8) {
    // Next byte is error code
    if let Some(error_code) = self.try_read_tx_byte() {
        let error = match error_code {
            0xFF => FpgaError::InvalidAddress,
            0xFE => FpgaError::Timeout,
            _ => FpgaError::ProtocolError,
        };
        self.host_bus_response_queue.push_back(HostBusResponse::Error(error));
        self.host_cap_request = None;
        self.host_bus_host_state = HostBusHostState::Idle;
    }
}
```

---

### Task 3: Testing

#### 3.1 RTL Test: Host Read from LED Peripheral

Add to `testbench/tests/host_bus_interface_test.rs`:

```rust
#[test]
fn test_host_initiated_read() {
    let mut dut = setup_dut();
    
    // Host sends read request to LED peripheral (0x50000000)
    // RX FSM will assemble into req_buf
    // Packet type 0010, size=word (10), we=0 (read)
    let header = 0b0010_10_0_0;  // = 0x28
    
    send_rx_byte(&mut dut, header);
    send_rx_byte(&mut dut, 0x00);  // addr[7:0]
    send_rx_byte(&mut dut, 0x00);  // addr[15:8]
    send_rx_byte(&mut dut, 0x00);  // addr[23:16]
    send_rx_byte(&mut dut, 0x50);  // addr[31:24] = 0x50000000
    
    // Wait for RX FSM to complete buffer assembly
    step(&mut dut);
    assert_eq!(dut.req_buf_valid.get(), 1);  // Request buffer ready
    
    // TX FSM will process req_buf, issue bus request, and send response
    // Response: packet type 0011, size=word, we=0
    assert_tx_byte(&mut dut, 0b0011_10_0_0);  // Response header
    assert_tx_byte(&mut dut, 0x00);  // rdata[7:0]
    assert_tx_byte(&mut dut, 0x00);  // rdata[15:8]
    assert_tx_byte(&mut dut, 0x00);  // rdata[23:16]
    assert_tx_byte(&mut dut, 0x00);  // rdata[31:24]
}
```

#### 3.2 RTL Test: Invalid Address Error

```rust
#[test]
fn test_host_invalid_address() {
    let mut dut = setup_dut();
    
    // Host sends request to invalid address (0x80000000, Host DRAM range)
    // RX FSM assembles into req_buf
    let header = 0b0010_10_0_0;  // Read word
    
    send_rx_byte(&mut dut, header);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x80);  // Invalid: Host address
    
    // Wait for buffer assembly
    step(&mut dut);
    assert_eq!(dut.req_buf_valid.get(), 1);
    
    // TX FSM validates address and sends error
    assert_tx_byte(&mut dut, 0b1111_10_0_0);  // Error header
    assert_tx_byte(&mut dut, 0xFF);           // Error code: invalid address
}
```

#### 3.3 CPU-Sim Test: End-to-End LED Test

Create new file `cpu-sim/tests/test_host_bus_requests.rs`:

```rust
use cpu_sim::{HostBusRequest, HostBusResponse, Simulator};

#[test]
fn test_host_write_led() {
    let mut sim = Simulator::new(/* appropriate config */);
    
    // Start simulation until first instruction completes
    sim.run_until_instruction_complete(|view| {
        // Host writes to LED peripheral
        let request = HostBusRequest {
            addr: 0x50000000,
            wdata: 0xAA,
            size: 0,  // byte
            we: true,
        };
        
        // Send request via SimulatorView
        view.send_bus_request(request).unwrap();
    });
    
    // Continue stepping to allow request to process
    for _ in 0..100 {
        sim.step();
    }
    
    // Check response via the next instruction_complete callback
    sim.run_until_instruction_complete(|view| {
        let response = view.receive_bus_response().expect("Expected response");
        assert!(matches!(response, HostBusResponse::WriteAck));
        
        // Verify LED output changed (access via view)
        assert_eq!(view.read_peripheral_led(), 0xAA);
    });
}

#[test]
fn test_host_read_led() {
    let mut sim = Simulator::new(/* appropriate config */);
    
    // Write a known value to LED first (via CPU or previous host write)
    // ... setup code ...
    
    sim.run_until_instruction_complete(|view| {
        // Host reads from LED peripheral
        let request = HostBusRequest {
            addr: 0x50000000,
            wdata: 0,
            size: 0,  // byte
            we: false,
        };
        
        view.send_bus_request(request).unwrap();
    });
    
    // Step simulation to process request
    for _ in 0..100 {
        sim.step();
    }
    
    // Check read response
    sim.run_until_instruction_complete(|view| {
        let response = view.receive_bus_response().expect("Expected response");
        if let HostBusResponse::ReadData(data) = response {
            assert_eq!(data & 0xFF, 0xAA);  // Verify expected LED value
        } else {
            panic!("Expected ReadData response");
        }
    });
}
```

**Key Testing Pattern:**
- Use `Simulator::run_until_instruction_complete(callback)` to access `SimulatorView`
- The `SimulatorView` provides `send_bus_request()` and `receive_bus_response()` methods
- The `view` parameter in callbacks is the **only** public interface for driving host-initiated bus requests
- All peripheral reads/writes from tests should go through `SimulatorView` methods

---

## Implementation Checklist

### Phase 1: RTL Changes (host_bus_interface.sv)

- [ ] **CRITICAL:** Define dual state machine types: `rx_state_t` and `tx_state_t`
- [ ] Add RX FSM states for request/response buffer assembly
- [ ] Add TX FSM states for CPU requests and host responses
- [ ] Add request buffer registers (`req_buf_addr`, `req_buf_wdata`, `req_buf_valid`, etc.)
- [ ] Add response buffer registers (`resp_buf_rdata`, `resp_buf_valid`, etc.)
- [ ] Implement RX FSM combinational logic (packet type decoding in RX_IDLE)
- [ ] Implement RX FSM request buffer assembly states (RX_REQ_ADDR_*, RX_REQ_WDATA_*)
- [ ] Implement RX FSM response buffer assembly states (RX_RESP_RDATA_*)
- [ ] Implement buffer valid/consume control logic
- [ ] Implement TX FSM combinational logic (TX_IDLE with priority handling)
- [ ] Implement TX FSM CPU request transmission states
- [ ] Implement TX FSM host response transmission states
- [ ] Implement address validation in TX_IDLE (RTL_PERIPH_BASE check)
- [ ] Implement TX_HOST_BUS_WAIT state for bus transaction
- [ ] Implement TX_HOST_ERROR states (header, code)
- [ ] Update TX data mux based on `tx_state`
- [ ] Update `tx_valid` signal based on `tx_state`
- [ ] Update `rx_ready` signal: `!(req_buf_valid && resp_buf_valid)`
- [ ] Update `host_bus_*` output assignments (driven from req_buf)
- [ ] Run verilator lint check

### Phase 2: Rust Changes (cpu-sim/src/sim.rs)

- [ ] Add `HostBusRequest` type
- [ ] Add `HostBusResponse` enum
- [ ] Add `FpgaError` enum
- [ ] Add `HostBusHostState` enum
- [ ] Add state machine fields to `SimulatorView`
- [ ] Implement `send_bus_request()` with address validation
- [ ] Implement `receive_bus_response()`
- [ ] Implement `host_request_pending()`
- [ ] Update `handle_host_bus_interface()` for bidirectional traffic
- [ ] Implement `handle_host_tx()` (send host requests)
- [ ] Update `handle_host_rx()` to decode packet types
- [ ] Implement `handle_fpga_response_header()`
- [ ] Implement `handle_fpga_error_response()`
- [ ] Run `cargo fmt`
- [ ] Run `cargo clippy -- -D warnings`

### Phase 3: Testing

- [ ] **CRITICAL:** Add `test_dual_fsm_parallel_operation` to validate independent RX/TX FSMs
- [ ] Add `test_request_buffering_during_cpu_tx` to verify request buffer assembly during TX
- [ ] Add `test_response_buffering_during_host_request` to verify response buffer assembly
- [ ] Add `test_buffer_priority` to verify response processed before request when both buffered
- [ ] Add `test_host_initiated_read` to `host_bus_interface_test.rs`
- [ ] Add `test_host_initiated_write` to `host_bus_interface_test.rs`
- [ ] Add `test_host_invalid_address` to `host_bus_interface_test.rs`
- [ ] Add `test_both_buffers_full_backpressure` to verify rx_ready deassertion
- [ ] Create `cpu-sim/tests/test_host_bus_requests.rs`
- [ ] Add `test_host_write_led` (end-to-end test using `SimulatorView`)
- [ ] Add `test_host_read_led` (end-to-end test using `SimulatorView`)
- [ ] Run all tests and verify pass

### Phase 4: Documentation

- [ ] Update this plan document with implementation notes
- [ ] Update `AGENTS.md` if needed
- [ ] Mark original plan as "Implementation Complete"

---

## Dual-FSM Architecture Details

### Design Rationale

**Critical Design Requirement:** The FPGA must support **true full-duplex bidirectional communication** using a dual-FSM architecture:

1. **No Flow Control in UART:** In the real FPGA, UART has no hardware flow control mechanism. The host cannot pause transmission when the FPGA is busy.
2. **Data Loss Prevention:** Without buffer-based parallelism, incoming packets would be lost if they arrive while the FPGA is transmitting.
3. **Full-Duplex Operation:** UART is physically full-duplex (separate TX/RX lines), so simultaneous bidirectional communication is expected.
4. **Deadlock Prevention:** Single-FSM designs create deadlocks when both sides wait for responses simultaneously.

### Dual-FSM Architecture

**RX FSM (Independent Receive Path):**
- Continuously accepts incoming bytes from UART via `rx_data`/`rx_valid`/`rx_ready`
- Assembles bytes into **request buffer** (Host→FPGA packets, type 0010)
- Assembles bytes into **response buffer** (CPU response packets, type 0001/0011)
- Asserts `rx_ready = 0` only when **both** buffers are full (backpressure)
- Operates independently of TX FSM state

**TX FSM (Independent Transmit Path):**
- Monitors `req` signal for CPU-initiated requests
- Monitors `req_buf_valid` for buffered host requests ready to process
- Monitors `resp_buf_valid` for buffered CPU responses
- Transmits CPU requests and host responses via `tx_data`/`tx_valid`/`tx_ready`
- Operates independently of RX FSM state

**Key Parallel Operation:**

```
Time    RX FSM State        TX FSM State           Buffer Status
----    ----------------    -------------------    -------------
t=0     RX_IDLE            TX_CPU_ADDR_2          req=empty, resp=empty
t=1     RX_REQ_ADDR_0      TX_CPU_ADDR_3          req=filling, resp=empty
t=2     RX_REQ_ADDR_1      TX_CPU_WDATA_0         req=filling, resp=empty
t=3     RX_REQ_ADDR_2      TX_CPU_WDATA_1         req=filling, resp=empty
t=4     RX_REQ_ADDR_3      TX_CPU_RX_HEADER       req=valid, resp=empty
t=5     RX_IDLE            TX_IDLE                req=valid, resp=empty
t=6     RX_IDLE            TX_HOST_BUS_WAIT       req=processing, resp=empty
t=7     RX_RESP_RDATA_0    TX_HOST_BUS_WAIT       req=processing, resp=filling
t=8     RX_IDLE            TX_HOST_HEADER         req=consuming, resp=valid
```

**Buffer Priority Handling:**

When both buffers are full (`req_buf_valid && resp_buf_valid`):
1. TX FSM **MUST** process response first (unlocks arbitration for pending CPU request)
2. Then process buffered host request (may trigger new bus transaction)
3. This prevents arbitration deadlock

### Implementation Example

**Dual-FSM State Updates:**

```systemverilog
// Separate always_ff blocks for each FSM
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_state <= RX_IDLE;
        tx_state <= TX_IDLE;
    end else begin
        rx_state <= rx_next_state;
        tx_state <= tx_next_state;
    end
end

// RX FSM runs independently
always_comb begin
    rx_next_state = rx_state;
    case (rx_state)
        RX_IDLE: if (rx_valid && rx_ready) /* decode packet type */
        // ... RX state transitions independent of TX FSM ...
    endcase
end

// TX FSM runs independently
always_comb begin
    tx_next_state = tx_state;
    case (tx_state)
        TX_IDLE: begin
            if (resp_buf_valid) /* process response (priority) */
            else if (req_buf_valid) /* process request */
            else if (req) /* CPU request */
        end
        // ... TX state transitions independent of RX FSM ...
    endcase
end
```

### Testing Requirements

All RTL tests must validate dual-FSM parallel operation:

```rust
#[test]
fn test_dual_fsm_parallel_operation() {
    let mut dut = setup_dut();
    
    // Start CPU→Host transaction (TX FSM active)
    dut.req.set(1);
    dut.addr.set(0x50000000);
    step(&mut dut);
    
    // While TX FSM transmits, inject host request (RX FSM assembles)
    for _ in 0..3 {
        step(&mut dut);  // TX progresses: TX_HEADER → TX_ADDR_*
    }
    
    // Send host request bytes (RX FSM assembles into req_buf)
    send_rx_byte(&mut dut, 0b0010_10_0_0);  // Header
    send_rx_byte(&mut dut, 0x00);  // addr[7:0]
    send_rx_byte(&mut dut, 0x00);  // addr[15:8]
    send_rx_byte(&mut dut, 0x00);  // addr[23:16]
    send_rx_byte(&mut dut, 0x51);  // addr[31:24]
    
    // Verify: req_buf_valid should be asserted while TX still active
    assert_eq!(dut.req_buf_valid.get(), 1);
    
    // Wait for CPU TX to complete
    wait_for_state(&mut dut, TX_IDLE);
    
    // TX FSM should now process buffered request
    wait_for_state(&mut dut, TX_HOST_BUS_WAIT);
    
    // Expect host response transmitted
    assert_tx_byte(&mut dut, 0b0011_10_0_0);
}
```

**Why This Is Not Optional:**

The Rust simulator can work around a single-FSM design by carefully sequencing requests, but the **real FPGA deployment cannot**. Host software running on a PC will send UART bytes continuously without knowledge of FPGA internal state. The dual-FSM architecture is the only correct solution for hardware deployment.

---

## References

- Parent Plan: `host-initiated-bus-requests-implementation-plan.md`
- AGENTS.md: RTL peripheral address ranges
- RTL Style Guide: `docs/rtl-style-guide.md` (if exists)
- Rust Coding Standards: Project follows `cargo fmt` and `cargo clippy`

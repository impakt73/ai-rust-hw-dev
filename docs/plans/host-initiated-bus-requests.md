# Host-Initiated Bus Requests Implementation Plan

## 1. Overview

This document describes the implementation plan for upgrading the host bus communication system to support **host-initiated bus requests**. This feature enables the host (Rust simulation/FPGA host) to initiate memory transactions to the target/FPGA via the `host_bus_interface` module, complementing the existing CPU-initiated request path.

### 1.1 Current State

The existing system supports:
- **CPU-initiated requests (type 0000)**: CPU sends requests via `host_tx_*`, host responds via `host_rx_*`
- **Host responses to CPU (type 0001)**: Host sends acknowledgements/read data back to CPU

The master interface in `host_bus_interface` is currently unused (outputs tied to 0).

### 1.2 Target State

Enable bidirectional communication:
- **Host-initiated requests (type 0010)**: Host sends requests via `host_rx_*` to target
- **FPGA responses to host (type 0011)**: Target responds via `host_tx_*` to host

---

## 2. Protocol Specification

### 2.1 Extended Header Format

The 1-byte header format remains consistent with the existing protocol:

```
Bits [7:4]: Packet type
  0000 = CPU-initiated request     (FPGA → Host TX)
  0001 = Host response to CPU      (Host → FPGA RX)
  0010 = Host-initiated request    (Host → FPGA RX)  ← NEW
  0011 = FPGA response to Host     (FPGA → Host TX)  ← NEW
Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
Bit  [1]:   Reserved (0)
Bit  [0]:   we (1=write, 0=read)
```

### 2.2 Packet Formats

**Host-initiated request (type 0010):**
```
[header][addr0][addr1][addr2][addr3][data0]...[dataN]
  1 byte   4 bytes (little-endian)    0-4 bytes (writes only)
```

**FPGA response to host (type 0011):**
```
[header][data0]...[dataN]
  1 byte   0-4 bytes (reads only)
```

---

## 3. Deadlock Prevention Rules

To avoid deadlocks in the bidirectional communication system:

### 3.1 Rule 1: No Self-Routing
Neither side may send a request that would be routed back to itself based on the memory address. The address space must be partitioned:
- **Host-owned addresses**: DRAM (0x80000000+), Rust peripherals
- **Target-owned addresses**: RTL peripherals (0x50000000-0x5FFFFFFF)

### 3.2 Rule 2: Single Outstanding Request
Both sides may only have a single outstanding request at any time.

### 3.3 Rule 3: Host Processes Immediately
The host side must process incoming requests as soon as they are received, even when it has an outstanding request to the target side.

### 3.4 Rule 4: Target Completes Before Processing
The target side must complete outstanding outgoing requests before it processes incoming requests from the host.

### 3.5 Rule 5: Target Must Accept Data
The target side must accept new data from the host even if it has an outstanding host-bound request. This requires buffering on the RX path.

---

## 4. RTL Design

### 4.1 New Module: `host_rx_buffer`

A new RTL module that handles incoming host data buffering. This module will be instantiated inside `host_bus_interface`.

#### 4.1.1 Module Interface

```systemverilog
module host_rx_buffer (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // Buffered Response Packet (for CPU-initiated requests)
    output logic        resp_valid,       // Complete response packet available
    output logic        resp_we,          // Response write enable (echoed from request)
    output logic [1:0]  resp_size,        // Response access size
    output logic [31:0] resp_rdata,       // Response read data (for reads)
    input  logic        resp_consumed,    // Response has been processed
    
    // Buffered Request Packet (for Host-initiated requests)
    output logic        req_valid,        // Complete request packet available
    output logic        req_we,           // Request write enable
    output logic [1:0]  req_size,         // Request access size
    output logic [31:0] req_addr,         // Request address
    output logic [31:0] req_wdata,        // Request write data (for writes)
    input  logic        req_consumed      // Request has been processed
);
```

#### 4.1.2 Internal State Machine

```
                    ┌─────────────────┐
                    │      IDLE       │
                    │ (rx_ready=1)    │
                    └────────┬────────┘
                             │ rx_valid && rx_ready
                             ▼
                    ┌─────────────────┐
                    │  PARSE_HEADER   │
                    │ Determine type  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
    │RESP_RX_DATA │  │REQ_RX_ADDR  │  │ (ignored)   │
    │(type 0001)  │  │(type 0010)  │  │             │
    └──────┬──────┘  └──────┬──────┘  └─────────────┘
           │                │
           ▼                ▼
    ┌─────────────┐  ┌─────────────┐
    │ RESP_READY  │  │REQ_RX_WDATA │
    │resp_valid=1 │  │(if we=1)    │
    └──────┬──────┘  └──────┬──────┘
           │                │
           │                ▼
           │         ┌─────────────┐
           │         │  REQ_READY  │
           │         │ req_valid=1 │
           │         └──────┬──────┘
           │                │
           └────────┬───────┘
                    │ *_consumed
                    ▼
           ┌─────────────────┐
           │  Back to IDLE   │
           └─────────────────┘
```

#### 4.1.3 Buffer Requirements

The module must buffer:
1. **One complete response packet**: header + up to 4 data bytes (for CPU-initiated request responses)
2. **One complete request packet**: header + 4 address bytes + up to 4 data bytes (for host-initiated requests)

The module should only lower `rx_ready` when **both** storage locations contain complete packets. Per Rule 5, this prevents data loss since no more data can arrive once both slots are full.

#### 4.1.4 Implementation Details

> **Note:** The following SystemVerilog code is a reference implementation to guide development. 
> It demonstrates the key design patterns but may require refinement during actual implementation,
> particularly for synthesis tool compatibility and timing optimization.

```systemverilog
module host_rx_buffer (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // Buffered Response Packet (for CPU-initiated requests)
    output logic        resp_valid,
    output logic        resp_we,
    output logic [1:0]  resp_size,
    output logic [31:0] resp_rdata,
    input  logic        resp_consumed,
    
    // Buffered Request Packet (for Host-initiated requests)
    output logic        req_valid,
    output logic        req_we,
    output logic [1:0]  req_size,
    output logic [31:0] req_addr,
    output logic [31:0] req_wdata,
    input  logic        req_consumed
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [3:0] {
        STATE_IDLE          = 4'd0,
        STATE_RESP_RDATA_0  = 4'd1,   // Receiving response data byte 0
        STATE_RESP_RDATA_1  = 4'd2,   // Receiving response data byte 1
        STATE_RESP_RDATA_2  = 4'd3,   // Receiving response data byte 2
        STATE_RESP_RDATA_3  = 4'd4,   // Receiving response data byte 3
        STATE_REQ_ADDR_0    = 4'd5,   // Receiving request address byte 0
        STATE_REQ_ADDR_1    = 4'd6,   // Receiving request address byte 1
        STATE_REQ_ADDR_2    = 4'd7,   // Receiving request address byte 2
        STATE_REQ_ADDR_3    = 4'd8,   // Receiving request address byte 3
        STATE_REQ_WDATA_0   = 4'd9,   // Receiving request write data byte 0
        STATE_REQ_WDATA_1   = 4'd10,  // Receiving request write data byte 1
        STATE_REQ_WDATA_2   = 4'd11,  // Receiving request write data byte 2
        STATE_REQ_WDATA_3   = 4'd12   // Receiving request write data byte 3
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Storage Registers
    // ============================================================
    // Response packet storage
    logic        resp_valid_reg;
    logic        resp_we_reg;
    logic [1:0]  resp_size_reg;
    logic [31:0] resp_rdata_reg;
    
    // Request packet storage
    logic        req_valid_reg;
    logic        req_we_reg;
    logic [1:0]  req_size_reg;
    logic [31:0] req_addr_reg;
    logic [31:0] req_wdata_reg;
    
    // Temporary header fields (used during parsing)
    logic        temp_we;
    logic [1:0]  temp_size;
    
    // Combinational signals for header parsing
    logic [3:0]  header_packet_type;
    logic        header_we;
    logic [1:0]  header_size;
    
    // ============================================================
    // Header Parsing (combinational)
    // ============================================================
    assign header_packet_type = rx_data[7:4];
    assign header_we          = rx_data[0];
    assign header_size        = rx_data[3:2];
    
    // ============================================================
    // Output Assignments
    // ============================================================
    assign resp_valid = resp_valid_reg;
    assign resp_we    = resp_we_reg;
    assign resp_size  = resp_size_reg;
    assign resp_rdata = resp_rdata_reg;
    
    assign req_valid  = req_valid_reg;
    assign req_we     = req_we_reg;
    assign req_size   = req_size_reg;
    assign req_addr   = req_addr_reg;
    assign req_wdata  = req_wdata_reg;
    
    // ============================================================
    // rx_ready Logic
    // ============================================================
    // Ready to receive when:
    // 1. Not both buffers are full (can store new packet)
    // 2. Either in IDLE (waiting for header) or actively receiving packet data
    logic can_store_packet;
    logic is_receiving;
    assign can_store_packet = !(resp_valid_reg && req_valid_reg);
    assign is_receiving = (state >= STATE_RESP_RDATA_0) && (state <= STATE_REQ_WDATA_3);
    assign rx_ready = can_store_packet && (state == STATE_IDLE || is_receiving);
    
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
                if (rx_valid && rx_ready) begin
                    // Parse header to determine packet type
                    case (header_packet_type)
                        4'b0001: begin  // Host response to CPU request
                            // Check if it's a write response (no data) or read response
                            if (header_we) begin
                                // Write response - header only, mark complete
                                next_state = STATE_IDLE;  // Stays in idle, resp_valid set in ff block
                            end else begin
                                // Read response - need to receive data bytes
                                next_state = STATE_RESP_RDATA_0;
                            end
                        end
                        4'b0010: begin  // Host-initiated request
                            next_state = STATE_REQ_ADDR_0;
                        end
                        default: begin
                            // Unknown packet type, stay in idle
                            next_state = STATE_IDLE;
                        end
                    endcase
                end
            end
            
            // Response data receive states - use temp_size which was captured from header
            STATE_RESP_RDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_RESP_RDATA_1;
                    end
                end
            end
            
            STATE_RESP_RDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        next_state = STATE_IDLE;  // Halfword: done
                    end else begin
                        next_state = STATE_RESP_RDATA_2;
                    end
                end
            end
            
            STATE_RESP_RDATA_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_RESP_RDATA_3;
                end
            end
            
            STATE_RESP_RDATA_3: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_IDLE;  // Word: done
                end
            end
            
            // Request address receive states
            STATE_REQ_ADDR_0: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_1;
            end
            
            STATE_REQ_ADDR_1: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_2;
            end
            
            STATE_REQ_ADDR_2: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_3;
            end
            
            STATE_REQ_ADDR_3: begin
                if (rx_valid && rx_ready) begin
                    if (temp_we) begin
                        // Write request - receive write data
                        next_state = STATE_REQ_WDATA_0;
                    end else begin
                        // Read request - done receiving
                        next_state = STATE_IDLE;
                    end
                end
            end
            
            // Request write data receive states
            STATE_REQ_WDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_REQ_WDATA_1;
                    end
                end
            end
            
            STATE_REQ_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        next_state = STATE_IDLE;  // Halfword: done
                    end else begin
                        next_state = STATE_REQ_WDATA_2;
                    end
                end
            end
            
            STATE_REQ_WDATA_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_REQ_WDATA_3;
                end
            end
            
            STATE_REQ_WDATA_3: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_IDLE;  // Word: done
                end
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end
    
    // ============================================================
    // Data Capture Logic
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_valid_reg <= 1'b0;
            resp_we_reg    <= 1'b0;
            resp_size_reg  <= 2'b00;
            resp_rdata_reg <= 32'h0;
            
            req_valid_reg  <= 1'b0;
            req_we_reg     <= 1'b0;
            req_size_reg   <= 2'b00;
            req_addr_reg   <= 32'h0;
            req_wdata_reg  <= 32'h0;
            
            temp_we        <= 1'b0;
            temp_size      <= 2'b00;
        end else begin
            // Handle consumed signals
            if (resp_consumed) begin
                resp_valid_reg <= 1'b0;
            end
            if (req_consumed) begin
                req_valid_reg <= 1'b0;
            end
            
            // Handle state machine data capture
            if (rx_valid && rx_ready) begin
                case (state)
                    STATE_IDLE: begin
                        // Parse header using combinational signals
                        case (header_packet_type)
                            4'b0001: begin  // Host response to CPU request
                                resp_we_reg    <= header_we;
                                resp_size_reg  <= header_size;
                                temp_size      <= header_size;  // Store for later state decisions
                                resp_rdata_reg <= 32'h0;  // Clear for accumulation
                                
                                if (header_we) begin
                                    // Write response - complete immediately
                                    resp_valid_reg <= 1'b1;
                                end
                            end
                            4'b0010: begin  // Host-initiated request
                                temp_we        <= header_we;
                                temp_size      <= header_size;
                                req_we_reg     <= header_we;
                                req_size_reg   <= header_size;
                                req_addr_reg   <= 32'h0;  // Clear for accumulation
                                req_wdata_reg  <= 32'h0;  // Clear for accumulation
                            end
                            default: ; // Ignore unknown packet types
                        endcase
                    end
                    
                    // Response data capture (little-endian) - use temp_size for decisions
                    STATE_RESP_RDATA_0: begin
                        resp_rdata_reg[7:0] <= rx_data;
                        if (temp_size == 2'b00) begin
                            resp_valid_reg <= 1'b1;  // Byte read complete
                        end
                    end
                    
                    STATE_RESP_RDATA_1: begin
                        resp_rdata_reg[15:8] <= rx_data;
                        if (temp_size == 2'b01) begin
                            resp_valid_reg <= 1'b1;  // Halfword read complete
                        end
                    end
                    
                    STATE_RESP_RDATA_2: begin
                        resp_rdata_reg[23:16] <= rx_data;
                    end
                    
                    STATE_RESP_RDATA_3: begin
                        resp_rdata_reg[31:24] <= rx_data;
                        resp_valid_reg <= 1'b1;  // Word read complete
                    end
                    
                    // Request address capture (little-endian)
                    STATE_REQ_ADDR_0: req_addr_reg[7:0]   <= rx_data;
                    STATE_REQ_ADDR_1: req_addr_reg[15:8]  <= rx_data;
                    STATE_REQ_ADDR_2: req_addr_reg[23:16] <= rx_data;
                    STATE_REQ_ADDR_3: begin
                        req_addr_reg[31:24] <= rx_data;
                        if (!temp_we) begin
                            req_valid_reg <= 1'b1;  // Read request complete
                        end
                    end
                    
                    // Request write data capture (little-endian)
                    STATE_REQ_WDATA_0: begin
                        req_wdata_reg[7:0] <= rx_data;
                        if (temp_size == 2'b00) begin
                            req_valid_reg <= 1'b1;  // Byte write complete
                        end
                    end
                    
                    STATE_REQ_WDATA_1: begin
                        req_wdata_reg[15:8] <= rx_data;
                        if (temp_size == 2'b01) begin
                            req_valid_reg <= 1'b1;  // Halfword write complete
                        end
                    end
                    
                    STATE_REQ_WDATA_2: begin
                        req_wdata_reg[23:16] <= rx_data;
                    end
                    
                    STATE_REQ_WDATA_3: begin
                        req_wdata_reg[31:24] <= rx_data;
                        req_valid_reg <= 1'b1;  // Word write complete
                    end
                    
                    default: ;
                endcase
            end
        end
    end

endmodule
```

### 4.2 Updates to `host_bus_interface`

The existing `host_bus_interface` module must be updated to:

1. **Instantiate `host_rx_buffer`**: Consume all RX signals
2. **Prioritize responses**: Process buffered responses before requests
3. **Send requests to master interface**: Forward buffered requests to bus arbiter

#### 4.2.1 Updated State Machine

The `host_bus_interface` state machine will be extended with states for handling host-initiated requests:

```
Existing CPU-initiated flow:
  IDLE → CAPTURE → TX_HEADER → TX_ADDR → TX_WDATA → (wait for response via buffer)

New Host-initiated flow:
  (buffer provides req_valid) → HOST_REQ_WAIT → HOST_TX_RESP_HEADER → HOST_TX_RESP_DATA
```

#### 4.2.2 Key Changes

```systemverilog
// Instantiate the new RX buffer module
host_rx_buffer rx_buf (
    .clk(clk),
    .rst_n(rst_n),
    
    // RX interface from host
    .rx_data(rx_data),
    .rx_valid(rx_valid),
    .rx_ready(rx_ready),
    
    // Response outputs (for CPU-initiated requests)
    .resp_valid(buf_resp_valid),
    .resp_we(buf_resp_we),
    .resp_size(buf_resp_size),
    .resp_rdata(buf_resp_rdata),
    .resp_consumed(buf_resp_consumed),
    
    // Request outputs (for Host-initiated requests)
    .req_valid(buf_req_valid),
    .req_we(buf_req_we),
    .req_size(buf_req_size),
    .req_addr(buf_req_addr),
    .req_wdata(buf_req_wdata),
    .req_consumed(buf_req_consumed)
);

// State machine updates for host-initiated requests
typedef enum logic [4:0] {
    // ... existing states ...
    STATE_HOST_REQ_PENDING = 5'd18,   // Request pending on master interface
    STATE_HOST_TX_HEADER   = 5'd19,   // Sending response header
    STATE_HOST_TX_DATA_0   = 5'd20,   // Sending response data byte 0
    STATE_HOST_TX_DATA_1   = 5'd21,   // Sending response data byte 1
    STATE_HOST_TX_DATA_2   = 5'd22,   // Sending response data byte 2
    STATE_HOST_TX_DATA_3   = 5'd23    // Sending response data byte 3
} state_t;

// Priority logic: Response from buffer > Host-initiated request
// Only process host requests when no outstanding CPU transaction
always_comb begin
    // ... existing logic ...
    
    // Check for buffered response (higher priority)
    if (state == STATE_RX_WR_HEADER || state == STATE_RX_RD_HEADER) begin
        if (buf_resp_valid) begin
            // Consume buffered response instead of waiting for direct RX
            // ...
        end
    end
    
    // Check for buffered host request (lower priority, only in IDLE)
    if (state == STATE_IDLE && !cpu_req_pending && buf_req_valid) begin
        // Start processing host-initiated request
        next_state = STATE_HOST_REQ_PENDING;
    end
end
```

---

## 5. Rust Simulation Changes

### 5.1 SimulatorView API Extensions

Add new functions to the `SimulatorView` interface:

```rust
impl<'a> SimulatorView<'a> {
    /// Send a bus request from the host to the RTL target
    ///
    /// The request will be processed by the RTL host_bus_interface module
    /// and routed through the bus arbiter to the appropriate peripheral.
    ///
    /// # Arguments
    /// * `addr` - Target address (must be in RTL peripheral space: 0x50000000-0x5FFFFFFF)
    /// * `wdata` - Write data (ignored for reads)
    /// * `we` - Write enable (true = write, false = read)
    /// * `size` - Access size (0 = byte, 1 = halfword, 2 = word)
    ///
    /// # Returns
    /// * `Ok(())` - Request queued successfully
    /// * `Err(String)` - Request rejected (already pending, or invalid address)
    pub fn send_bus_request(
        &mut self,
        addr: u32,
        wdata: u32,
        we: bool,
        size: u8,
    ) -> Result<(), String>;

    /// Receive a bus response from the RTL target
    ///
    /// Returns the response for the most recently completed host-initiated request.
    ///
    /// # Returns
    /// * `Some(response)` - Response received (contains rdata for reads)
    /// * `None` - No response available yet
    pub fn receive_bus_response(&mut self) -> Option<HostBusResponse>;
}

/// Response from a host-initiated bus request
#[derive(Debug, Clone)]
pub struct HostBusResponse {
    /// Read data (only valid for read requests)
    pub rdata: u32,
    /// Access size (0 = byte, 1 = halfword, 2 = word)
    pub size: u8,
    /// Whether this was a write request
    pub we: bool,
}
```

### 5.2 Simulator Internal State

Add backing storage in the `Simulator` struct:

```rust
pub struct Simulator<F, T> {
    // ... existing fields ...
    
    /// Pending host-initiated request (None = no pending request)
    host_request_pending: Option<HostBusRequest>,
    
    /// Response from completed host-initiated request (None = no response ready)
    host_response_ready: Option<HostBusResponse>,
    
    /// State machine for host-initiated request serialization
    host_request_state: HostRequestState,
}

#[derive(Debug, Clone)]
struct HostBusRequest {
    addr: u32,
    wdata: u32,
    we: bool,
    size: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostRequestState {
    Idle,
    TxHeader,
    TxAddr { byte_idx: u8 },
    TxWdata { byte_idx: u8 },
    RxHeader,
    RxRdata { byte_idx: u8 },
}
```

### 5.3 Step Function Updates

Update `handle_host_bus_interface` to handle bidirectional communication:

```rust
fn handle_host_bus_interface(&mut self) {
    // PRIORITY 1: Handle outgoing host-initiated requests
    // This runs in parallel with CPU-initiated response handling
    self.handle_host_request_tx();
    
    // PRIORITY 2: Receive FPGA response to host-initiated request
    self.handle_host_response_rx();
    
    // PRIORITY 3: Existing CPU-initiated request handling
    // (already implemented, continues to work as before)
    match self.host_bus_state {
        // ... existing state machine ...
    }
}

fn handle_host_request_tx(&mut self) {
    match self.host_request_state {
        HostRequestState::Idle => {
            if let Some(ref req) = self.host_request_pending {
                // Start sending request when we can
                // Only send when CPU transaction is not using RX
                if !self.is_cpu_transaction_using_rx() {
                    self.host_request_state = HostRequestState::TxHeader;
                }
            }
        }
        HostRequestState::TxHeader => {
            // Send header byte: {packet_type=0010, size, 0, we}
            let req = self.host_request_pending.as_ref().unwrap();
            let header = 0x20 | ((req.size & 0x03) << 2) | (req.we as u8);
            
            self.cpu.host_rx_valid = 1;
            self.cpu.host_rx_data = header;
            
            if self.cpu.host_rx_ready != 0 {
                self.host_request_state = HostRequestState::TxAddr { byte_idx: 0 };
            }
        }
        // ... continue with address and data bytes ...
    }
}

fn handle_host_response_rx(&mut self) {
    if self.host_request_state == HostRequestState::RxHeader {
        // Waiting for response header (packet type 0011)
        if self.cpu.host_tx_valid != 0 {
            let header = self.cpu.host_tx_data;
            let packet_type = (header >> 4) & 0x0F;
            
            if packet_type == 0x03 {
                // FPGA response to host
                let we = (header & 0x01) != 0;
                let size = ((header >> 2) & 0x03) as u8;
                
                if we {
                    // Write response - complete
                    self.host_response_ready = Some(HostBusResponse {
                        rdata: 0,
                        size,
                        we: true,
                    });
                    self.host_request_pending = None;
                    self.host_request_state = HostRequestState::Idle;
                } else {
                    // Read response - receive data
                    self.host_request_state = HostRequestState::RxRdata { byte_idx: 0 };
                }
            }
        }
    }
}
```

---

## 6. Testing Plan

### 6.1 RTL Testbench Tests (testbench/tests/host_rx_buffer_test.rs)

Create a new test file for the `host_rx_buffer` module:

```rust
// Test cases:

#[test]
fn test_reset_state()
// Verify all outputs are LOW/zero after reset

#[test]
fn test_receive_response_byte()
// Send packet type 0001, size=byte, verify resp_valid asserts

#[test]
fn test_receive_response_halfword()
// Send packet type 0001, size=halfword, verify 2-byte accumulation

#[test]
fn test_receive_response_word()
// Send packet type 0001, size=word, verify 4-byte accumulation

#[test]
fn test_receive_write_response()
// Send packet type 0001, we=1, verify immediate resp_valid

#[test]
fn test_receive_request_read()
// Send packet type 0010, we=0, verify req_valid after address

#[test]
fn test_receive_request_write_byte()
// Send packet type 0010, we=1, size=byte, verify req_valid after 1 data byte

#[test]
fn test_receive_request_write_word()
// Send packet type 0010, we=1, size=word, verify req_valid after 4 data bytes

#[test]
fn test_both_buffers_full()
// Fill both response and request buffers, verify rx_ready goes LOW

#[test]
fn test_consume_response()
// Fill response buffer, assert resp_consumed, verify resp_valid clears

#[test]
fn test_consume_request()
// Fill request buffer, assert req_consumed, verify req_valid clears

#[test]
fn test_interleaved_packets()
// Send alternating response and request packets

#[test]
fn test_backpressure_recovery()
// Fill both buffers, consume one, verify rx_ready re-asserts
```

### 6.2 Extended host_bus_interface Tests

Add tests to `testbench/tests/host_bus_interface_test.rs`:

```rust
#[test]
fn test_host_initiated_read_word()
// Send host-initiated read request, verify master interface signals

#[test]
fn test_host_initiated_write_word()
// Send host-initiated write request, verify master interface signals

#[test]
fn test_host_response_packet_format()
// Complete host request, verify TX response packet format (type 0011)

#[test]
fn test_concurrent_cpu_and_host_requests()
// Start CPU request, then send host request, verify correct handling

#[test]
fn test_response_priority()
// With both response and request buffered, verify response processed first
```

### 6.3 CPU-Sim Integration Tests

Add new test file: `cpu-sim/tests/test_host_initiated_requests.rs`

#### 6.3.1 Minimal Synchronization Test

This is the foundational test that proves the system works:

```rust
#[test]
fn test_host_initiated_basic_sync() {
    init_test_logger();
    
    // Synchronization fence address
    const FENCE_ADDR: u32 = 0x8000_1000;
    
    // Program that spins on memory location until it changes from 0 to 1
    let instructions = vec![
        // Setup: Load fence address
        lui(15, 0x80001000 >> 12),       // x15 = fence address base
        // Spin loop: wait for memory[FENCE_ADDR] != 0
        lw(14, 15, 0),                   // x14 = memory[fence_addr]
        beq(14, 0, -4),                  // if x14 == 0, loop back to lw
        // Exit: Write to tohost
        lui(10, 0x10000000 >> 12),       // x10 = tohost address
        addi(11, 0, 1),                  // x11 = 1 (success)
        sw(10, 11, 0),                   // memory[tohost] = 1
        jal(0, 0),                       // infinite loop
    ];
    
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();
    
    let fence_written = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fence_written_clone = fence_written.clone();
    
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(move |sim: &mut SimulatorView| {
            // On each instruction complete, check if we should release the fence
            // After some cycles, write 1 to the fence address via host-initiated request
            let mut written = fence_written_clone.lock().unwrap();
            if !*written {
                // Send host-initiated write to LED peripheral (RTL space)
                // This tests the full path: Host → RX → Buffer → Master → Bus → Peripheral
                
                // For this basic test, we'll just use the SimulatorView memory API
                // to modify the fence, proving the callback mechanism works
                sim.write_memory_region(FENCE_ADDR, &1u32.to_le_bytes(), false);
                *written = true;
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Setup: Write program and initialize fence to 0
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            sim.write_memory_region(FENCE_ADDR, &0u32.to_le_bytes(), false);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should complete");
    
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code after fence release"
    );
}
```

#### 6.3.2 Host-Initiated LED Write Test

Test actual RTL peripheral access:

```rust
#[test]
fn test_host_initiated_led_write() {
    init_test_logger();
    
    // Address constants
    const FENCE_ADDR: u32 = 0x8000_1000;
    const LED_EXPECTED_ADDR: u32 = 0x8000_1004;
    const LED_BASE: u32 = 0x50000000;
    
    // Program that:
    // 1. Waits for fence to be released
    // 2. Reads the expected LED value from memory (written by host)
    // 3. Reads the actual LED value from LED peripheral
    // 4. Compares and writes result to tohost
    let instructions = vec![
        // Setup addresses
        lui(15, FENCE_ADDR >> 12),        // x15 = fence address
        lui(14, LED_BASE >> 12),          // x14 = LED base address
        lui(13, LED_EXPECTED_ADDR >> 12), // x13 = expected value address
        
        // Wait for fence
        lw(12, 15, 0),                    // x12 = memory[fence]
        beq(12, 0, -4),                   // spin while fence == 0
        
        // Read expected and actual LED values
        lw(11, 13, LED_EXPECTED_ADDR & 0xFFF),  // x11 = expected LED value
        lw(10, 14, 0),                    // x10 = LED peripheral value
        andi(10, 10, 0xFF),               // mask to 8 bits
        
        // Compare
        lui(9, 0x10000000 >> 12),         // x9 = tohost address
        sub(8, 10, 11),                   // x8 = actual - expected
        bne(8, 0, 12),                    // if not equal, fail
        
        // Success
        addi(7, 0, 1),
        sw(9, 7, 0),
        jal(0, 0),
        
        // Failure
        addi(7, 0, 2),
        sw(9, 7, 0),
        jal(0, 0),
    ];
    
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();
    
    let host_request_sent = std::sync::Arc::new(std::sync::Mutex::new(false));
    let host_request_sent_clone = host_request_sent.clone();
    
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut sent = host_request_sent_clone.lock().unwrap();
            if !*sent {
                // Send host-initiated write to LED peripheral
                let led_value = 0xA5u8;
                sim.send_bus_request(LED_BASE, led_value as u32, true, 0)
                    .expect("Should queue host request");
                
                // Store expected value in memory for CPU to read
                sim.write_memory_region(LED_EXPECTED_ADDR, &[led_value], false);
                
                // Release fence
                sim.write_memory_region(FENCE_ADDR, &1u32.to_le_bytes(), false);
                *sent = true;
            }
            
            // Check for response
            if let Some(response) = sim.receive_bus_response() {
                assert!(response.we, "Should be write response");
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            sim.write_memory_region(FENCE_ADDR, &0u32.to_le_bytes(), false);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should complete");
    
    assert_eq!(
        result.tohost_value,
        Some(1),
        "LED value should match expected"
    );
}
```

#### 6.3.3 Additional CPU-Sim Tests

```rust
#[test]
fn test_host_initiated_led_read()
// Host reads LED value that CPU previously wrote

#[test]
fn test_host_initiated_clock_read()
// Host reads clock peripheral registers

#[test]
fn test_host_initiated_uart_loopback()
// Host writes to UART TX, CPU reads from UART RX

#[test]
fn test_multiple_host_requests()
// Multiple sequential host-initiated requests

#[test]
fn test_host_request_during_cpu_activity()
// CPU executing instructions while host sends request
```

---

## 7. Implementation Order

### Phase 1: RTL Buffer Module
1. Create `rtl/host_rx_buffer.sv` with full state machine
2. Add Verilator wrapper for standalone testing
3. Write and pass all `host_rx_buffer_test.rs` tests

### Phase 2: Host Bus Interface Updates
1. Update `rtl/host_bus_interface.sv` to instantiate buffer
2. Add host-initiated request processing states
3. Update master interface outputs
4. Write and pass extended `host_bus_interface_test.rs` tests

### Phase 3: Rust Simulation Updates
1. Add `HostBusRequest`, `HostBusResponse`, and `HostRequestState` to `sim.rs`
2. Implement `send_bus_request()` and `receive_bus_response()` on SimulatorView
3. Update `handle_host_bus_interface()` for bidirectional operation
4. Test with minimal synchronization test

### Phase 4: Integration Testing
1. Run basic sync test (fence pattern)
2. Test LED peripheral access from host
3. Test clock peripheral read from host
4. Test concurrent CPU and host access
5. Verify no regressions in existing tests

### Phase 5: Documentation
1. Update protocol comments in RTL modules
2. Update AGENTS.md with new API
3. Add usage examples to documentation

---

## 8. Validation Checklist

Before marking complete:

- [ ] All new RTL modules lint clean: `verilator --lint-only rtl/*.sv`
- [ ] Verilator cache cleared: `cargo clean`
- [ ] All Rust code formatted: `cargo fmt`
- [ ] Clippy auto-fix run: `cargo clippy --fix --allow-dirty`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] All tests pass: `cargo test --verbose` (including new tests)
- [ ] No regressions in existing test suite (264+ tests)
- [ ] FPGA synthesis passes: `(cd fpga && make)`

---

## 9. Risk Assessment

### 9.1 Timing Closure
- **Risk**: Adding buffer module may impact timing
- **Mitigation**: Keep buffer logic simple, use registered outputs

### 9.2 Deadlock
- **Risk**: Improper priority handling could cause deadlock
- **Mitigation**: Follow deadlock prevention rules strictly, extensive testing

### 9.3 Data Corruption
- **Risk**: Interleaved packets could corrupt state
- **Mitigation**: Separate storage for response and request, clear state transitions

### 9.4 Backward Compatibility
- **Risk**: Changes could break existing CPU-initiated flow
- **Mitigation**: Run full regression suite, preserve existing state machine

---

## 10. Appendix: Packet Examples

### 10.1 Host-Initiated Word Write to LED (0x50000000 = 0xAB)

```
Host TX (via host_rx_*):
  [0x29]          - Header: type=0010, size=10, we=1
  [0x00]          - Address[7:0]
  [0x00]          - Address[15:8]
  [0x00]          - Address[23:16]
  [0x50]          - Address[31:24]
  [0xAB]          - WData[7:0]
  [0x00]          - WData[15:8]
  [0x00]          - WData[23:16]
  [0x00]          - WData[31:24]

FPGA TX (via host_tx_*):
  [0x39]          - Header: type=0011, size=10, we=1 (write ack)
```

### 10.2 Host-Initiated Word Read from LED (0x50000000)

```
Host TX (via host_rx_*):
  [0x28]          - Header: type=0010, size=10, we=0
  [0x00]          - Address[7:0]
  [0x00]          - Address[15:8]
  [0x00]          - Address[23:16]
  [0x50]          - Address[31:24]

FPGA TX (via host_tx_*):
  [0x38]          - Header: type=0011, size=10, we=0 (read response)
  [0xAB]          - RData[7:0]
  [0x00]          - RData[15:8]
  [0x00]          - RData[23:16]
  [0x00]          - RData[31:24]
```

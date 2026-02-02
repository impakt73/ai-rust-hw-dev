# Host-Initiated Bus Requests - Remaining Implementation Work

**Author:** AI Documentation Review  
**Date:** February 2, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Status:** Active Implementation Plan  
**Parent Document:** `host-initiated-bus-requests-implementation-plan.md`

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
   - Host→FPGA request state machine in `host_bus_interface.sv`
   - IDLE state logic to detect and process incoming host requests
   - Address validation for host requests (RTL peripheral range only)
   - Error response states and logic
   - **Critical Fix:** Proper receive-byte handling in IDLE state (see Known Issue below)

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

## Known Issue: IDLE State Receive-Byte Handling

### Problem Description

The current `host_bus_interface.sv` IDLE state logic only checks for CPU-initiated requests (`req` signal). It does **not** monitor the RX interface for incoming host-initiated requests. This means:

- When the Host sends a request (packet type 0010), the FPGA will not recognize it
- The bytes will arrive on `rx_data` with `rx_valid` asserted, but `rx_ready` is not asserted in IDLE
- The IDLE state must check `rx_valid`, decode the header, and assert `rx_ready` to consume the byte

### Current Code (Incorrect)

```systemverilog
STATE_IDLE: begin
    if (req) begin
        next_state = STATE_CAPTURE;
    end
end
```

### Required Fix (Summary)

The IDLE state must:
1. Check `rx_valid` for incoming data
2. Decode the header byte to detect packet type 0010 (host-initiated request)
3. Assert `rx_ready` to consume the header byte
4. Capture the header fields (`we`, `size`) in registers
5. Transition to `STATE_HOST_RX_ADDR_0` to start receiving address bytes

**Full implementation details are provided in Task 1 (sections 1.3, 1.4, and 1.9) below.**

---

## Detailed Implementation Tasks

### Task 1: RTL State Machine for Host→FPGA Requests

**File:** `rtl/host_bus_interface.sv`

#### 1.1 Add New States

Extend the state enum to add host-initiated request handling states:

```systemverilog
typedef enum logic [5:0] {
    // Existing CPU→Host states (unchanged)
    STATE_IDLE        = 6'd0,
    STATE_CAPTURE     = 6'd1,
    STATE_TX_HEADER   = 6'd2,
    STATE_TX_ADDR_0   = 6'd3,
    STATE_TX_ADDR_1   = 6'd4,
    STATE_TX_ADDR_2   = 6'd5,
    STATE_TX_ADDR_3   = 6'd6,
    STATE_TX_WDATA_0  = 6'd7,
    STATE_TX_WDATA_1  = 6'd8,
    STATE_TX_WDATA_2  = 6'd9,
    STATE_TX_WDATA_3  = 6'd10,
    STATE_RX_WR_HEADER = 6'd11,
    STATE_RX_RD_HEADER = 6'd12,
    STATE_RX_RDATA_0   = 6'd13,
    STATE_RX_RDATA_1   = 6'd14,
    STATE_RX_RDATA_2   = 6'd15,
    STATE_RX_RDATA_3   = 6'd16,
    STATE_COMPLETE     = 6'd17,
    
    // NEW: Host→FPGA request states
    STATE_HOST_RX_ADDR_0    = 6'd20,  // Receive address[7:0] (header consumed in IDLE)
    STATE_HOST_RX_ADDR_1    = 6'd21,  // Receive address[15:8]
    STATE_HOST_RX_ADDR_2    = 6'd22,  // Receive address[23:16]
    STATE_HOST_RX_ADDR_3    = 6'd23,  // Receive address[31:24]
    STATE_HOST_RX_WDATA_0   = 6'd24,  // Receive wdata[7:0] (writes only)
    STATE_HOST_RX_WDATA_1   = 6'd25,  // Receive wdata[15:8]
    STATE_HOST_RX_WDATA_2   = 6'd26,  // Receive wdata[23:16]
    STATE_HOST_RX_WDATA_3   = 6'd27,  // Receive wdata[31:24]
    STATE_HOST_BUS_REQ      = 6'd28,  // Issue request to bus arbiter
    STATE_HOST_BUS_WAIT     = 6'd29,  // Wait for bus response
    STATE_HOST_TX_HEADER    = 6'd30,  // Send response header (type 0011)
    STATE_HOST_TX_RDATA_0   = 6'd31,  // Send rdata[7:0] (reads only)
    STATE_HOST_TX_RDATA_1   = 6'd32,  // Send rdata[15:8]
    STATE_HOST_TX_RDATA_2   = 6'd33,  // Send rdata[23:16]
    STATE_HOST_TX_RDATA_3   = 6'd34,  // Send rdata[31:24]
    STATE_HOST_ERROR        = 6'd35,  // Error: send error response
    STATE_HOST_ERROR_CODE   = 6'd36   // Error: send error code byte
} state_t;
```

**Note:** Changed encoding from `logic [4:0]` to `logic [5:0]` to accommodate additional states (max value: 36 decimal = 100100 binary, requires 6 bits).

#### 1.4 Add Host Request Capture Logic

Add registers to capture host-initiated request parameters:

```systemverilog
// New registers for host-initiated transactions
logic [31:0] host_cap_addr;      // Captured host request address
logic [31:0] host_cap_wdata;     // Captured host request write data
logic        host_cap_we;        // Captured host request write enable
logic [1:0]  host_cap_size;      // Captured host request size
logic [31:0] host_resp_rdata;    // Response read data from bus
```

**Capture header fields when transitioning from IDLE to HOST_RX_ADDR_0:**

```systemverilog
// Add to existing register update block or create new one
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        host_cap_we   <= 1'b0;
        host_cap_size <= 2'b00;
        host_cap_addr <= 32'h0;
        host_cap_wdata <= 32'h0;
    end else begin
        // Capture header fields when detecting host request in IDLE
        if (state == STATE_IDLE && rx_valid && is_host_initiated_request_header(rx_data)) begin
            host_cap_we   <= rx_data[0];
            host_cap_size <= rx_data[3:2];
        end
        
        // Capture address bytes (see section 1.5 for full implementation)
        // Capture write data bytes (see section 1.5 for full implementation)
    end
end
```

#### 1.3 Update IDLE State Logic

**Critical Fix:** Make IDLE state check for incoming host requests and consume the header byte:

```systemverilog
STATE_IDLE: begin
    // Priority 1: Check for incoming Host-initiated request
    if (rx_valid && is_host_initiated_request_header(rx_data)) begin
        // Packet type 0010: Host request header detected
        // The header byte will be consumed (rx_ready asserted, see below)
        // Fields will be captured in register update (see section 1.4)
        next_state = STATE_HOST_RX_ADDR_0;  // Skip to address reception
    end
    // Priority 2: Check for CPU-initiated request (existing)
    else if (req) begin
        next_state = STATE_CAPTURE;
    end
end
```

**Important:** The IDLE state must assert `rx_ready` when a valid host request header is detected, so the byte gets consumed. See section 1.9 for the updated `rx_ready` signal.

#### 1.5 Implement Host Request RX Path

States to receive host request from RX interface. Note that `STATE_HOST_RX_HEADER` is eliminated since the header is consumed in IDLE:

```systemverilog
// Start with address byte 0 (header already consumed in IDLE)
STATE_HOST_RX_ADDR_0: begin
    if (rx_valid && rx_ready) begin
        host_cap_addr[7:0] <= rx_data;
        next_state = STATE_HOST_RX_ADDR_1;
    end
end

STATE_HOST_RX_ADDR_1: begin
    if (rx_valid && rx_ready) begin
        host_cap_addr[15:8] <= rx_data;
        next_state = STATE_HOST_RX_ADDR_2;
    end
end

STATE_HOST_RX_ADDR_2: begin
    if (rx_valid && rx_ready) begin
        host_cap_addr[23:16] <= rx_data;
        next_state = STATE_HOST_RX_ADDR_3;
    end
end

STATE_HOST_RX_ADDR_3: begin
    if (rx_valid && rx_ready) begin
        host_cap_addr[31:24] <= rx_data;
        // Address complete: check if write (need data) or read (validate & issue)
        if (host_cap_we) begin
            next_state = STATE_HOST_RX_WDATA_0;  // Write: receive data
        end else begin
            next_state = STATE_HOST_BUS_REQ;     // Read: validate & issue
        end
    end
end

// Write data states (little-endian)
STATE_HOST_RX_WDATA_0: begin
    if (rx_valid && rx_ready) begin
        host_cap_wdata[7:0] <= rx_data;
        if (host_cap_size == 2'b00) begin
            next_state = STATE_HOST_BUS_REQ;     // Byte: done
        end else begin
            next_state = STATE_HOST_RX_WDATA_1;  // Half/Word: continue
        end
    end
end

STATE_HOST_RX_WDATA_1: begin
    if (rx_valid && rx_ready) begin
        host_cap_wdata[15:8] <= rx_data;
        if (host_cap_size == 2'b01) begin
            next_state = STATE_HOST_BUS_REQ;     // Half: done
        end else begin
            next_state = STATE_HOST_RX_WDATA_2;  // Word: continue
        end
    end
end

STATE_HOST_RX_WDATA_2: begin
    if (rx_valid && rx_ready) begin
        host_cap_wdata[23:16] <= rx_data;
        next_state = STATE_HOST_RX_WDATA_3;
    end
end

STATE_HOST_RX_WDATA_3: begin
    if (rx_valid && rx_ready) begin
        host_cap_wdata[31:24] <= rx_data;
        next_state = STATE_HOST_BUS_REQ;
    end
end
```

#### 1.5 Implement Address Validation and Bus Request

```systemverilog
// Address range constants
localparam RTL_PERIPH_BASE  = 32'h5000_0000;
localparam RTL_PERIPH_LIMIT = 32'h6000_0000;

// Validate address and issue bus request
STATE_HOST_BUS_REQ: begin
    // Check if address is in RTL peripheral range
    if (host_cap_addr >= RTL_PERIPH_BASE && host_cap_addr < RTL_PERIPH_LIMIT) begin
        // Valid address: issue bus request through arbiter
        next_state = STATE_HOST_BUS_WAIT;
    end else begin
        // Invalid address: send error response
        next_state = STATE_HOST_ERROR;
    end
end

STATE_HOST_BUS_WAIT: begin
    if (host_bus_ready) begin
        // Bus transaction complete
        // Capture read data if this was a read
        if (!host_cap_we) begin
            host_resp_rdata <= host_bus_rdata;
        end
        // Send response header
        next_state = STATE_HOST_TX_HEADER;
    end
end
```

#### 1.6 Implement Host Response TX Path

Send response back to host (packet type 0011):

```systemverilog
STATE_HOST_TX_HEADER: begin
    // tx_byte set in TX mux (see below)
    if (tx_valid && tx_ready) begin
        if (host_cap_we) begin
            // Write: response is just header, done
            next_state = STATE_IDLE;
        end else begin
            // Read: send data bytes
            next_state = STATE_HOST_TX_RDATA_0;
        end
    end
end

// Read response data bytes (little-endian)
STATE_HOST_TX_RDATA_0: begin
    if (tx_valid && tx_ready) begin
        if (host_cap_size == 2'b00) begin
            next_state = STATE_IDLE;              // Byte: done
        end else begin
            next_state = STATE_HOST_TX_RDATA_1;   // Half/Word: continue
        end
    end
end

STATE_HOST_TX_RDATA_1: begin
    if (tx_valid && tx_ready) begin
        if (host_cap_size == 2'b01) begin
            next_state = STATE_IDLE;              // Half: done
        end else begin
            next_state = STATE_HOST_TX_RDATA_2;   // Word: continue
        end
    end
end

STATE_HOST_TX_RDATA_2: begin
    if (tx_valid && tx_ready) begin
        next_state = STATE_HOST_TX_RDATA_3;
    end
end

STATE_HOST_TX_RDATA_3: begin
    if (tx_valid && tx_ready) begin
        next_state = STATE_IDLE;
    end
end
```

#### 1.7 Implement Error Response States

```systemverilog
STATE_HOST_ERROR: begin
    // Send error response header (packet type 1111)
    // tx_byte set in TX mux
    if (tx_valid && tx_ready) begin
        next_state = STATE_HOST_ERROR_CODE;
    end
end

STATE_HOST_ERROR_CODE: begin
    // Send error code byte (0xFF = invalid address)
    // tx_byte set in TX mux
    if (tx_valid && tx_ready) begin
        next_state = STATE_IDLE;
    end
end
```

#### 1.8 Update TX Data Multiplexer

Add host response TX data to the mux:

```systemverilog
always_comb begin
    tx_byte = 8'h00;
    
    case (state)
        // Existing CPU→Host TX states
        STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};  // Type 0000
        STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
        STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
        STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
        STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
        STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
        STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
        STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
        STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
        
        // NEW: Host→FPGA response TX states
        STATE_HOST_TX_HEADER:  tx_byte = {4'b0011, host_cap_size, 1'b0, host_cap_we};  // Type 0011
        STATE_HOST_TX_RDATA_0: tx_byte = host_resp_rdata[7:0];
        STATE_HOST_TX_RDATA_1: tx_byte = host_resp_rdata[15:8];
        STATE_HOST_TX_RDATA_2: tx_byte = host_resp_rdata[23:16];
        STATE_HOST_TX_RDATA_3: tx_byte = host_resp_rdata[31:24];
        
        // NEW: Error response TX states
        STATE_HOST_ERROR:      tx_byte = {4'b1111, host_cap_size, 1'b0, host_cap_we};  // Type 1111
        STATE_HOST_ERROR_CODE: tx_byte = 8'hFF;  // Error code: invalid address
        
        default: tx_byte = 8'h00;
    endcase
end
```

#### 1.9 Update TX Valid and RX Ready Signals

```systemverilog
// TX valid: asserted during CPU TX states OR Host response TX states OR error states
assign tx_valid = (state >= STATE_TX_HEADER && state <= STATE_TX_WDATA_3) ||
                  (state >= STATE_HOST_TX_HEADER && state <= STATE_HOST_TX_RDATA_3) ||
                  (state == STATE_HOST_ERROR) ||
                  (state == STATE_HOST_ERROR_CODE);

// RX ready: asserted during CPU RX states OR Host request RX states OR in IDLE when detecting host request
assign rx_ready = (state >= STATE_RX_WR_HEADER && state <= STATE_RX_RDATA_3) ||
                  (state >= STATE_HOST_RX_ADDR_0 && state <= STATE_HOST_RX_WDATA_3) ||
                  (state == STATE_IDLE && rx_valid && is_host_initiated_request_header(rx_data));
```

**Critical:** The last line of `rx_ready` ensures that when a host request header arrives in IDLE, the byte is consumed on that same clock cycle (assuming `rx_valid` is asserted).

#### 1.10 Update Bus Master Interface Outputs

Connect the host request to the bus master interface:

```systemverilog
// Bus Master Interface (Host→RTL path)
assign host_bus_addr  = host_cap_addr;
assign host_bus_wdata = host_cap_wdata;
assign host_bus_we    = host_cap_we;
assign host_bus_size  = host_cap_size;

// Assert request only in HOST_BUS_WAIT state
assign host_bus_req = (state == STATE_HOST_BUS_WAIT);
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
    // Packet type 0010, size=word (10), we=0 (read)
    let header = 0b0010_10_0_0;  // = 0x28
    
    send_rx_byte(&mut dut, header);
    send_rx_byte(&mut dut, 0x00);  // addr[7:0]
    send_rx_byte(&mut dut, 0x00);  // addr[15:8]
    send_rx_byte(&mut dut, 0x00);  // addr[23:16]
    send_rx_byte(&mut dut, 0x50);  // addr[31:24] = 0x50000000
    
    // Expect FPGA to issue bus request and send response
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
    let header = 0b0010_10_0_0;  // Read word
    
    send_rx_byte(&mut dut, header);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x00);
    send_rx_byte(&mut dut, 0x80);  // Invalid: Host address
    
    // Expect error response
    assert_tx_byte(&mut dut, 0b1111_10_0_0);  // Error header
    assert_tx_byte(&mut dut, 0xFF);           // Error code: invalid address
}
```

#### 3.3 CPU-Sim Test: End-to-End LED Test

Create new file `cpu-sim/tests/test_host_bus_requests.rs`:

```rust
#[test]
fn test_host_write_led() {
    let mut sim = Simulator::new(...);
    
    // Host writes to LED peripheral
    let request = HostBusRequest {
        addr: 0x50000000,
        wdata: 0xAA,
        size: 0,  // byte
        we: true,
    };
    
    sim.send_bus_request(request).unwrap();
    
    // Step simulation to process request
    for _ in 0..100 {
        sim.step();
    }
    
    // Check response
    let response = sim.receive_bus_response().unwrap();
    assert_eq!(response, HostBusResponse::WriteAck);
    
    // Verify LED output changed
    assert_eq!(sim.led_out(), 0xAA);
}
```

---

## Implementation Checklist

### Phase 1: RTL Changes (host_bus_interface.sv)

- [ ] Change state enum from `logic [4:0]` to `logic [5:0]`
- [ ] Add new states for host request handling (20-36, no separate HOST_RX_HEADER needed)
- [ ] Add host request capture registers
- [ ] Update IDLE state to check `rx_valid` and decode packet type
- [ ] Implement `is_host_initiated_request_header()` function
- [ ] Update IDLE register capture to latch header fields when host request detected
- [ ] Implement HOST_RX_ADDR states (0-3)
- [ ] Implement HOST_RX_WDATA states (0-3) for writes
- [ ] Implement address validation (RTL_PERIPH_BASE check)
- [ ] Implement HOST_BUS_REQ and HOST_BUS_WAIT states
- [ ] Implement HOST_TX_HEADER state
- [ ] Implement HOST_TX_RDATA states (0-3) for reads
- [ ] Implement HOST_ERROR states (header, code)
- [ ] Update TX data mux to include host response cases
- [ ] Update `tx_valid` signal to include host TX states
- [ ] Update `rx_ready` signal to include host RX states AND IDLE host request detection
- [ ] Update `host_bus_*` output assignments (remove hardcoded zeros)
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

- [ ] Add `test_host_initiated_read` to `host_bus_interface_test.rs`
- [ ] Add `test_host_initiated_write` to `host_bus_interface_test.rs`
- [ ] Add `test_host_invalid_address` to `host_bus_interface_test.rs`
- [ ] Create `cpu-sim/tests/test_host_bus_requests.rs`
- [ ] Add `test_host_write_led` (end-to-end test)
- [ ] Add `test_host_read_led` (end-to-end test)
- [ ] Run all tests and verify pass

### Phase 4: Documentation

- [ ] Update this plan document with implementation notes
- [ ] Update `AGENTS.md` if needed
- [ ] Mark original plan as "Implementation Complete"

---

## Testing Notes

### Simultaneous Request Handling

**Note:** The current `host_bus_interface.sv` does NOT use a FIFO for RX buffering. It uses direct state machine handshaking. The original plan discusses "RX FIFO byte-level buffering" as a mechanism to handle simultaneous requests, but this is **not currently implemented**.

**Design Decision for Initial Implementation:**

Given the asymmetric handling strategy where:
- **Host side:** Processes incoming CPU requests immediately even while waiting for its own response
- **FPGA side:** Cannot process incoming host requests while CPU transaction is active (bus locked)

The simultaneous request scenario is resolved by the **Host side's non-blocking RX processing** rather than FPGA-side buffering. The FPGA's state machine will simply remain in CPU transaction states, and the Host's request will wait on the Host side until the FPGA returns to IDLE.

**Future Enhancement:** If deeper buffering is needed, a FIFO can be added at the RX interface, but it is not required for the initial implementation.

---

## References

- Parent Plan: `host-initiated-bus-requests-implementation-plan.md`
- AGENTS.md: RTL peripheral address ranges
- RTL Style Guide: `docs/rtl-style-guide.md` (if exists)
- Rust Coding Standards: Project follows `cargo fmt` and `cargo clippy`

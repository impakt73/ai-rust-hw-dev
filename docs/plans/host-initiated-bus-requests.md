# Host-Initiated Bus Requests Implementation Plan

## Overview

This document provides a detailed technical implementation plan for upgrading the existing host bus communication system to support **host-initiated bus requests**. Currently, the system only supports CPU-initiated requests (FPGA → Host path), but this upgrade will enable the host to initiate requests that are processed by the target/FPGA via the `host_bus_interface` module.

## Current Architecture Summary

### Existing Data Flow
```
CPU → bus_arbiter → bus.sv → host_bus_interface → Host TX → Simulator (Rust)
                                                 ← Host RX ← Simulator (Rust)
```

### Current Protocol (CPU-Initiated Only)
- **Packet Type 0000**: CPU-initiated request (FPGA → Host TX)
- **Packet Type 0001**: Host response to CPU request (Host → FPGA RX)
- **Packet Type 0010**: Host-initiated request (Reserved, not implemented)
- **Packet Type 0011**: FPGA response to Host request (Reserved, not implemented)

### Extended Header Format
```
Bits [7:4]: Packet type
  0000 = CPU-initiated request (FPGA → Host TX)
  0001 = Host response to CPU request (Host → FPGA RX)
  0010 = Host-initiated request (Host → FPGA RX)
  0011 = FPGA response to Host request (FPGA → Host TX)
Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
Bit  [1]:   Reserved (0)
Bit  [0]:   we (1=write, 0=read)
```

---

## Deadlock Avoidance Rules

The following rules **MUST** be followed to avoid deadlocks in the bidirectional communication system:

### Rule 1: No Self-Routing Requests
Any side of the connection must avoid sending a request that will ultimately be routed back to itself based on the associated memory address.
- **Host side**: Must not send requests to addresses handled by Rust peripherals (0x10000000-0x4FFFFFFF)
- **Target/FPGA side**: Must not send requests to addresses that would be routed to external memory (handled by host)

### Rule 2: Single Outstanding Request Per Side
Both sides of the connection must only allow a single outstanding request at a time.
- Host cannot have more than one pending request to the target
- Target cannot have more than one pending request to the host

### Rule 3: Host Must Process Requests Immediately
The host side must process incoming requests as soon as they are received, **even when it has an outstanding request to the target side**.
- This prevents blocking when both sides have requests in flight

### Rule 4: Target Prioritizes Outstanding Outgoing Requests
The target side must complete outstanding outgoing requests before it processes incoming requests from the host.
- This ensures the target's bus is not held indefinitely

### Rule 5: Target Must Accept Data During Outstanding Request
The target side must accept new data from the host even if it has an outstanding host-bound request.
- This prevents RX starvation when waiting for a response

---

## RX Buffering Strategy

### Problem Statement
Without buffering, if the target side's bus is locked due to an outstanding CPU→Host request, incoming Host→FPGA request data would be blocked. This could cause deadlock if the target cannot read ahead past the incoming request to find the response data needed to complete the outstanding request.

### Solution: Dual-Packet RX Buffer
The RX side of the host bus interface will buffer incoming data to accept new data every clock cycle when available. The buffer must be able to hold:
1. **One complete request packet** (up to 9 bytes: header + 4 addr + 4 data)
2. **One complete response packet** (up to 5 bytes: header + 4 data)

**Total buffer requirement**: 14 bytes minimum

### Buffer Implementation
```systemverilog
// RX buffer - can hold request (9 bytes) + response (5 bytes) = 14 bytes
// Using 16 bytes (4 words) for alignment and margin
logic [7:0] rx_buffer [0:15];
logic [3:0] rx_write_ptr;
logic [3:0] rx_read_ptr;
logic [4:0] rx_count;  // 0-16

// rx_ready is HIGH when buffer has space
assign rx_ready = (rx_count < 14);  // Leave room for both packet types
```

### Why This Works
Once the buffer holds both a request and a response:
- No more data can arrive (Rule 2 limits each side to one outstanding request)
- The target can process the response to unblock the CPU
- Then process the buffered request

---

## RTL Implementation Details

### File: `rtl/host_bus_interface.sv`

#### 1. New State Machine States

Add states for handling Host-initiated requests:

```systemverilog
typedef enum logic [4:0] {
    // Existing states (unchanged)
    STATE_IDLE        = 5'd0,
    STATE_CAPTURE     = 5'd1,
    STATE_TX_HEADER   = 5'd2,
    STATE_TX_ADDR_0   = 5'd3,
    STATE_TX_ADDR_1   = 5'd4,
    STATE_TX_ADDR_2   = 5'd5,
    STATE_TX_ADDR_3   = 5'd6,
    STATE_TX_WDATA_0  = 5'd7,
    STATE_TX_WDATA_1  = 5'd8,
    STATE_TX_WDATA_2  = 5'd9,
    STATE_TX_WDATA_3  = 5'd10,
    STATE_RX_WR_HEADER = 5'd11,
    STATE_RX_RD_HEADER = 5'd12,
    STATE_RX_RDATA_0   = 5'd13,
    STATE_RX_RDATA_1   = 5'd14,
    STATE_RX_RDATA_2   = 5'd15,
    STATE_RX_RDATA_3   = 5'd16,
    STATE_COMPLETE    = 5'd17,
    
    // New states for Host-initiated requests
    STATE_HOST_REQ_ADDR_0   = 5'd18,  // Parse address byte 0
    STATE_HOST_REQ_ADDR_1   = 5'd19,  // Parse address byte 1
    STATE_HOST_REQ_ADDR_2   = 5'd20,  // Parse address byte 2
    STATE_HOST_REQ_ADDR_3   = 5'd21,  // Parse address byte 3
    STATE_HOST_REQ_WDATA_0  = 5'd22,  // Parse write data byte 0
    STATE_HOST_REQ_WDATA_1  = 5'd23,  // Parse write data byte 1
    STATE_HOST_REQ_WDATA_2  = 5'd24,  // Parse write data byte 2
    STATE_HOST_REQ_WDATA_3  = 5'd25,  // Parse write data byte 3
    STATE_HOST_BUS_REQ      = 5'd26,  // Assert host_bus_req
    STATE_HOST_BUS_WAIT     = 5'd27,  // Wait for host_bus_ready
    STATE_HOST_RESP_HEADER  = 5'd28,  // Send response header
    STATE_HOST_RESP_DATA_0  = 5'd29,  // Send response data byte 0
    STATE_HOST_RESP_DATA_1  = 5'd30,  // Send response data byte 1
    STATE_HOST_RESP_DATA_2  = 5'd31   // Send response data byte 2/3
} state_t;
```

#### 2. RX Buffer Implementation

Add circular buffer for incoming data:

```systemverilog
// ============================================================
// RX Circular Buffer (16 bytes)
// ============================================================
logic [7:0]  rx_buffer [0:15];
logic [3:0]  rx_wr_ptr;
logic [3:0]  rx_rd_ptr;
logic [4:0]  rx_count;

// Buffer empty/full conditions
wire rx_buffer_empty = (rx_count == 5'd0);
wire rx_buffer_full  = (rx_count >= 5'd14);  // Leave room for max packet

// RX ready when buffer has room
assign rx_ready = !rx_buffer_full || in_rx_phase;  // Always accept during RX phase

// Write to buffer on valid handshake
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_wr_ptr <= 4'd0;
        rx_count  <= 5'd0;
    end else if (rx_valid && rx_ready) begin
        rx_buffer[rx_wr_ptr] <= rx_data;
        rx_wr_ptr <= rx_wr_ptr + 4'd1;
        if (!(consuming_rx_byte))
            rx_count <= rx_count + 5'd1;
    end else if (consuming_rx_byte && !rx_buffer_empty) begin
        rx_count <= rx_count - 5'd1;
    end
end

// Read pointer advances when state machine consumes a byte
logic consuming_rx_byte;
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        rx_rd_ptr <= 4'd0;
    end else if (consuming_rx_byte && !rx_buffer_empty) begin
        rx_rd_ptr <= rx_rd_ptr + 4'd1;
    end
end

// Buffered byte available for state machine
wire [7:0] rx_buffered_byte = rx_buffer[rx_rd_ptr];
wire rx_byte_available = !rx_buffer_empty;
```

#### 3. Host Request Capture Registers

```systemverilog
// ============================================================
// Host-Initiated Request Registers
// ============================================================
logic [31:0] host_req_addr;      // Host request address
logic [31:0] host_req_wdata;     // Host request write data
logic        host_req_we;        // Host request write enable
logic [1:0]  host_req_size;      // Host request access size
logic [31:0] host_req_rdata;     // Host request read data (response)
```

#### 4. Modified State Machine Logic

The state machine needs modification to:
1. Check RX buffer for incoming packets in IDLE state
2. Parse packet type and route accordingly
3. Handle Host-initiated requests (type 0010)
4. Send responses (type 0011) after bus transaction completes

```systemverilog
always_comb begin
    next_state = state;
    consuming_rx_byte = 1'b0;
    
    case (state)
        STATE_IDLE: begin
            // Priority 1: Handle CPU-initiated request
            if (req) begin
                next_state = STATE_CAPTURE;
            end
            // Priority 2: Check RX buffer for incoming packet
            else if (rx_byte_available) begin
                // Peek at packet type
                case (rx_buffered_byte[7:4])
                    4'b0001: begin  // Response to CPU request - ERROR in IDLE
                        // Should not receive response without outstanding request
                        // Log error or handle gracefully
                    end
                    4'b0010: begin  // Host-initiated request
                        consuming_rx_byte = 1'b1;  // Consume header
                        // Note: In actual implementation, these would be registered
                        // in an always_ff block. Shown here conceptually.
                        // host_req_we = rx_buffered_byte[0];
                        // host_req_size = rx_buffered_byte[3:2];
                        next_state = STATE_HOST_REQ_ADDR_0;
                    end
                    default: ;  // Ignore unknown packet types
                endcase
            end
        end
        
        // ... (existing states for CPU-initiated path)
        
        // Host-initiated request handling states
        STATE_HOST_REQ_ADDR_0: begin
            if (rx_byte_available) begin
                consuming_rx_byte = 1'b1;
                // Note: addr capture done in separate always_ff block
                next_state = STATE_HOST_REQ_ADDR_1;
            end
        end
        
        // ... (similar for ADDR_1, ADDR_2, ADDR_3)
        
        STATE_HOST_REQ_ADDR_3: begin
            if (rx_byte_available) begin
                consuming_rx_byte = 1'b1;
                // Note: addr capture done in separate always_ff block
                if (host_req_we) begin
                    next_state = STATE_HOST_REQ_WDATA_0;
                end else begin
                    next_state = STATE_HOST_BUS_REQ;
                end
            end
        end
        
        // ... (similar for WDATA states)
        
        STATE_HOST_BUS_REQ: begin
            // Assert host_bus_req, wait for ready
            if (host_bus_ready) begin
                // Note: rdata capture done in separate always_ff block
                next_state = STATE_HOST_RESP_HEADER;
            end
        end
        
        STATE_HOST_RESP_HEADER: begin
            if (tx_ready) begin
                if (host_req_we) begin
                    next_state = STATE_IDLE;  // Write response is just header
                end else begin
                    next_state = STATE_HOST_RESP_DATA_0;  // Read needs data
                end
            end
        end
        
        // ... (response data states)
        
    endcase
end
```

#### 5. Bus Master Interface Connection

Connect the host request registers to the bus master interface:

```systemverilog
// ============================================================
// Bus Master Interface (Host→CPU path)
// ============================================================
assign host_bus_addr  = host_req_addr;
assign host_bus_wdata = host_req_wdata;
assign host_bus_we    = host_req_we;
assign host_bus_size  = host_req_size;
assign host_bus_req   = (state == STATE_HOST_BUS_REQ);
```

#### 6. Response TX Data Multiplexer

Add response packet transmission:

```systemverilog
// TX data for Host-initiated response (packet type 0011)
always_comb begin
    case (state)
        STATE_HOST_RESP_HEADER: begin
            // {packet_type=0011, size[1:0], 1'b0, we}
            tx_byte = {4'b0011, host_req_size, 1'b0, host_req_we};
            tx_valid_internal = 1'b1;
        end
        STATE_HOST_RESP_DATA_0: begin
            tx_byte = host_req_rdata[7:0];
            tx_valid_internal = 1'b1;
        end
        STATE_HOST_RESP_DATA_1: begin
            tx_byte = host_req_rdata[15:8];
            tx_valid_internal = 1'b1;
        end
        // ... etc
        default: ;
    endcase
end
```

---

## Rust Implementation Details

### File: `cpu-sim/src/sim.rs`

#### 1. New SimulatorView Methods

Add methods for host-initiated bus requests:

```rust
impl<'a> SimulatorView<'a> {
    /// Send a bus request from the host to the FPGA
    ///
    /// This initiates a host-to-FPGA bus transaction. The request will be
    /// serialized and sent via the RX interface to the host_bus_interface
    /// module, which will execute the transaction on the FPGA's internal bus.
    ///
    /// # Arguments
    /// * `addr` - Target address for the bus transaction
    /// * `we` - Write enable (true = write, false = read)
    /// * `size` - Access size (0 = byte, 1 = halfword, 2 = word)
    /// * `wdata` - Write data (only used when we = true)
    ///
    /// # Returns
    /// * `Ok(())` if request was successfully queued
    /// * `Err(String)` if a request is already outstanding
    pub fn send_bus_request(
        &mut self,
        addr: u32,
        we: bool,
        size: u8,
        wdata: u32,
    ) -> Result<(), String> {
        // Check if a request is already outstanding
        if self.pending_host_request.is_some() {
            return Err("Host bus request already outstanding".to_string());
        }
        
        // Store the request
        self.pending_host_request = Some(HostBusRequest {
            addr,
            we,
            size,
            wdata,
            state: HostRequestState::Pending,
        });
        
        Ok(())
    }
    
    /// Receive a bus response from the FPGA
    ///
    /// Checks if a response has been received for the outstanding host-initiated
    /// bus request.
    ///
    /// # Returns
    /// * `Some(rdata)` if a response has been received (read data for reads, 0 for writes)
    /// * `None` if no response is available yet
    pub fn receive_bus_response(&mut self) -> Option<u32> {
        if let Some(ref mut req) = self.pending_host_request {
            if matches!(req.state, HostRequestState::Complete(rdata)) {
                let rdata = match req.state {
                    HostRequestState::Complete(data) => data,
                    _ => 0,
                };
                self.pending_host_request = None;
                return Some(rdata);
            }
        }
        None
    }
}
```

#### 2. New Types for Host Requests

```rust
/// State of a host-initiated bus request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostRequestState {
    /// Request is pending serialization
    Pending,
    /// Request has been sent, waiting for response
    WaitingForResponse,
    /// Response received with data
    Complete(u32),
}

/// Host-initiated bus request
#[derive(Debug, Clone)]
struct HostBusRequest {
    addr: u32,
    we: bool,
    size: u8,
    wdata: u32,
    state: HostRequestState,
}
```

#### 3. Modified Simulator State

Add pending request storage to Simulator:

```rust
pub struct Simulator<F, T>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    // ... existing fields ...
    
    // Host-initiated bus request (at most one outstanding)
    pending_host_request: Option<HostBusRequest>,
    
    // Host-initiated response state machine
    host_response_state: HostResponseState,
}
```

#### 4. Modified Step Function

Update the step function to handle bidirectional communication:

```rust
fn handle_host_bus_interface(&mut self) {
    // Priority 1: Serialize pending host request
    if let Some(ref mut req) = self.pending_host_request {
        if matches!(req.state, HostRequestState::Pending) {
            self.serialize_host_request(req);
        }
    }
    
    // Priority 2: Check for incoming response to host request
    if let Some(ref mut req) = self.pending_host_request {
        if matches!(req.state, HostRequestState::WaitingForResponse) {
            self.check_for_host_response(req);
        }
    }
    
    // Priority 3: Handle CPU-initiated requests (existing logic)
    // This MUST continue even when a host request is outstanding
    self.handle_cpu_initiated_request();
}

fn serialize_host_request(&mut self, req: &mut HostBusRequest) {
    // Build packet: header + address + (optional) write data
    // Header: {packet_type=0010, size[1:0], 1'b0, we}
    let header = 0x20 | ((req.size & 0x03) << 2) | (if req.we { 0x01 } else { 0x00 });
    
    // Queue bytes to RX
    self.host_rx_queue.push_back(header);
    self.host_rx_queue.push_back((req.addr >> 0) as u8);
    self.host_rx_queue.push_back((req.addr >> 8) as u8);
    self.host_rx_queue.push_back((req.addr >> 16) as u8);
    self.host_rx_queue.push_back((req.addr >> 24) as u8);
    
    if req.we {
        let bytes_to_send = match req.size {
            0 => 1,  // byte
            1 => 2,  // halfword
            _ => 4,  // word
        };
        for i in 0..bytes_to_send {
            self.host_rx_queue.push_back((req.wdata >> (i * 8)) as u8);
        }
    }
    
    req.state = HostRequestState::WaitingForResponse;
}

fn check_for_host_response(&mut self, req: &mut HostBusRequest) {
    // Check TX for response packet (type 0011)
    if self.cpu.host_tx_valid != 0 && self.host_response_state == HostResponseState::Idle {
        let header = self.cpu.host_tx_data;
        let packet_type = (header >> 4) & 0x0F;
        
        if packet_type == 0x03 {
            // This is a response to our request
            self.cpu.host_tx_ready = 1;
            
            let is_write = (header & 0x01) != 0;
            if is_write {
                // Write response - no data bytes
                req.state = HostRequestState::Complete(0);
            } else {
                // Read response - collect data bytes
                self.host_response_state = HostResponseState::CollectingData {
                    byte_idx: 0,
                    size: (header >> 2) & 0x03,
                    rdata: 0,
                };
            }
        }
    }
    // ... handle data collection states ...
}
```

---

## Testing Strategy

### Phase 1: RTL-Focused Tests (testbench/tests/host_bus_interface_test.rs)

#### 1.1 Host Request Packet Parsing Tests

```rust
#[test]
fn test_host_request_word_write() {
    // Send host-initiated word write request
    // Verify host_bus_* signals are correctly driven
    // Verify response packet is correctly formatted
}

#[test]
fn test_host_request_word_read() {
    // Send host-initiated word read request
    // Provide bus response data
    // Verify response packet contains correct read data
}

#[test]
fn test_host_request_byte_access() {
    // Test byte-sized host-initiated access
}

#[test]
fn test_host_request_halfword_access() {
    // Test halfword-sized host-initiated access
}
```

#### 1.2 RX Buffer Tests

```rust
#[test]
fn test_rx_buffer_basic() {
    // Verify data can be written to and read from buffer
}

#[test]
fn test_rx_buffer_full_request_plus_response() {
    // Fill buffer with request (9 bytes) + response (5 bytes)
    // Verify buffer handles both packets correctly
}

#[test]
fn test_rx_ready_deasserts_when_full() {
    // Verify rx_ready goes LOW when buffer is near capacity
}
```

#### 1.3 Concurrent Request Tests

```rust
#[test]
fn test_cpu_request_during_host_request() {
    // Start host-initiated request
    // While processing, CPU also makes a request
    // Verify both complete correctly
}

#[test]
fn test_host_request_during_cpu_request() {
    // Start CPU-initiated request
    // While waiting for response, send host request to buffer
    // Complete CPU request
    // Verify host request is then processed
}
```

### Phase 2: CPU-Level Tests (cpu-sim/tests/)

#### 2.1 Basic Host Request Tests

```rust
#[test]
fn test_host_request_led_read() {
    // Use send_bus_request to read LED peripheral
    // Verify receive_bus_response returns correct value
}

#[test]
fn test_host_request_led_write() {
    // Use send_bus_request to write LED peripheral
    // Verify write completed
    // Have CPU read back the value to confirm
}
```

#### 2.2 Memory Fence Synchronization Test (Minimal Proof of Concept)

This is the **critical validation test** that proves the host-initiated request system works:

```rust
#[test]
fn test_host_initiated_fence_synchronization() {
    init_test_logger();
    
    // Memory fence address in DRAM
    const FENCE_ADDR: u32 = 0x8000_1000;
    
    // Build CPU program:
    // 1. Load fence address into register
    // 2. Spin loop: while (memory[FENCE_ADDR] == 0) { }
    // 3. Exit via tohost
    let instructions = vec![
        lui(15, 0x80000),         // x15 = 0x80000000 (LUI loads upper 20 bits)
        addi(15, 15, 0x1000),     // x15 = 0x80001000 (FENCE_ADDR)
        // Spin loop start (offset 0x08)
        lw(14, 15, 0),            // x14 = memory[FENCE_ADDR]
        beq(14, 0, -4),           // if x14 == 0, loop back to lw
        // Exit
        lui(7, 0x10000),          // x7 = 0x10000000 (tohost base)
        addi(8, 0, 1),            // x8 = 1 (success code)
        sw(7, 8, 0),              // write to tohost
        jal(0, 0),                // infinite loop (halt)
    ];
    
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();
    
    let mut fence_written = false;
    let iterations = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let iterations_clone = iterations.clone();
    
    let result = run_program(
        100_000,  // Max cycles
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            // This callback runs after each instruction completes
            let count = iterations_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            
            // After some iterations, use host-initiated request to break the fence
            if count == 50 && !fence_written {
                // Write 1 to FENCE_ADDR via host-initiated bus request
                sim.send_bus_request(FENCE_ADDR, true, 2, 1)
                    .expect("Failed to send bus request");
                fence_written = true;
            }
            
            // Poll for response (will be None until request completes)
            if fence_written {
                if let Some(_) = sim.receive_bus_response() {
                    // Request completed - CPU should break out of loop now
                }
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Setup: Write program and initialize fence to 0
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            sim.write_memory_region(FENCE_ADDR, &[0, 0, 0, 0], false);  // fence = 0
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");
    
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit via tohost after fence is released"
    );
}
```

#### 2.3 RTL Peripheral Access Tests

```rust
#[test]
fn test_host_request_clock_peripheral() {
    // Read elapsed time from clock peripheral via host request
}

#[test]
fn test_host_request_uart_status() {
    // Read UART status register via host request
}
```

---

## Implementation Checklist

### RTL Changes (`rtl/host_bus_interface.sv`)

- [ ] Add RX circular buffer (16 bytes)
- [ ] Add buffer write logic with rx_valid/rx_ready handshake
- [ ] Add buffer read logic for state machine consumption
- [ ] Add host request capture registers
- [ ] Add new states for parsing Host-initiated requests (type 0010)
- [ ] Add new states for sending FPGA responses (type 0011)
- [ ] Connect host request registers to bus master interface
- [ ] Add TX mux for response packets
- [ ] Update rx_ready signal to account for buffer status
- [ ] Ensure CPU-initiated path still works correctly (regression)

### Rust Changes (`cpu-sim/src/sim.rs`)

- [ ] Add `HostBusRequest` and `HostRequestState` types
- [ ] Add `pending_host_request` field to Simulator
- [ ] Add `host_response_state` field to Simulator
- [ ] Implement `send_bus_request()` method on SimulatorView
- [ ] Implement `receive_bus_response()` method on SimulatorView
- [ ] Modify `handle_host_bus_interface()` to serialize host requests
- [ ] Add response parsing logic for packet type 0011
- [ ] Ensure CPU-initiated handling continues during host request

### Testing

- [ ] RTL: Host request word write/read tests
- [ ] RTL: Host request byte/halfword access tests
- [ ] RTL: RX buffer basic functionality tests
- [ ] RTL: RX buffer capacity tests (request + response)
- [ ] RTL: Concurrent CPU and Host request tests
- [ ] CPU-sim: Basic host request to LED peripheral
- [ ] CPU-sim: Host request to clock peripheral
- [ ] CPU-sim: Memory fence synchronization test (critical POC)
- [ ] CPU-sim: Regression tests for existing functionality

---

## Risk Assessment

### High Risk Items
1. **Deadlock scenarios**: Careful adherence to rules is critical
2. **Buffer overflow**: RX buffer must be sized correctly
3. **State machine complexity**: Additional states increase verification burden

### Mitigation Strategies
1. Extensive simulation testing with concurrent request scenarios
2. Formal verification of buffer sizing logic
3. Comprehensive test coverage for all state transitions

---

## Timeline Estimate

| Phase | Description | Estimated Time |
|-------|-------------|----------------|
| 1 | RTL RX buffer implementation | 2-3 hours |
| 2 | RTL state machine additions | 3-4 hours |
| 3 | Rust simulator changes | 2-3 hours |
| 4 | RTL-focused tests | 2-3 hours |
| 5 | CPU-sim tests | 2-3 hours |
| 6 | Integration testing & debugging | 2-4 hours |
| **Total** | | **13-20 hours** |

---

## References

- `rtl/host_bus_interface.sv` - Current implementation
- `rtl/bus_arbiter.sv` - Bus arbitration logic
- `rtl/bus.sv` - Address decoding
- `cpu-sim/src/sim.rs` - Rust simulator implementation
- `testbench/tests/host_bus_interface_test.rs` - Existing RTL tests
- `cpu-sim/tests/test_led_peripheral.rs` - Example cpu-sim test pattern

# Host Bus Protocol Status

## Current State (WIP)

The bi-directional host bus protocol implementation has protocol-level conflicts that prevent reliable operation of host-initiated requests when the CPU is actively executing (busy-loop scenarios).

## Problem Description

When host-initiated bus requests are queued during instruction complete callbacks, they conflict with CPU-initiated fetch requests. Both the CPU and host try to use the same shared serial bus simultaneously, leading to deadlocks:

```
Deadlock condition:
- CPU: tx_valid=1, wants to send fetch request
- Host: rx_valid=1, wants to send LED write request  
- Neither accepts: tx_ready=0, rx_ready=0
- Both stuck waiting for the other
```

## Root Cause

The `host_bus_interface.sv` RTL module manages TWO state machines sharing ONE serial interface:
1. **CPU-initiated requests**: CPU → bus → host_bus_interface (slave port) → Rust
2. **Host-initiated requests**: Rust → host_bus_interface (RX) → bus_arbiter → RTL peripherals

When both state machines are active, they compete for the TX/RX signals, causing protocol corruption.

## Attempted Fixes

### 1. Idle Period Requirement (Partial Solution)
- Host requests wait for 5 idle cycles before starting
- Ensures RTL has time to return to IDLE state
- **Issue**: In busy-loop scenarios, idle periods are too short/infrequent

### 2. Deadlock Detection & Recovery  
- Detects `tx_valid=1 && rx_valid=1 && !tx_ready && !rx_ready`
- Host backs off and requeues request
- **Issue**: Detection happens after deadlock, not prevention

### 3. Stall Timeout
- Aborts host requests that don't complete within timeout
- **Issue**: Doesn't address root cause of conflicts

## What Works

- ✅ CPU-initiated requests (fetch, load, store to DRAM/host peripherals)
- ✅ Host responses to CPU requests
- ✅ Basic API types (`HostBusRequest`, `HostBusResponse`)
- ✅ Address validation (host can only access RTL peripheral range)

## What Doesn't Work

- ❌ Host-initiated requests during CPU busy loops
- ❌ Sequential host requests
- ❌ Host read/write to LED peripheral while CPU is running

## Test Status

```
Passing:  3/10 tests
- test_host_bus_request_types_exist
- test_fpga_error_types_exist  
- test_host_request_invalid_address_rejected_by_api

Failing: 7/10 tests (all involve actual host→RTL communication)
- test_host_write_led_simple
- test_host_read_led_after_cpu_write
- test_host_write_during_cpu_activity
- test_host_bus_end_to_end_led
- test_host_write_byte
- test_host_write_halfword
- test_sequential_host_requests
```

## Recommended Solutions

### Option 1: RTL Signal Enhancement (Cleanest)
Add explicit `rtl_idle` output to `host_bus_interface.sv`:

```systemverilog
// New output signal
output logic rtl_idle,  // Asserted when module is ready for host requests

// Implementation
assign rtl_idle = (state == STATE_IDLE) && 
                  !req &&  // No pending CPU request
                  !host_bus_req;  // No active host bus transaction
```

**Pros**: Clean separation, explicit protocol state  
**Cons**: Requires RTL changes, must verify FPGA synthesis

### Option 2: Request Buffering in RTL (Most Robust)
Implement a FIFO queue in `host_bus_interface.sv` to buffer host requests:

```systemverilog
// Add FIFO for host requests
sync_fifo #(.WIDTH(8), .DEPTH(4)) host_req_fifo (...);

// Accept host requests even when busy processing CPU request
// Process them sequentially when idle
```

**Pros**: Handles high-frequency scenarios, fully decoupled  
**Cons**: More complex RTL, increases resource usage

### Option 3: Software Rate Limiting (Temporary Workaround)
Modify test pattern to include explicit idle windows:

```rust
// Instead of tight busy loop:
lw x11, 0x100(x13)
beq x11, x0, -4

// Use loop with delay:
lw x11, 0x100(x13)
beq x11, x0, +8
nop  // delay slots
nop
nop
nop
j loop_start
```

**Pros**: No RTL changes required  
**Cons**: Doesn't solve fundamental issue, reduces CPU utilization

## Next Steps

1. **Short-term**: Implement Option 3 to unblock testing
2. **Medium-term**: Implement Option 1 for clean protocol
3. **Long-term**: Consider Option 2 if high-frequency host requests are needed

## Files Modified

- `cpu-sim/src/sim.rs`: Added coordination fields and deadlock detection
- `cpu-sim/tests/test_host_bus_requests.rs`: Increased cycle limits for debugging

## Additional Context

- The RTL bus arbiter already implements Host > CPU priority for bus access
- The protocol uses variable-length packets (1-9 bytes depending on operation)
- Memory latency can add additional cycles to CPU requests
- The hung detector triggers after 10,000 cycles without instruction completion

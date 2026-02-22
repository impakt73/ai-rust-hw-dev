# Host Bus Multi-Unit Transfer Expansion Plan

## 1. Goal

Upgrade the host bus protocol and implementation so a **single host transaction** can transfer many units (byte/halfword/word), including large bursts for program/data movement and FIFO-style transfers where address increment can be disabled.

The design target includes:

1. Single read returning at least 5 consecutive words.
2. Single write sending at least 5 consecutive words.
3. **16-bit transfer count** support for large transfers.
4. Optional increment suppression for source/destination behavior needed by FIFO-like interfaces.

---

## 2. Scope and non-goals

### 2.1 In scope

1. Replace current host-bus packet layout with a single new burst-native protocol.
2. Update RTL (`host_bus_interface`, `host_rx_buffer`) for multi-unit execution.
3. Update Rust protocol mirror (`host-bus-handler`) and runtime bulk-transfer paths.
4. Preserve the **single outstanding request** rule as a hard requirement.
5. Add focused tests for burst correctness and no-increment modes.

### 2.2 Out of scope

1. Multiple outstanding transaction support.
2. CPU core protocol changes.
3. Unrelated arbiter redesign.

---

## 3. Current baseline (verified)

1. Current wire format uses a 1-byte header with packet type/size/we.
2. Current host requests are single-unit.
3. `device-runtime` bulk memory helpers currently loop through many single requests.

These constraints are the primary reason large transfers are slower than needed.

---

## 4. New protocol specification (single protocol)

This plan uses one protocol format only. There is no legacy/compatibility mode.
It intentionally replaces the previous packet-type encoding and wire layout.

### 4.1 Header format (optimized, fixed 3-byte header)

#### Byte 0: control

- `[7:6] kind`
  - `00` = host request
  - `01` = FPGA response
  - `10/11` = reserved
- `[5] we` (`1` write, `0` read)
- `[4:3] size` (`00` byte, `01` halfword, `10` word, `11` reserved)
- `[2] addr_inc` (`1` increment target address per unit, `0` hold target address)
- `[1] host_buf_inc` (`1` increment host-side buffer index, `0` hold host buffer index)
- `[0] status_or_reserved`
  - Request: must be `0`
  - Response: `0` success, `1` error

#### Byte 1-2: transfer count

- `transfer_count_minus_1` in little-endian `u16` (wire values `0..=65535`)
- effective transfer count = `u16_value + 1` (semantic range `1..=65536`)
- supports `1..=65536` units per packet

### 4.2 Payload format

Request payload (`kind=00`):

1. `addr` (4 bytes, little-endian)
2. write data for `we=1`: `transfer_count * unit_bytes`

Response payload (`kind=01`):

1. read data for `we=0`: `transfer_count * unit_bytes`
2. write responses (`we=1`) carry no data when successful
3. on error (`status_or_reserved=1`), payload includes failing unit index (`u16`, little-endian) for deterministic diagnostics

### 4.3 Addressing/data movement semantics

Per unit `i`:

- `unit_bytes = {1,2,4}` from `size`
- target address:
  - `addr + i*unit_bytes` if `addr_inc=1`
  - `addr` if `addr_inc=0`
- host buffer index:
  - `i` if `host_buf_inc=1`
  - `0` if `host_buf_inc=0`

This supports:

1. Normal contiguous copies (`addr_inc=1`, `host_buf_inc=1`)
2. FIFO read/write endpoints (`addr_inc=0`, `host_buf_inc=1`)
3. Constant-pattern fill/drain (`host_buf_inc=0`)

---

## 5. Mandatory protocol/flow rules

1. **Single outstanding request is mandatory** across RTL and host software.
2. A burst request is atomic at protocol level: one request packet → one response packet.
3. Payload ordering is always little-endian.
4. Wire encoding stores `count-1`, so wire value `0` means one transfer; there is no wire encoding for zero transfers, and API-level validation must reject zero before serialization.

---

## 6. Rust data model/API changes

### 6.1 `host-bus-handler` request model

Move from single-unit request to burst descriptor:

```rust
pub struct BusRequest {
    pub addr: u32,
    pub size: AccessSize,
    pub we: bool,
    pub transfer_count: u16, // 1..=65536 (encoded as count-1 on wire)
    pub addr_inc: bool,
    pub host_buf_inc: bool,
    pub wdata: Vec<u8>,      // for writes
}
```

Implementation requirements:

1. Keep `read(...)` / `write(...)` convenience wrappers mapping to `transfer_count=1`.
2. Add `read_multi(...)` / `write_multi(...)` constructors.
3. Enforce payload sizing rules for write requests.
4. Keep single-outstanding guard (`RequestPending`) unchanged.

### 6.2 `device-runtime` bulk helpers

Add multi-unit helpers and route existing region helpers through them:

1. `read_memory_region_multi(start_addr, len, addr_inc, host_buf_inc, ...)`
2. `write_memory_region_multi(start_addr, data, addr_inc, host_buf_inc, ...)`

Behavior:

- Prefer word bursts for aligned ranges.
- Use halfword/byte bursts for tails and alignment constraints.
- Chunk by max units per packet (`<= 65536`, inclusive upper bound).

---

## 7. RTL implementation plan

### 7.1 Files

1. `rtl/host_rx_buffer.sv`
2. `rtl/host_bus_interface.sv`

### 7.2 RX parser (`host_rx_buffer`)

1. Parse fixed 3-byte header.
2. Decode `u16 transfer_count_minus_1` and compute active count.
3. Buffer request metadata: `we`, `size`, `addr_inc`, `host_buf_inc`, `transfer_count`, `addr`.
4. For writes, receive `count * unit_bytes` write payload.
5. Emit packet only when complete.

### 7.3 Bus execution (`host_bus_interface`)

1. For host request, run per-unit loop internally with computed address/index behavior.
2. Execute one bus transaction per unit while preserving single external outstanding request contract.
3. For read bursts, collect and transmit aggregated response payload.
4. For write bursts, transmit single ack response after all units complete.
5. On fault/timeout, set error status and terminate burst deterministically.

### 7.4 Throughput notes

1. This keeps control overhead nearly constant per packet.
2. Large `u16` burst count reduces per-request framing overhead for program loading and memory copy traffic.

---

## 8. Testing plan

### 8.1 `host-bus-handler` unit tests

1. Header encode/decode for all control combinations.
2. 5-word read burst decode/response assembly.
3. 5-word write burst serialization.
4. `addr_inc=0` repeated-address behavior.
5. `host_buf_inc=0` repeated-host-index behavior.
6. Invalid packet handling (bad size, malformed payload length, reserved kinds).
7. Single-outstanding enforcement.

### 8.2 RTL tests (`testbench/tests/host_bus_interface_test.rs`)

1. Host read burst of 5 words produces one response with 20 payload bytes.
2. Host write burst of 5 words executes 5 internal writes and one ack.
3. FIFO-write mode (`addr_inc=0`) targets one address repeatedly.
4. FIFO-read mode (`addr_inc=0`) reads one address repeatedly.
5. Error-path test: forced timeout/fault returns `status_or_reserved=1`.
6. Single-outstanding behavior preserved during burst operation.

### 8.3 Runtime integration tests (`device-runtime/tests`)

1. Contiguous DRAM copy using burst path matches source bytes.
2. FIFO-like peripheral interaction using `addr_inc=0` works for read/write.
3. Existing memory-region APIs continue returning identical data semantics.

### 8.4 Validation commands

1. `cargo test -p host-bus-handler`
2. `cargo test --package testbench --test host_bus_interface_test`
3. `cargo test -p device-runtime --test test_rtl_peripherals`
4. `cargo test -q`

---

## 9. Implementation sequence (AI-agent checklist)

1. Update protocol constants/docs and `host-bus-handler` packet encode/decode.
2. Add burst request metadata and helpers (`read_multi`/`write_multi`).
3. Update RTL RX buffering and state machines for fixed 3-byte header + `u16` count.
4. Implement burst execution and response packaging in `host_bus_interface`.
5. Add/adjust RTL and Rust tests for burst/no-increment/error paths.
6. Migrate `device-runtime` memory region helpers to burst operations.
7. Run targeted tests, then full regression.

---

## 10. Risks and mitigations

1. **Large burst timeout risk**
   - Mitigation: explicit timeout policy and response error status with failing index.
2. **Payload sizing bugs**
   - Mitigation: strict length validation and unit tests around edge counts/sizes.
3. **State-machine complexity growth**
   - Mitigation: keep parser and executor loops explicit and test each mode (`addr_inc`/`host_buf_inc`) independently.

---

## 11. Definition of done

1. One host request can read/write at least 5 words in one transaction.
2. Protocol supports up to 16-bit transfer count semantics (`1..=65536` units).
3. FIFO-style mode with no address increment is verified.
4. Single outstanding request rule is preserved and tested.
5. Bulk transfer paths in runtime use the new burst protocol.

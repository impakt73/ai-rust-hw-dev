# Host Bus Multi-Unit Transfer Expansion Plan

## 1. Goal

Add protocol and implementation support for **multi-unit (burst) host bus transfers** so one host request can transfer multiple consecutive units (byte/halfword/word), including modes that **optionally suppress source or destination address increment** for FIFO-like MMIO endpoints.

This plan is written for direct execution by an AI coding agent and is intentionally implementation-oriented.

---

## 2. Scope and Non-Goals

### 2.1 In scope

1. Extend host-bus packet format to represent transfer count and increment behavior.
2. Implement burst handling in:
   - RTL `host_bus_interface` RX/TX state machines
   - Rust `host-bus-handler` protocol mirror
   - Runtime integration paths that currently issue one request per unit
3. Preserve existing single-transfer behavior and backward compatibility.
4. Add targeted tests for protocol encode/decode, request execution, and FIFO-style no-increment transfers.

### 2.2 Out of scope

1. Reworking unrelated bus arbitration policy.
2. Introducing multiple concurrent outstanding host requests (keep single outstanding transaction model).
3. Changing CPU core memory protocol.

---

## 3. Current Baseline (verified in repo)

1. Protocol header is currently 1 byte with fields:
   - packet type `[7:4]`
   - size `[3:2]`
   - reserved `[1]`
   - we `[0]`
2. Requests/responses currently carry one transfer unit per transaction.
3. `device-runtime::write_memory_region` and `read_memory_region` loop over many single requests (`send_host_request` + poll response each step), causing overhead for bulk traffic.
4. `host-bus-handler::BusRequest` represents one address + one data word + one access size.

---

## 4. Protocol Extension

### 4.1 Design constraints

1. Keep existing packet type values (`0000`, `0001`, `0010`, `0011`).
2. Keep little-endian payload ordering.
3. Make legacy packets still valid and decodable.
4. Keep deterministic framing (receiver always knows exact payload length after header parsing).

### 4.2 Extended request/response framing

Use **2-byte header** for burst-capable packets (types `0010` and `0011`), with compatibility fallback for legacy single transfers.

#### Byte 0 (base header, retained)

- `[7:4]` packet_type
- `[3:2]` size (00 byte, 01 half, 10 word)
- `[1]` burst_mode (0=legacy single-transfer framing, 1=extended framing)
- `[0]` we

#### Byte 1 (extended control, only when burst_mode=1)

- `[7:0]` transfer_count_minus_1 (0 => 1 transfer, 4 => 5 transfers)

#### Byte 2 (extended flags, only when burst_mode=1)

- `[0]` addr_inc (1=increment target address by transfer width each unit, 0=hold address constant)
- `[1]` host_buf_inc (1=increment host-side buffer index, 0=reuse same source/destination element)
- `[7:2]` reserved (0)

> Why two increment flags: for FIFO-like operation you often hold one side constant but increment the other. `addr_inc` controls target bus address stepping; `host_buf_inc` controls host memory buffer stepping in runtime helpers.

#### Payload

- Request (`type 0010`):
  - address (4 bytes)
  - write data payload for writes: `transfer_count * unit_bytes`
- Response (`type 0011`):
  - read data payload for reads: `transfer_count * unit_bytes`

#### Backward compatibility

1. If `burst_mode=0`, parse exactly as current protocol (single transfer).
2. Emit legacy framing by default for count=1 + default increments to reduce risk.
3. Add a capability/version knob so host software can force legacy mode when connected to older FPGA images.

---

## 5. Data Model & API Changes (Rust)

### 5.1 `host-bus-handler` request model

Replace single-unit assumptions with explicit transfer descriptor:

```rust
pub struct BusRequest {
    pub addr: u32,
    pub size: AccessSize,
    pub we: bool,
    pub wdata: Vec<u8>,        // packed little-endian payload for writes
    pub transfer_count: u16,   // >=1
    pub addr_inc: bool,
    pub host_buf_inc: bool,
}
```

Implementation notes:

1. Keep ergonomic constructors:
   - existing `read(...)` / `write(...)` become wrappers for `transfer_count=1`
   - add `read_multi(...)`, `write_multi(...)`.
2. Enforce payload length = `transfer_count * size.byte_count()` for writes unless `host_buf_inc=false` (then allow one unit and replicate on transmit).
3. Keep single outstanding request rule unchanged.

### 5.2 Runtime bulk API

Add high-level APIs in `device-runtime`:

1. `read_memory_region_multi(start_addr, len_bytes, addr_inc, host_buf_inc, ...)`
2. `write_memory_region_multi(start_addr, data, addr_inc, host_buf_inc, ...)`

Behavior:

- Default (`addr_inc=true`, `host_buf_inc=true`) accelerates normal contiguous copies.
- FIFO read mode: `addr_inc=false`, `host_buf_inc=true`.
- FIFO write mode: `addr_inc=false`, `host_buf_inc=true`.
- Constant-pattern fill mode: `addr_inc=true`, `host_buf_inc=false`.

Chunking rules:

1. Split operations into chunks fitting `u8`/protocol transfer count max (255+1 = 256 units) unless larger count encoding is chosen.
2. Maintain alignment-aware access-size selection (word/half/byte), but prefer larger units in bulk path.

---

## 6. RTL Implementation Plan

### 6.1 Files to modify

1. `rtl/host_bus_interface.sv`
2. `rtl/host_rx_buffer.sv`
3. (if needed) supporting wrapper/definitions referenced by those modules

### 6.2 RX path (`host_rx_buffer`) updates

1. Extend header parser to detect `burst_mode` and capture `transfer_count` + flags.
2. Expand internal request buffer metadata:
   - `req_transfer_count`
   - `req_addr_inc`
   - `req_host_buf_inc` (can be ignored by pure RTL data movement but should be preserved for symmetry/forwarding if needed)
3. Update write-data receive FSM to accept `N * bytes_for_size(size)` bytes.
4. For read requests, complete packet after addr + extended header parse.
5. Preserve existing behavior for legacy packets.

### 6.3 Main host bus interface state machine

1. Bus-master execution loop for host-initiated request:
   - Iterate `i in [0..transfer_count)`
   - `effective_addr = base_addr + (addr_inc ? i * unit_bytes : 0)`
   - Issue one internal bus access per unit
2. Response TX logic for burst reads:
   - Header (extended)
   - Stream `N` data units in order
3. Write response logic for burst writes:
   - Ack once for full burst completion (not per unit)
4. Error/timeout strategy:
   - If any unit fails/timeout, terminate burst and emit error response (if protocol has no explicit status today, add status bit in extended flags or reserve a packet subtype)

### 6.4 Throughput/latency behavior

1. Keep current single outstanding transaction externally.
2. Internally allow one request to consume many bus cycles while remaining atomic from host protocol perspective.

---

## 7. Host Software Integration (Sim + FPGA)

### 7.1 `host-bus-handler` encode/decode state machines

1. TX state machine:
   - emit extended header bytes when `transfer_count>1` or non-default increment flags
   - stream request write payload across multiple units
2. RX state machine:
   - parse extended response header
   - collect `N` units for read responses
3. Keep `can_accept_rx` and buffering rules compatible with existing deadlock prevention.

### 7.2 Device runtime usage

1. Switch `read_memory_region`/`write_memory_region` internals to multi-unit API for aligned spans.
2. Keep fallback to legacy one-unit requests for unaligned tail fragments if needed.
3. Ensure timeout events include enough context (base addr + unit index) for diagnostics.

### 7.3 CLI and tooling (optional but recommended)

1. Add optional command arguments in `fpga-host` shell for burst count and increment flags.
2. Keep default command UX unchanged.

---

## 8. Compatibility & Rollout Strategy

### 8.1 Capability negotiation

Introduce a lightweight protocol capability check at runtime startup (or explicit config):

1. `legacy` mode: force old framing (count=1 only)
2. `burst_v1` mode: enable extended framing/features

### 8.2 Staged rollout

1. Phase A: parser support + legacy parity tests.
2. Phase B: write bursts (`we=1`) end-to-end.
3. Phase C: read bursts (`we=0`) end-to-end.
4. Phase D: no-increment modes + runtime adoption in bulk memory paths.

---

## 9. Testing Plan

### 9.1 Unit tests (`host-bus-handler`)

1. Header encode/decode for legacy and extended forms.
2. Read burst with `transfer_count=5` returns 5 words in order.
3. Write burst payload serialization for mixed sizes.
4. `addr_inc=false` validates repeated same target address behavior.
5. `host_buf_inc=false` validates repeated host element behavior.
6. Rejection tests: invalid size code, zero transfer count, payload length mismatch.

### 9.2 RTL tests (`testbench/tests/host_bus_interface_test.rs`)

Add focused tests mirroring protocol behavior:

1. Host-initiated read burst of 5 words returns one response packet with 20 data bytes.
2. Host-initiated write burst of 5 words issues five bus writes then one ack.
3. FIFO write test: `addr_inc=false` verifies all internal writes target one address.
4. FIFO read test: `addr_inc=false` verifies repeated reads from one address.
5. Legacy single-transfer tests remain passing unchanged.

### 9.3 Runtime integration tests (`device-runtime/tests`)

1. Bulk contiguous transfer speed-path correctness versus existing per-word baseline.
2. FIFO-like transfer against RTL peripheral register window.
3. Regression tests for existing `read_memory_region` / `write_memory_region` behavior.

### 9.4 Validation commands

1. `cargo test -p host-bus-handler`
2. `cargo test --test host_bus_interface_test`
3. `cargo test -p device-runtime --test test_rtl_peripherals`
4. full regression once targeted tests pass: `cargo test -q`

---

## 10. Implementation Sequence (AI-Agent Checklist)

1. Add/adjust protocol constants and request metadata structs in `host-bus-handler`.
2. Update TX/RX state machines in `host-bus-handler`; make existing tests pass.
3. Extend RTL packet parsing/storage (`host_rx_buffer.sv`) and bus execution/response emit (`host_bus_interface.sv`).
4. Update RTL testbench helpers for extended header generation and expected payload lengths.
5. Add runtime bulk APIs and migrate memory region helpers to use them.
6. Add integration tests for burst + no-increment modes.
7. Run targeted tests, then full regression.
8. Update docs where protocol header is currently documented as 1-byte only.

---

## 11. Risks and Mitigations

1. **Framing ambiguity risk**
   - Mitigation: explicit `burst_mode` bit + strict parser state machine + exhaustive malformed-packet tests.
2. **Large payload buffering pressure in RTL**
   - Mitigation: stream through FSM per unit instead of requiring full-packet buffering.
3. **Backward compatibility break**
   - Mitigation: maintain legacy parser/emitter path and capability gating.
4. **Timeout semantics for partial bursts**
   - Mitigation: define deterministic fail-fast behavior and emit actionable timeout metadata.

---

## 12. Definition of Done

1. A single host read request can return at least 5 consecutive words in one transaction.
2. A single host write request can send at least 5 consecutive words in one transaction.
3. `addr_inc=false` mode works for FIFO-like target endpoints.
4. `host_buf_inc=false` mode works for constant-source/constant-destination patterns.
5. Legacy single-transfer behavior remains compatible and covered by tests.
6. Bulk memory transfer helpers in runtime use the new mechanism by default (when capability enabled).

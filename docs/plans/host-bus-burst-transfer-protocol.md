# Host Bus Burst Transfer Protocol Implementation Plan

## 1. Goal / Scope

### 1.1 Goal
Add native burst transfer support to the host bus packet protocol so host-initiated memory reads/writes can transfer up to **64K words per request** (65,536 beats when `size=word`), while preserving current user-facing behavior of memory APIs.

### 1.2 In Scope
- Host bus packet protocol rewrite to include burst-native fields.
- RTL updates in host bus path only:
  - `rtl/io/host_bus_interface.sv`
  - `rtl/io/host_bus_rx.sv`
  - `rtl/io/host_bus_tx.sv`
  - related host-bus testbenches.
- Rust host-side protocol/transport updates:
  - `host-bus-handler` packet encode/decode/state machines.
  - `device-runtime` low-level region read/write logic (primary isolation point).

### 1.3 Out of Scope
- **No changes to the RTL system bus fabric/arbitration protocol itself** (no new bus signals, no bus protocol redesign).
- No requirement to keep binary packet encoding backward compatible with the prior protocol.
- No API-level behavior regressions for existing memory region operations.

### 1.4 Compatibility Requirements
- Protocol byte encoding compatibility: **not required**.
- User-facing functionality compatibility: **required**.
- Bidirectional protocol support (host-initiated and target-initiated flows): **required**.
- Single read/write transactions must remain representable as `burst_len = 1`.

---

## 2. Protocol Specification (Burst-Native)

## 2.1 Common Fields

All packet types move to a unified burst-capable framing format:

```
Byte 0: CTRL0
  [7:4] packet_type
        0000 = CPU-initiated request      (FPGA -> Host)
        0001 = Host response to CPU req   (Host -> FPGA)
        0010 = Host-initiated request     (Host -> FPGA)
        0011 = FPGA response to host req  (FPGA -> Host)
  [3:2] size (00=byte, 01=halfword, 10=word, 11=invalid)
  [1]   src_fixed   (1 = source address does not increment per beat)
  [0]   dst_fixed   (1 = destination address does not increment per beat)

Byte 1: CTRL1
  [0]   we (1=write, 0=read)
  [7:1] reserved (must be 0)

Bytes 2..3: burst_len_m1 (little-endian u16)
  effective_burst_len = burst_len_m1 + 1
  legal range: 1..65536 beats

Bytes 4..7: base_addr (little-endian u32)

Bytes 8..N: payload (if required by type + we)
```

### 2.2 Address Increment Semantics

- Address stride per beat = `1 << size` bytes.
- If increment is enabled, beat `i` address is:
  - `base_addr + i * stride`
- If fixed flag is set for the active side, beat `i` uses:
  - `base_addr`

Flag usage by operation:
- **Write transaction:** destination is bus target address (`dst_fixed` controls FIFO-style repeated writes to one address).
- **Read transaction:** source is bus target address (`src_fixed` controls FIFO-style repeated reads from one address).

Both flags remain present in all packet types so protocol remains direction-symmetric.

### 2.3 Packet Shapes

1) Request packets (`0000`, `0010`) always include:
- `CTRL0`, `CTRL1`, `burst_len_m1`, `base_addr`
- write payload only when `we=1`.

2) Response packets (`0001`, `0011`) always include:
- `CTRL0`, `CTRL1`, `burst_len_m1`, `base_addr` (echoed metadata for validation/correlation)
- read payload only when `we=0`.

### 2.4 Payload Length Rules

- `beat_bytes = {1,2,4}` from `size`.
- Write request payload bytes = `burst_len * beat_bytes`.
- Read response payload bytes = `burst_len * beat_bytes`.
- Read request and write response payload size = 0.

### 2.5 Mandatory Validation Checks

Receiver must reject/frame-drop packet when:
- `size == 2'b11`
- `CTRL1[7:1] != 0`
- computed payload length does not match received bytes
- burst length decodes to 0 (should be impossible with `len_m1 + 1`, but still guard)
- incrementing address overflows 32-bit range during beat iteration

---

## 3. Encoded Packet Examples

All examples use little-endian multibyte fields.

### 3.1 Burst Write (incrementing destination)

Write 4 words to `0x50001000`:
`[0x11223344, 0x55667788, 0x99AABBCC, 0xDDEEFF00]`

```
[0]  0x28  CTRL0: type=0010, size=word(10), src_fixed=0, dst_fixed=0
[1]  0x01  CTRL1: we=1
[2]  0x03  burst_len_m1[7:0]  (4 beats -> 3)
[3]  0x00  burst_len_m1[15:8]
[4]  0x00  base_addr[7:0]
[5]  0x10  base_addr[15:8]
[6]  0x00  base_addr[23:16]
[7]  0x50  base_addr[31:24]
[8]  0x44  beat0
[9]  0x33
[10] 0x22
[11] 0x11
[12] 0x88  beat1
[13] 0x77
[14] 0x66
[15] 0x55
[16] 0xCC  beat2
[17] 0xBB
[18] 0xAA
[19] 0x99
[20] 0x00  beat3
[21] 0xFF
[22] 0xEE
[23] 0xDD
```

### 3.2 Burst Read (incrementing source) + Response

Read 2 words from `0x50002000`.

Request:
```
[0] 0x28  CTRL0: type=0010, size=word, src_fixed=0, dst_fixed=0
[1] 0x00  CTRL1: we=0
[2] 0x01  burst_len_m1 (2 beats -> 1)
[3] 0x00
[4] 0x00
[5] 0x20
[6] 0x00
[7] 0x50
```

Response (example data `[0xA1A2A3A4, 0xB1B2B3B4]`):
```
[0] 0x38  CTRL0: type=0011, size=word, src_fixed=0, dst_fixed=0
[1] 0x00  CTRL1: we=0
[2] 0x01
[3] 0x00
[4] 0x00
[5] 0x20
[6] 0x00
[7] 0x50
[8] 0xA4
[9] 0xA3
[10] 0xA2
[11] 0xA1
[12] 0xB4
[13] 0xB3
[14] 0xB2
[15] 0xB1
```

### 3.3 FIFO-Style Non-Incrementing Write (`dst_fixed=1`)

Write 4 words to fixed FIFO data register at `0x50000020`:

```
[0] 0x29  CTRL0: type=0010, size=word, src_fixed=0, dst_fixed=1
[1] 0x01  we=1
[2] 0x03
[3] 0x00
[4] 0x20
[5] 0x00
[6] 0x00
[7] 0x50
[8..23] payload bytes for 4 words
```

### 3.4 FIFO-Style Non-Incrementing Read (`src_fixed=1`)

Read 8 words from fixed FIFO read register at `0x50000024`:

Request:
```
[0] 0x2A  CTRL0: type=0010, size=word, src_fixed=1, dst_fixed=0
[1] 0x00  we=0
[2] 0x07  burst_len_m1 (8 beats)
[3] 0x00
[4] 0x24
[5] 0x00
[6] 0x00
[7] 0x50
```

Response header starts with:
```
[0] 0x3A  CTRL0: type=0011, size=word, src_fixed=1, dst_fixed=0
[1] 0x00
...
```

---

## 4. RTL Plan

### 4.1 `host_bus_rx.sv`
- Replace fixed 1-address + optional 1-word payload parser with burst parser:
  - Parse 8-byte metadata prefix first.
  - Compute expected payload length from `size`, `we`, and `burst_len`.
  - Stream payload beats into an internal beat buffer/FIFO interface toward `host_bus_interface`.
- Add metadata outputs:
  - `packet_src_fixed`, `packet_dst_fixed`
  - `packet_burst_len` (u16, decoded as beats)
  - `packet_base_addr`
- Add explicit error/flush path for malformed packets.

### 4.2 `host_bus_tx.sv`
- Extend serializer to emit 8-byte metadata prefix.
- Support beat loop for payload transmit:
  - write responses emit no payload
  - read responses emit `burst_len * beat_bytes` payload
- Keep ready/valid behavior consistent with existing TX backpressure rules.

### 4.3 `host_bus_interface.sv` (core control change)

Implement a transaction-loop FSM that iterates beats without changing system bus protocol.

Proposed high-level states:
1. `S_IDLE` - wait for decoded packet or CPU-side request.
2. `S_LOAD_REQ` - latch packet metadata (`we/size/len/base/flags`).
3. `S_ISSUE_BEAT` - drive one bus master transfer.
4. `S_WAIT_BEAT_READY` - wait `host_bus_ready`.
5. `S_CAPTURE_READ_BEAT` - store `host_bus_rdata` beat for response payload.
6. `S_NEXT_BEAT` - increment beat counter and conditionally increment active bus address.
7. `S_ENQUEUE_RESP` - push response metadata+payload into TX path.
8. `S_TX_DRAIN` - wait until TX accepts full response packet.

Address update logic per beat:
- `addr_next = base_addr` when fixed flag active for that direction.
- `addr_next = base_addr + beat_idx * beat_bytes` otherwise.

Direction handling:
- Host-initiated packets (`0010`) run RX -> bus master loop -> TX response.
- CPU-initiated side remains supported (TX request / RX response path still operational).
- For CPU-originated requests (single-beat system bus transactions), protocol fields are encoded with `burst_len=1`, fixed flags cleared.

### 4.4 Internal Buffering Strategy
- Keep at most one active burst in `host_bus_interface` control path initially (matches current single-outstanding model).
- Use beat counter and small metadata registers.
- Read bursts require response payload staging (can be streaming into TX buffer if tx can accept per beat; otherwise small burst data FIFO/register RAM).

### 4.5 Affected RTL Files
- `rtl/io/host_bus_interface.sv`
- `rtl/io/host_bus_rx.sv`
- `rtl/io/host_bus_tx.sv`
- `testbench/tests/host_bus_interface_test.rs`
- `testbench/tests/host_rx_buffer_test.rs` (or renamed equivalent if test module naming is updated)

---

## 5. Rust Plan

### 5.1 `host-bus-handler` Protocol Layer
- Introduce burst-aware request/response structures (new fields: `burst_len`, `src_fixed`, `dst_fixed`).
- Update RX/TX state machines to:
  - parse/emit new 8-byte metadata prefix
  - stream variable payload lengths based on burst metadata
  - validate metadata echo for responses
- Preserve bidirectional handling behavior (`accept_request` / `complete_request` still works).

Likely touched:
- `host-bus-handler/src/lib.rs`
- `host-bus-handler/src/tests.rs`

### 5.2 `device-runtime` (isolate high-level impact here)

Primary design requirement: keep changes mostly under low-level region helpers.

In `device-runtime/src/lib.rs`:
- Rework `write_memory_region`:
  - chunk large writes into burst packets (max 65,536 beats per packet).
  - maintain current head/tail alignment behavior; use word bursts for aligned interiors.
  - keep existing public API unchanged.
- Rework `read_memory_region` similarly:
  - issue burst reads and splice response bytes into output buffer.
- Add explicit argument/config path for fixed-address FIFO mode when needed by callers (internally represented via flags).

Runtime transport glue:
- `device-runtime/src/fpga.rs` and `device-runtime/src/sim.rs` should only need localized updates to pass/receive burst metadata through existing event routing.
- Keep timeout/error reporting at user level equivalent (same error classes, clearer context strings for burst beat ranges).

### 5.3 Affected Rust Files
- `host-bus-handler/src/lib.rs`
- `host-bus-handler/src/tests.rs`
- `device-runtime/src/lib.rs`
- `device-runtime/src/fpga.rs`
- `device-runtime/src/sim.rs`
- `device-runtime/tests/test_memory.rs`
- `device-runtime/tests/test_rtl_peripherals.rs`

---

## 6. Migration Sequence

1. **Protocol constants/types first (Rust + RTL docs/comments)**
   - Define shared field semantics and max limits.
2. **Rust host-bus-handler implementation**
   - Add burst metadata parsing/serialization + tests.
3. **RTL RX/TX module updates**
   - Parse/serialize new framing with payload length checks.
4. **RTL orchestrator loop implementation (`host_bus_interface`)**
   - Add beat loop, address increment logic, response assembly.
5. **device-runtime low-level memory region migration**
   - Switch read/write region internals from per-access loops to burst calls.
6. **Integration + regression**
   - Re-enable existing tests and add burst-specific suites.
7. **Cleanup/documentation**
   - Remove obsolete single-transfer-only assumptions and comments.

---

## 7. Verification Strategy

### 7.1 RTL-Focused
- Packet decode tests:
  - valid min/max burst lengths
  - malformed length/payload mismatches
  - reserved-bit rejection
- Functional burst tests:
  - incrementing read/write bursts (1, 2, 4, large values)
  - `src_fixed` read FIFO mode
  - `dst_fixed` write FIFO mode
- Boundary tests:
  - 32-bit address overflow rejection on incrementing bursts.

### 7.2 Rust-Focused
- `host-bus-handler` unit tests for:
  - encode/decode round-trip of all packet classes
  - bidirectional interleaving with bursts
  - single transfer as burst length 1
- `device-runtime` tests:
  - ensure existing `read_memory_region` / `write_memory_region` behavior is preserved
  - verify large region transfers are chunked and complete correctly
  - FIFO fixed-address semantics checks where applicable.

### 7.3 End-to-End
- Sim runtime + FPGA runtime parity on burst read/write scenarios.
- Regression that existing user flows (program load, register access, LED tests) still pass without API changes.

---

## 8. Risks / Mitigations

1. **Risk: Large burst payload buffering pressure in RTL**
   - Mitigation: stream per beat where possible; limit in-flight bursts to one; use bounded payload staging.

2. **Risk: Protocol parser desynchronization on malformed streams**
   - Mitigation: strict length accounting + explicit drop-to-idle resync behavior.

3. **Risk: Address overflow in incrementing mode**
   - Mitigation: pre-check `base_addr + (burst_len-1)*stride` before first beat on both Rust and RTL sides.

4. **Risk: Regression of existing single access flows**
   - Mitigation: treat all legacy operations as `burst_len=1` and keep existing public APIs/tests unchanged.

5. **Risk: Bidirectional starvation/deadlock under long bursts**
   - Mitigation: preserve single-outstanding transaction rule and explicit arbitration priority rules already used by host bus interface.

---

## 9. Definition of Done

- Burst packet framing implemented and documented with fields in Section 2.
- Host bus supports up to 65,536 word beats per request.
- Single read/write transactions work as burst length 1.
- Non-incrementing source/destination modes function for FIFO-style patterns.
- No RTL system bus protocol/interface changes introduced.
- Bidirectional protocol operation remains functional.
- `device-runtime` public behavior remains compatible; bulk transfer performance path uses burst operations.
- New/updated tests cover protocol encode/decode, RTL beat looping, fixed/incrementing address modes, and regression of existing workflows.

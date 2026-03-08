# FPGA RTL Address/Data Channel Migration Plan

## 1. Overview and Goals

### 1.1 Problem Statement

The FPGA RTL still relies on a legacy unified bus contract:

- request: `addr`, `wdata`, `we`, `size`, `req`
- response: `rdata`, `ready`

The CPU no longer natively uses that interface. In `rtl/common/top.sv` the CPU already emits split address/data channels, but those channels are immediately converted back into the legacy bus by `rtl/common/io/bus_bridge.sv`, then routed through `rtl/common/io/host_bus_mux.sv`, `rtl/common/memory/bus_arbiter.sv`, and `rtl/common/memory/bus.sv`.

The target state is to remove that compatibility stack and make the FPGA-side interconnect natively use the split address/data channel interface everywhere.

### 1.2 Migration Goals

This migration must:

1. Convert all RTL peripherals to the split address/data channel interface.
2. Replace the legacy `bus.sv` fabric with `rtl/common/memory/registered_bus.sv`.
3. Configure `registered_bus` with exactly two masters:
   - **master 0** = `host_bus_interface` (highest priority)
   - **master 1** = CPU
4. Eliminate the need for `host_bus_mux.sv`.
5. Eliminate the need for `bus_bridge.sv`.
6. Leave no legacy unified-bus RTL in the final design.

### 1.3 Non-Goals

- Backward compatibility with the old FPGA bus interface is **not** required.
- Preserving legacy modules for optional fallback is **not** required.
- A transitional adapter layer may be used while implementation is in progress, but the merged result should delete all obsolete legacy-bus code.

---

## 2. Current Architecture Summary

### 2.1 Current Top-Level Data Flow

Today `rtl/common/top.sv` effectively routes memory traffic like this:

```text
CPU A/D interface
    │
    ▼
bus_bridge
    │  (legacy unified bus)
    ▼
host_bus_mux
 ┌──┴──────────────┐
 │                 │
 ▼                 ▼
CPU->RTL path      CPU->Host path
 │                 │
 ▼                 ▼
bus_arbiter        host_bus_interface slave port
 │
 ▼
bus
 ├─ system_controller
 ├─ led_controller_peripheral
 ├─ clock_peripheral
 └─ sram_peripheral
```

### 2.2 Legacy Modules That Must Disappear

The following modules exist only because the fabric still speaks the old interface:

- `rtl/common/io/bus_bridge.sv`
- `rtl/common/io/host_bus_mux.sv`
- `rtl/common/memory/bus_arbiter.sv`
- `rtl/common/memory/bus.sv`

Once the migration is complete, all four should be deleted and all top-level comments in `rtl/common/top.sv` updated to describe the new native A/D routing.

### 2.3 Modules That Must Be Converted

The following modules still expose the legacy unified bus ports and therefore require interface changes:

- `rtl/common/io/host_bus_interface.sv`
- `rtl/common/peripherals/led_controller_peripheral.sv`
- `rtl/common/peripherals/clock_peripheral.sv`
- `rtl/common/peripherals/system_controller_peripheral.sv`
- `rtl/common/peripherals/sram_peripheral.sv`

### 2.4 Existing New-Fabric Asset

`rtl/common/memory/registered_bus.sv` already implements the split-channel interconnect shape:

- master A: `*_mem_a_addr`, `*_mem_a_wdata`, `*_mem_a_we`, `*_mem_a_size`, `*_mem_a_valid`, `*_mem_a_ready`
- master D: `*_mem_d_rdata`, `*_mem_d_valid`, `*_mem_d_ready`
- matching slave A/D ports

It also already supports:

- multiple masters
- fixed-priority arbitration by master index
- one buffered request
- one buffered response
- one outstanding request per slave

That makes it the right foundation for the final FPGA interconnect.

### 2.5 Current Gaps in `registered_bus`

`registered_bus.sv` is not yet a drop-in replacement for the FPGA top-level because of one critical limitation:

1. **Address decode is still top-nibble based.**  
   The current implementation compares `pending_req_addr[31:28]` against `slave_base_addr[i][31:28]`, and `slave_addr_size` acts only as an enable.

This is sufficient for the current 0x2/0x5/0x6/0x7 RTL windows, but it is **not** sufficient to replace `host_bus_mux`, because the host-side address space spans multiple top nibbles (`0x8`, `0x9`, `0xA`, `0xB`, `0xC`, `0xF`).

---

## 3. Target Architecture

### 3.1 Final Top-Level Structure

The final architecture should look like this:

```text
                           ┌──────────────────────────────┐
                           │       registered_bus         │
                           │      NUM_MASTERS = 2         │
                           │      NUM_SLAVES  = 5         │
                           └──────────────────────────────┘

Masters:
  master 0 = host_bus_interface master port   (highest priority)
  master 1 = CPU native memory port

Slaves:
  slave 0 = system_controller
  slave 1 = led_controller_peripheral
  slave 2 = clock_peripheral
  slave 3 = sram_peripheral
  slave 4 = host_bus_interface slave port  (external/Rust-owned address space)
```

This keeps a single fabric for both:

- **CPU-originated requests** to RTL peripherals and external memory
- **Host-originated requests** to RTL peripherals

### 3.2 Priority Rule

The legacy arbiter gives host traffic priority over CPU traffic. The new fabric must preserve that behavior by wiring the masters as:

- `master_mem_*[0]` = `host_bus_interface`
- `master_mem_*[1]` = CPU

Because `registered_bus.sv` already grants the lowest-index requesting master first, this preserves the existing priority policy without extra arbitration RTL.

### 3.3 Required Address Map Entries

The final bus decode table should be expressed as real base/size ranges, not nibble aliases:

| Slave | Module | Base | Size |
|---|---|---:|---:|
| 0 | `system_controller` | `0x2000_0000` | `0x0000_0010` |
| 1 | `led_controller_peripheral` | `0x5000_0000` | `0x0000_0010` |
| 2 | `clock_peripheral` | `0x6000_0000` | `0x0000_0010` |
| 3 | `sram_peripheral` | `0x7000_0000` | `0x0000_3000` |
| 4 | `host_bus_interface` slave port | `0x8000_0000` | `0x8000_0000` |

The last entry represents the full upper half of the address space and is what makes `host_bus_mux` unnecessary.

### 3.4 Range-Decode Requirement

`registered_bus.sv` must be upgraded from nibble matching to true range matching.

The comparison should be written in a way that handles the `0x8000_0000` / `0x8000_0000` half-space range safely without 32-bit overflow. A robust implementation is:

```text
offset = {1'b0, addr} - {1'b0, base}
match  = (size != 0) && (offset < {1'b0, range_size})
```

This preserves:

- exact small windows for RTL peripherals
- a single wide host-facing window for all external/Rust-owned addresses
- the existing unmapped-address behavior for anything not covered by the configured ranges

### 3.5 Unified Slave Interface Contract

Every FPGA-visible slave should expose the same A/D interface:

```systemverilog
// Request
input  logic [31:0] mem_a_addr;
input  logic [31:0] mem_a_wdata;
input  logic        mem_a_we;
input  logic [1:0]  mem_a_size;
input  logic        mem_a_valid;
output logic        mem_a_ready;

// Response
output logic [31:0] mem_d_rdata;
output logic        mem_d_valid;
input  logic        mem_d_ready;
```

The final codebase should not contain mixed legacy/unified bus ports on top-level fabric-facing modules.

---

## 4. Module-by-Module Migration Plan

### 4.1 Phase 1: Upgrade `registered_bus` for Full-System Use

**Primary files**

- `rtl/common/memory/registered_bus.sv`
- `rtl/common/wrappers/registered_bus_wrapper.sv`
- `testbench/tests/registered_bus_test.rs`

**Required work**

1. Replace top-nibble decode with true base/size range matching.
2. Preserve fixed-priority arbitration by master index.
3. Preserve one-outstanding-per-slave behavior.
4. Preserve unmapped behavior:
   - writes are dropped
   - reads return `32'h0`
   - response still completes
5. Expand the wrapper/test collateral so the updated decode semantics are explicitly validated.

**Validation to add**

- address inside a small peripheral range matches correctly
- address just outside a small peripheral range is unmapped
- a host-space address like `0xF000_0000` routes to the host-interface slave when configured with the upper-half window
- master 0 still wins over master 1 when both request simultaneously

### 4.2 Phase 2: Convert Simple RTL Peripherals First

**Primary files**

- `rtl/common/peripherals/led_controller_peripheral.sv`
- `rtl/common/peripherals/clock_peripheral.sv`
- `rtl/common/peripherals/system_controller_peripheral.sv`

These three modules are the least risky conversion targets because their current behavior is logically simple and their response data is already available locally.

**Required work**

1. Replace legacy ports with A/D ports.
2. Capture an accepted A-channel request when `mem_a_valid && mem_a_ready`.
3. Return the response on D with stable `mem_d_rdata` and `mem_d_valid`.
4. Preserve current register semantics exactly:
   - LED byte-lane masking
   - clock registers remain read-only
   - system controller register map and pulse behavior remain unchanged

**Important behavioral note**

The current `registered_bus` implementation captures slave responses after a request has already become outstanding. That means these peripherals should not depend on same-cycle `req/ready` completion. Even for logically “single-cycle” peripherals, the A/D version should be implemented as:

1. accept request
2. compute or latch response payload
3. assert `mem_d_valid` until `mem_d_ready`

This keeps the peripherals aligned with the registered fabric instead of recreating legacy `ready` semantics internally.

### 4.3 Phase 3: Convert `sram_peripheral` Separately

**Primary file**

- `rtl/common/peripherals/sram_peripheral.sv`

`sram_peripheral.sv` is the most complex slave because it already contains:

- 1-cycle registered reads
- immediate writes
- split transactions for unaligned halfword/word accesses crossing word boundaries

**Required work**

1. Replace legacy ports with A/D ports.
2. Treat each accepted A request as an internally tracked transaction until a D response is issued.
3. Preserve all existing subword behavior, including split read/write handling.
4. Preserve write semantics for byte/halfword/word accesses.
5. Ensure the slave does not accept a second request while an earlier request is still active internally.

**Recommended structure**

- reuse the current internal split-operation state
- replace `req`/`ready` with:
  - `mem_a_ready = idle`
  - `mem_d_valid = operation_complete`
- latch request metadata on A handshake so later internal cycles do not depend on live A-channel inputs

### 4.4 Phase 4: Convert `host_bus_interface` Into a Native A/D Slave + Master

**Primary file**

- `rtl/common/io/host_bus_interface.sv`

This module needs two independent A/D roles after the migration:

1. **slave role** for CPU-originated accesses to external/Rust-owned address space
2. **master role** for host-originated accesses into FPGA-visible address space

**Required interface changes**

Replace the current legacy ports:

- slave side: `addr`, `wdata`, `rdata`, `we`, `size`, `req`, `ready`
- master side: `host_bus_addr`, `host_bus_wdata`, `host_bus_rdata`, `host_bus_we`, `host_bus_size`, `host_bus_req`, `host_bus_ready`

with A/D equivalents on both sides.

**Behavior to preserve**

- CPU-originated host requests remain single-outstanding.
- CPU requests are serialized onto TX packets exactly as today.
- CPU responses from RX produce one D-channel response back to the bus.
- Host-originated packet execution continues to support burst metadata and beat-by-beat address progression.

**Behavior to change**

- CPU requests should be accepted on an A handshake rather than on raw `req`.
- CPU response completion should be signaled with `mem_d_valid` / `mem_d_ready`, not `ready`.
- Host-initiated fabric accesses should be issued on the master A channel and completed from the master D channel.

**Key design constraint**

The converted module must avoid combinational dependency loops between:

- `registered_bus` master/slave ready signals
- TX packet readiness
- RX packet availability

All request/response ownership should remain register-backed, exactly as the current module already does for its internal state.

### 4.5 Phase 5: Cut Over `top.sv`

**Primary file**

- `rtl/common/top.sv`

**Required work**

1. Remove all legacy unified-bus internal signals.
2. Remove instantiation of:
   - `host_bus_mux`
   - `bus_arbiter`
   - `bus`
   - `bus_bridge`
3. Instantiate one `registered_bus` with:
   - `NUM_MASTERS = 2`
   - `NUM_SLAVES = 5`
4. Wire the masters in host-priority order.
5. Wire the five slaves listed in Section 3.3.
6. Define the slave base/size arrays directly in `top.sv`.
7. Update comments and section headers so `top.sv` describes the new native A/D fabric rather than the deleted compatibility stack.

**Expected end state**

The CPU should connect directly to `registered_bus`, and `host_bus_interface` should participate in the same fabric as both:

- a slave for upper-half external addresses
- a master for host-initiated accesses

That is the change that removes both the CPU adapter and the host-bus mux from the design.

### 4.6 Phase 6: Delete Obsolete Legacy Code

After `top.sv` is cut over and validation passes, delete:

- `rtl/common/io/bus_bridge.sv`
- `rtl/common/io/host_bus_mux.sv`
- `rtl/common/memory/bus_arbiter.sv`
- `rtl/common/memory/bus.sv`

Also remove any stale wrapper code, comments, or test-only references that mention the old unified bus as an active implementation path.

---

## 5. Recommended Implementation Order

The lowest-risk execution order is:

1. Upgrade and test `registered_bus`
2. Convert LED / clock / system controller
3. Convert SRAM
4. Convert `host_bus_interface`
5. Cut over `top.sv`
6. Delete legacy modules

This order keeps the highest-risk work (`host_bus_interface` and `top.sv` cutover) until after the bus primitive and slave-side contracts are already stable.

---

## 6. Verification Plan

### 6.1 Targeted RTL/Testbench Validation

The migration should be validated incrementally instead of waiting for one final integration pass.

**Recommended targeted checks**

1. `registered_bus` tests:
   - range decode
   - upper-half host window decode
   - fixed-priority arbitration
   - one-outstanding-per-slave behavior
2. peripheral-focused tests:
   - LED read/write behavior still matches current semantics
   - clock registers remain readable and read-only
   - system controller status and write-trigger pulses are preserved
   - SRAM aligned and unaligned read/write behavior is unchanged
3. `host_bus_interface` tests:
   - CPU-originated request/response flow over A/D slave port
   - host-originated read/write flow over A/D master port
   - host priority over CPU when both contend for the fabric
4. top-level integration tests:
   - CPU can still access RTL peripherals
   - CPU can still access external memory through `host_bus_interface`
   - host-originated requests can still reach RTL peripherals after cutover

### 6.2 Repository Validation Commands

Because this work changes SystemVerilog RTL, the final implementation should be validated with the project-standard commands:

```bash
find rtl/common -name '*.sv' -exec verilator --lint-only {} +
(cd rtl/fpga && make)
cargo clean
cargo test --verbose
```

If Rust verification code changes as part of the migration, also run the required Rust formatting and clippy steps described in `AGENTS.md`.

---

## 7. Risks and Design Notes

### 7.1 Decode Semantics Are the Critical Enabler

If `registered_bus` remains nibble-decoded, the migration will stall because the host-facing address space cannot be represented by one slave slot. Converting to true base/size range decode is the most important enabling step.

### 7.2 “Always Ready” Legacy Slaves Must Not Recreate Legacy Timing

The simple peripherals currently expose `ready = 1'b1`, but the registered fabric is response-based, not ready-based. Their new implementations should produce a D-channel response rather than trying to fake legacy same-cycle completion.

### 7.3 Priority Must Be Encoded by Wiring, Not Extra Control Logic

The old host-over-CPU priority should come from master index ordering in `registered_bus`, not from preserving a second arbiter layer. Adding another arbiter would duplicate logic and undermine the simplification goal.

### 7.4 No Legacy Interface Should Survive the Merge

If a temporary adapter is introduced to reduce bring-up risk, it should be treated as scaffolding only. The merged end state should expose a single memory-fabric contract across the FPGA RTL: split address/data channels everywhere.

---

## 8. Expected Final Cleanup Checklist

- [ ] `registered_bus.sv` performs true base/size range decode
- [ ] `host_bus_interface.sv` exposes A/D slave and A/D master ports
- [ ] all RTL peripherals expose A/D slave ports
- [ ] `top.sv` instantiates a single `registered_bus`
- [ ] host bus interface is wired as master 0
- [ ] CPU is wired as master 1
- [ ] host-facing slave window covers the external/Rust-owned address space
- [ ] `bus_bridge.sv` deleted
- [ ] `host_bus_mux.sv` deleted
- [ ] `bus_arbiter.sv` deleted
- [ ] `bus.sv` deleted
- [ ] stale legacy-bus comments/tests removed

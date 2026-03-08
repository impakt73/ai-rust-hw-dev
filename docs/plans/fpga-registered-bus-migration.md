# FPGA RTL Registered-Bus Migration Plan

## 1. Overview

This document describes the RTL migration from the legacy unified peripheral bus interface to the newer address/data channel interface already used by the CPU-facing memory path. The end-state goal is an FPGA RTL interconnect where:

- all RTL peripherals speak the `mem_a_*` / `mem_d_*` interface,
- the legacy `bus.sv` decoder is replaced by `registered_bus.sv`,
- the interconnect is configured with **two masters**:
  1. host-initiated target accesses from `host_bus_interface`
  2. CPU accesses to RTL peripherals from `host_bus_mux`
- host-initiated accesses retain **higher priority** than CPU accesses, and
- the existing `host_bus_mux` remains in the design so the host bus interface is not exposed back into the new bus fabric.

Backwards compatibility with the legacy unified bus interface is **not** a goal for this work. Once the migration is complete, the old interface should be unused and can be deleted in a follow-up cleanup change.

---

## 2. Current-State Inventory

### 2.1 Production RTL topology today

The current `rtl/common/top.sv` peripheral path is:

```text
CPU
  -> host_bus_mux
  -> bus_bridge
  -> bus_arbiter
  -> bus
  -> RTL peripherals
```

At the same time, CPU requests targeting upper-half addresses bypass the RTL peripheral path and go through `host_bus_interface` to the external host path.

Relevant production instances live in:

- `rtl/common/top.sv`
- `rtl/common/io/host_bus_mux.sv`
- `rtl/common/io/bus_bridge.sv`
- `rtl/common/memory/bus_arbiter.sv`
- `rtl/common/memory/bus.sv`
- `rtl/common/io/host_bus_interface.sv`

### 2.2 Legacy unified bus interface

The legacy RTL peripheral interface is the single request/response bus used by the existing decoder and peripherals:

- `addr[31:0]`
- `wdata[31:0]`
- `rdata[31:0]`
- `we`
- `size[1:0]`
- `req`
- `ready`

This interface is still used by:

- `rtl/common/memory/bus.sv`
- `rtl/common/memory/bus_arbiter.sv`
- `rtl/common/io/bus_bridge.sv` (legacy side)
- `rtl/common/io/host_bus_interface.sv` (host-initiated master side)
- `rtl/common/peripherals/led_controller_peripheral.sv`
- `rtl/common/peripherals/clock_peripheral.sv`
- `rtl/common/peripherals/sram_peripheral.sv`
- `rtl/common/peripherals/system_controller_peripheral.sv`

### 2.3 New address/data channel interface

The newer interface already used in the CPU memory path is split into:

#### Address channel
- `mem_a_addr[31:0]`
- `mem_a_wdata[31:0]`
- `mem_a_we`
- `mem_a_size[1:0]`
- `mem_a_valid`
- `mem_a_ready`

#### Data channel
- `mem_d_rdata[31:0]`
- `mem_d_valid`
- `mem_d_ready`

#### Completion contract

The migrated system will use the following architectural rule for all address/data channel transactions:

- **every accepted memory request must complete on the D channel, regardless of whether it is a read or a write,**
- software and RTL requesters must treat `mem_d_valid && mem_d_ready` as the acknowledgement that the operation has completed, and
- for **writes**, `mem_d_rdata` is **unspecified** and must not be used.

This interface already exists in production or near-production RTL in:

- `rtl/common/io/host_bus_mux.sv`
- `rtl/common/io/host_bus_interface.sv` (CPU-facing slave side)
- `rtl/common/io/bus_bridge.sv` (A/D side)
- `rtl/common/memory/registered_bus.sv`

### 2.4 Current priority behavior

The legacy `bus_arbiter.sv` gives **host-initiated requests priority over CPU requests**. That ordering must be preserved in the new design.

### 2.5 Current decode behavior

The current `bus.sv` decoder and `registered_bus.sv` both decode slaves by top nibble (`addr[31:28]`), which matches the memory-map documentation for RTL peripheral windows:

- `0x2...` system controller
- `0x5...` LED controller
- `0x6...` clock peripheral
- `0x7...` SRAM peripheral

The migration should preserve this decode model unless a separate design change intentionally tightens address-range enforcement.

---

## 3. Migration Goals

### 3.1 Functional goals

1. Convert all synthesizable RTL peripherals to the address/data channel interface.
2. Convert the host-initiated master side of `host_bus_interface` to the same interface.
3. Replace the `bus_bridge + bus_arbiter + bus` chain with a single `registered_bus` instance.
4. Configure the new interconnect with exactly two masters:
   - master 0: host-initiated target accesses
   - master 1: CPU-originated accesses to the RTL peripheral half-space
5. Preserve host priority by master ordering in `registered_bus`.
6. Keep `host_bus_mux` in front of the CPU path.
7. Require all reads and writes to wait for D-channel completion, with write-response data treated as unspecified.

### 3.2 Non-goals

The following are explicitly out of scope for this change:

- maintaining legacy bus compatibility,
- supporting mixed old/new peripheral interface variants in the long term,
- redesigning the top-nibble peripheral address map,
- changing the CPU-to-host external memory path for upper-half addresses.

---

## 4. Target Architecture

The target production topology in `rtl/common/top.sv` should be:

```text
CPU
  -> host_bus_mux
      -> RTL-peripheral A/D path -------------------+
                                                    |
Host bus interface host-initiated A/D path -------->| registered_bus
                                                    |
                                                    +-> system_controller_peripheral
                                                    +-> led_controller_peripheral
                                                    +-> clock_peripheral
                                                    +-> sram_peripheral

CPU upper-half accesses
  -> host_bus_mux
  -> host_bus_interface CPU-facing slave path
  -> external host / Rust peripherals / DRAM
```

### 4.1 Why `host_bus_mux` must remain

`host_bus_mux` must stay in the design for two reasons:

1. **Loop prevention**  
   It prevents CPU requests for upper-half addresses from entering the RTL interconnect and prevents the host bus interface from being exposed as a slave behind the new registered bus, which could create accidental communication loops.

2. **Simple peripheral decode**  
   It preserves the clean split between:
   - lower-half RTL peripheral traffic handled locally in FPGA RTL, and
   - upper-half Rust/DRAM traffic handled through the host bus interface.

### 4.2 Registered-bus master ordering

`registered_bus.sv` uses fixed-priority arbitration where the lowest master index wins. The new top-level wiring must therefore use:

- **Master 0** = host-initiated target access channel from `host_bus_interface`
- **Master 1** = CPU RTL-peripheral channel from `host_bus_mux`

This preserves the legacy `Host > CPU` arbitration rule.

---

## 5. Required RTL Changes by Module

### 5.1 `rtl/common/peripherals/led_controller_peripheral.sv`

Convert the peripheral from the legacy unified bus to address/data channels.

Planned changes:

- replace `addr/wdata/rdata/we/size/req/ready` with `mem_a_*` / `mem_d_*`,
- accept one request at a time using the standard valid/ready handshake,
- return the current LED register value on reads through the D channel,
- complete writes only when the D channel acknowledges the request, with unspecified `mem_d_rdata` for writes,
- preserve current byte/halfword/word behavior and reserved-bit masking.

Expected behavior:

- single-cycle request acceptance,
- single-cycle completion response,
- no functional change to the LED register semantics.

### 5.2 `rtl/common/peripherals/clock_peripheral.sv`

Convert the clock peripheral to address/data channels.

Planned changes:

- replace the legacy request/ready interface with `mem_a_*` / `mem_d_*`,
- preserve read-only semantics,
- return elapsed counter values through the D channel,
- preserve current invalid-write handling behavior as closely as possible while still issuing a D-channel completion acknowledgement.

Expected behavior:

- single-cycle request acceptance,
- single-cycle response for reads.

### 5.3 `rtl/common/peripherals/system_controller_peripheral.sv`

Convert the system controller to address/data channels.

Planned changes:

- migrate all register accesses to A/D handshakes,
- preserve reset, boot, halt, and status behavior,
- ensure writes that trigger control effects still do so exactly once per request that later completes on the D channel.

Expected behavior:

- single-cycle request acceptance for control writes,
- read responses through the D channel without changing architectural behavior.

### 5.4 `rtl/common/peripherals/sram_peripheral.sv`

Convert the SRAM peripheral to address/data channels.

This is the most timing-sensitive peripheral in the migration because it already has a registered read path.

Planned changes:

- replace the unified bus interface with `mem_a_*` / `mem_d_*`,
- preserve byte/halfword/word writes,
- preserve the existing on-chip SRAM address window and backing memory,
- explicitly document and implement request completion latency under the new protocol for both reads and writes.

Expected behavior:

- writes should still complete through the D channel without changing visible SRAM contents semantics,
- reads should retain deterministic latency even if the new interconnect adds an additional registered stage,
- the implementation should avoid creating combinational loops between `mem_a_ready` and `mem_d_valid`.

### 5.5 `rtl/common/io/host_bus_interface.sv`

The CPU-facing slave side is already on the new interface and should remain structurally unchanged. The host-initiated target-access master side still uses the legacy unified bus and must be converted.

Planned changes:

- replace `host_bus_addr/host_bus_wdata/host_bus_rdata/host_bus_we/host_bus_size/host_bus_req/host_bus_ready`
  with a host-master A/D channel pair,
- keep packet parsing and serialization responsibilities unchanged,
- issue host-initiated target requests into `registered_bus` using the new interface,
- consume D-channel completions and serialize them back to the host exactly once, treating write-response data as unspecified.

Important constraint:

- host-initiated requests must target only the lower-half RTL-owned address space in the migrated topology; upper-half addresses must not be routed back into the host path because that would violate the loop-prevention rule described in [Section 4.1](#41-why-host_bus_mux-must-remain).

### 5.6 `rtl/common/top.sv`

This file is the main structural integration point.

Planned changes:

1. remove the `bus_bridge` instance,
2. remove the `bus_arbiter` instance,
3. remove the `bus` instance,
4. instantiate `registered_bus` with:
   - `NUM_MASTERS = 2`
   - `NUM_SLAVES = 4`
5. wire the two masters in priority order:
   - master 0 = host-initiated path from `host_bus_interface`
   - master 1 = CPU RTL path from `host_bus_mux`
6. wire one slave slot per RTL peripheral window:
   - slave 0 = system controller (`0x2...`)
   - slave 1 = LED (`0x5...`)
   - slave 2 = clock (`0x6...`)
   - slave 3 = SRAM (`0x7...`)
7. keep the CPU upper-half path from `host_bus_mux` to `host_bus_interface` intact.

### 5.7 `rtl/common/memory/registered_bus.sv`

The intent is to **reuse** this module rather than redesign it. The plan should treat the current behavior as the implementation baseline:

- fixed-priority arbitration by master index,
- top-nibble slave decode,
- registered request/response tracking,
- unmapped requests return zero data.

Any RTL changes to this file should be limited to issues discovered while integrating the two-master, four-slave production topology.

---

## 6. Phased Implementation Strategy

### Phase 1: Define the production interface contract

Before rewiring the top-level, standardize how each migrated peripheral will use the A/D protocol:

- when a request is considered accepted,
- how all requests wait for D-channel acknowledgement before they are considered complete,
- how writes return an acknowledgement with unspecified `mem_d_rdata`,
- whether reads are immediate or registered,
- how unmapped accesses behave, and
- how invalid accesses behave.

Deliverable:

- one consistent response model used by all FPGA RTL peripherals, where every request completes on the D channel.

### Phase 2: Convert individual peripherals

Migrate peripherals one at a time in this order:

1. `led_controller_peripheral.sv`
2. `clock_peripheral.sv`
3. `system_controller_peripheral.sv`
4. `sram_peripheral.sv`

Rationale:

- start with the simplest stateless or low-state peripherals,
- defer SRAM until the A/D response contract is stable.

### Phase 3: Convert host-initiated master path

Update `host_bus_interface.sv` so host-initiated target accesses use A/D channels instead of the legacy unified bus.

Deliverable:

- a host-initiated master interface that can plug directly into `registered_bus`.

### Phase 4: Replace the production interconnect in `top.sv`

After both masters and all slaves are A/D-native:

- remove the bridge/arbiter/bus chain from the top-level,
- instantiate and wire `registered_bus`,
- confirm the lower-half/upper-half split is still enforced by `host_bus_mux`.

### Phase 5: Verification and cleanup preparation

Once the new topology is stable:

- verify that no production path still depends on the legacy unified bus,
- identify old-interface modules that are now dead code,
- defer deletion to a dedicated cleanup change.

---

## 7. Verification Plan

### 7.1 RTL validation

After RTL implementation, validate:

- SystemVerilog lint for the changed RTL modules,
- FPGA synthesis for the supported FPGA targets affected by the change,
- host-bus request/response behavior,
- CPU access to all lower-half RTL peripherals,
- host-initiated access to all migrated RTL peripherals.

### 7.2 Behavior that must be explicitly checked

1. **Priority preservation**  
   Simultaneous CPU and host requests must continue to prefer host access.

2. **Loop avoidance**  
   Host-initiated requests must not be able to route back into the host/external side.

3. **Decode preservation**  
   Existing lower-half peripheral windows must still map by the same top-nibble decode scheme.

4. **SRAM latency**  
   Read timing must remain deterministic and compatible with existing expectations.

5. **CPU external path integrity**  
   CPU accesses to upper-half addresses must still go only through `host_bus_interface`, not through `registered_bus`.

### 7.3 Recommended test coverage updates

Targeted tests for the migration should cover:

- CPU read/write access for LED, clock, SRAM, and system controller via the new bus fabric,
- host-initiated read/write access for the same peripherals,
- simultaneous host and CPU contention on the registered bus,
- unmapped lower-half address behavior,
- regression coverage for `registered_bus` arbitration and response routing.

---

## 8. Legacy Code Expected to Become Obsolete

After the migration is complete and verified, the following modules should be unused and candidates for deletion in a follow-up change:

- `rtl/common/io/bus_bridge.sv`
- `rtl/common/memory/bus.sv`
- `rtl/common/memory/bus_arbiter.sv`

Any top-level legacy bus signal declarations that existed only to connect those modules should also be removed in that later cleanup pass.

---

## 9. Risks and Open Questions

### 9.1 Peripheral response contract enforcement

The D-channel completion contract is now fixed for the migration: every request, including writes, must wait for D-channel acknowledgement before it is considered complete. For write requests, `mem_d_rdata` is unspecified and must not be consumed. The implementation risk is no longer deciding the contract, but applying it consistently across all peripherals and the host-initiated path.

### 9.2 SRAM response timing

`sram_peripheral.sv` already has non-zero read latency. The migration must ensure that the combination of SRAM timing and `registered_bus` staging does not create an off-by-one-cycle protocol mismatch.

### 9.3 Decode granularity

`registered_bus.sv` currently treats `slave_addr_size` as an enable and still matches only on the top nibble. This is acceptable for the migration because it matches the existing decoder behavior, but it should be documented clearly so future work does not assume range-exact decode.

---

## 10. Completion Criteria

This migration is complete when all of the following are true:

- every production RTL peripheral uses the A/D channel interface,
- `host_bus_interface` exposes an A/D master interface for host-initiated target accesses,
- `rtl/common/top.sv` uses `registered_bus` instead of `bus_bridge + bus_arbiter + bus`,
- the registered bus is configured with two masters and host priority is preserved,
- `host_bus_mux` remains in the production topology,
- no production path relies on the legacy unified peripheral bus interface anymore.

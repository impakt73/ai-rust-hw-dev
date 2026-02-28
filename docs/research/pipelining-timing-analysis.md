# RTL Pipelining & Timing Analysis

**Date:** 2026-02-28
**Scope:** All RTL modules in `rtl/common/`
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)
**Current Fmax:** ~34.91 MHz (25 MHz target, 40% margin)
**Current Logic Utilization:** 61% (4,688 / 7,680 LCs)

---

## 1. Architecture Overview

The design is a **multi-cycle non-pipelined RISC-V RV32IMACF CPU** with a 12-state FSM. The signal flow through the design follows this hierarchy:

```
fpga_top.sv
  └── top.sv (CPU + RTL peripherals)
        ├── reset_controller
        ├── cpu.sv (12-state FSM core)
        │     ├── fetch_buffer.sv (RV32C instruction buffering)
        │     ├── decompress.sv (16-bit → 32-bit expansion, combinational)
        │     ├── decoder.sv (instruction decode, combinational)
        │     ├── regfile.sv (dual-banked BRAM, 1-cycle latency)
        │     ├── branch_unit.sv (branch decision, combinational)
        │     ├── alu.sv (arithmetic/logic, includes mul/div units)
        │     │     ├── mul_unit.sv (shift-and-add, 32+ cycles)
        │     │     └── div_unit.sv (non-restoring, 32+ cycles)
        │     ├── fpu.sv (floating point, multi-cycle)
        │     ├── fp_regfile.sv (FP register file, BRAM)
        │     ├── csr_file.sv (CSR registers, BRAM-backed)
        │     ├── mem_interface.sv (memory sizing/formatting, combinational)
        │     └── writeback_mux.sv (result selection, combinational)
        ├── host_bus_mux.sv (address-based routing, combinational)
        ├── bus_arbiter.sv (CPU/Host arbitration, 3-state FSM)
        ├── bus.sv (peripheral address decode, combinational)
        ├── host_bus_interface.sv (byte serialization, FSM)
        │     ├── host_bus_rx.sv (RX parser, 9-state FSM)
        │     └── host_bus_tx.sv (TX serializer, 14-state FSM)
        ├── led_controller_peripheral.sv (single-cycle)
        ├── clock_peripheral.sv (single-cycle, cascaded counters)
        ├── sram_peripheral.sv (1-cycle read latency, byte masking)
        ├── system_controller_peripheral.sv (single-cycle)
        └── sys_led_controller.sv (activity indicators)
```

---

## 2. Critical Timing Paths (Ranked by Severity)

Based on the synthesis analysis and RTL inspection, these are the longest combinational paths in the design:

### Path 1: ALU Result → Bus → Peripheral (Critical Path)
**Delay:** ~26-29 ns | **Severity:** HIGH

```
cpu.alu_op_reg → alu_b mux → ALU computation (32-bit carry chain) →
  alu_result → mem_interface → host_bus_mux (combinational) →
  bus.sv (combinational address decode) → peripheral
```

The critical path starts at the ALU input mux, passes through the 32-bit carry chain (~4-5 ns), then traverses two purely combinational routing modules (`host_bus_mux` and `bus.sv`) before reaching peripheral logic. The lack of pipeline registers between the CPU, mux, and bus means the entire data path must settle in a single cycle.

### Path 2: Memory Read Data → Writeback Mux → Register File
**Delay:** ~15-20 ns | **Severity:** MEDIUM-HIGH

```
mem_rdata → mem_interface.formatted_load_data (sign extension mux) →
  mdr register → writeback_mux (8-input mux) → rd_data → regfile write
```

The writeback mux (`writeback_mux.sv`) is an 8-way priority multiplexer that selects between ALU result, memory data, CSR data, FPU result, LUI immediate, AUIPC computation, jump return address, and SC result. This wide fan-in creates a deep LUT cascade.

### Path 3: Decoder → FSM Next State Logic
**Delay:** ~12-15 ns | **Severity:** MEDIUM

```
ir_reg → decoder (large combinational case statement) →
  opcode/control signals → next_state logic (12-state case)
```

The decoder is a large purely combinational block that decodes all RV32IMACF opcodes. Its outputs feed directly into the FSM next-state logic, creating a long combinational chain. However, this is partially mitigated by the S_DECODE → S_REG_READ transition that registers decoder outputs.

### Path 4: Host Bus RX/TX Byte Stream Processing
**Delay:** ~10-12 ns | **Severity:** MEDIUM

```
rx_data → host_bus_rx (byte assembly + state machine) →
  packet fields → host_bus_interface (arbitration logic) →
  tx arbitration → host_bus_tx (serialization)
```

The host bus interface chain has multiple FSMs communicating via handshake signals. While each individual FSM is well-structured, the combinational paths through the TX arbitration logic in `host_bus_interface.sv` (lines 196-253) involve wide muxes that select between CPU requests and host responses.

### Path 5: Address Decode Chain (host_bus_mux → bus → peripheral)
**Delay:** ~8-10 ns | **Severity:** MEDIUM-LOW

```
cpu_mem_addr → host_bus_mux (32-bit comparison) →
  bus.sv (4-way address decode with 32-bit comparisons) →
  peripheral select → response mux
```

Both `host_bus_mux.sv` and `bus.sv` are purely combinational. The address comparisons (32-bit `>=` and `<`) feed into peripheral select signals, which then mux the response data. This creates a multi-level combinational path.

---

## 3. Pipelining Opportunities

### Opportunity A: Skid Buffer on CPU → External Memory Path
**Difficulty:** ★☆☆☆☆ (Easy) | **Impact:** Medium | **Category:** Near-Term

**Location:** Between `host_bus_mux` output and `host_bus_interface` input

**Current situation:** The CPU's external memory requests flow through `host_bus_mux` (combinational) directly into `host_bus_interface`. The CPU address, write data, write enable, and size signals travel through the mux with no register boundary.

**Proposed change:** Insert a `skid_buffer` (parameterized to carry the full bus request: addr + wdata + we + size = 32+32+1+2 = 67 bits) between the host path output of `host_bus_mux` and the slave input of `host_bus_interface`. This breaks the timing path without adding pipeline bubbles thanks to the skid buffer's 2-entry design.

```
BEFORE: cpu_mem → host_bus_mux (comb) → host_bus_interface
AFTER:  cpu_mem → host_bus_mux (comb) → skid_buffer → host_bus_interface
```

**Why skid buffer:** The existing `skid_buffer.sv` primitive is a full 2-entry buffer that registers both the forward path (valid/data) and the back-pressure path (ready). It breaks combinational paths in both directions without reducing throughput. The CPU's multi-cycle FSM already waits for `mem_ready`, so a 1-cycle latency increase is easily absorbed.

**Estimated Fmax improvement:** 2-4 MHz (breaks the longest combinational segment)

---

### Opportunity B: Skid Buffer on Bus Arbiter → System Bus Path
**Difficulty:** ★☆☆☆☆ (Easy) | **Impact:** Low-Medium | **Category:** Near-Term

**Location:** Between `bus_arbiter` output and `bus.sv` input

**Current situation:** The arbiter output signals (`arb_bus_addr`, `arb_bus_wdata`, etc.) are combinational muxes of CPU or Host master signals. These feed directly into `bus.sv`, which is another combinational address decoder. The two consecutive combinational stages create an unnecessarily long path.

**Proposed change:** Insert a `skid_buffer` (67-bit: addr+wdata+we+size) between the arbiter's slave-side output and the bus module input. The arbiter already has registered state, but its output mux (lines 107-143 of `bus_arbiter.sv`) is combinational.

```
BEFORE: bus_arbiter (comb mux) → bus.sv (comb decode) → peripheral
AFTER:  bus_arbiter (comb mux) → skid_buffer → bus.sv (comb decode) → peripheral
```

**Estimated Fmax improvement:** 1-3 MHz

---

### Opportunity C: Register the Writeback Mux Output
**Difficulty:** ★★☆☆☆ (Easy-Medium) | **Impact:** Medium | **Category:** Near-Term

**Location:** Output of `writeback_mux.sv` before register file write

**Current situation:** The writeback mux is an 8-way priority multiplexer with inputs from ALU, memory, CSR, FPU, and immediate values. Its output `rd_data` feeds directly into the register file write port. This is one of the widest fan-in muxes in the design.

**Proposed change:** The CPU's S_WRITEBACK state already provides a dedicated cycle for writeback. The mux output could be registered in S_EXECUTE or S_MEM_READ (depending on the instruction type) so that S_WRITEBACK simply uses a pre-computed registered value. This is partially implemented already (e.g., `alu_out_reg`, `mdr`, `fpu_out_reg`), but the final mux selection in `writeback_mux.sv` is still combinational during S_WRITEBACK.

**Approach:** Move the writeback mux selection earlier — compute `rd_data` during the state preceding S_WRITEBACK and register it. S_WRITEBACK would then simply use the registered value.

**Estimated Fmax improvement:** 2-3 MHz

---

### Opportunity D: Register ALU Output Before Memory Interface
**Difficulty:** ★★☆☆☆ (Easy-Medium) | **Impact:** Medium | **Category:** Near-Term

**Location:** Between ALU result and memory address path

**Current situation:** The ALU result is registered in `alu_out_reg` during S_EXECUTE and S_MEM_ADDR. However, `mem_interface.sv` uses `alu_out_reg` combinationally to drive `dmem_addr`, which then feeds through `host_bus_mux` (combinational) to either the bus or host interface. The path from `alu_out_reg` through two combinational modules is a significant contributor to the critical path.

**Note:** This is already partially addressed by the existing `alu_out_reg`, but the downstream combinational stages (`mem_interface` → `host_bus_mux` → `bus.sv`) amplify the path. Opportunity A (skid buffer on external path) would address the external portion; this opportunity addresses the internal RTL peripheral path.

---

### Opportunity E: Pipeline the Decoder
**Difficulty:** ★★★☆☆ (Medium) | **Impact:** Medium | **Category:** Medium-Term

**Location:** `decoder.sv` output stage

**Current situation:** The decoder is purely combinational and produces ~20+ control signals from a single 32-bit instruction input. Currently, its outputs are registered in `decode_reg_write` during S_DECODE. However, the decoder feeds from `ir_reg` (which is written in S_FETCH when `imem_ready` fires), and the FSM transitions to S_DECODE on the next cycle, where all outputs are captured. This is actually well-pipelined already.

**Potential improvement:** If the decoder becomes a bottleneck (e.g., when adding more extensions), split it into a 2-stage pipeline: stage 1 decodes the opcode and basic fields, stage 2 decodes the detailed control signals. This would require adding a new FSM state or restructuring S_DECODE.

---

### Opportunity F: Skid Buffer on Host Bus TX Path
**Difficulty:** ★★☆☆☆ (Easy-Medium) | **Impact:** Low-Medium | **Category:** Near-Term

**Location:** Between `host_bus_interface` TX packet logic and `host_bus_tx` module

**Current situation:** The TX arbitration in `host_bus_interface.sv` (lines 196-253) is a combinational mux that selects between host write responses, host read data, and CPU requests. This mux output drives `host_bus_tx` directly.

**Proposed change:** The TX packet signals are already structured as a valid/ready handshake (`tx_pkt_valid`, `tx_pkt_ready`). A `skid_buffer` can be inserted on this interface to break the combinational path from the TX arbitration mux to the TX serializer.

**Width:** The TX beat stream carries: valid + start + last + req + we + size + src_fixed + dst_fixed + burst_len_m1 + base_addr + data = 1+1+1+1+1+2+1+1+16+32+32 = 89 bits

---

### Opportunity G: Pipeline the Branch Decision
**Difficulty:** ★★★☆☆ (Medium) | **Impact:** Low | **Category:** Long-Term

**Location:** `branch_unit.sv`

**Current situation:** The branch unit performs 32-bit comparisons (equality, signed/unsigned less-than, signed/unsigned greater-or-equal) on registered operands. The branch result feeds into the PC update logic. This path was already optimized by pre-computing branch targets during S_DECODE.

**Observation:** This is already well-optimized. The branch unit operates on registered `a_reg` and `b_reg` values, and the branch target is pre-computed. Further pipelining here would provide diminishing returns.

---

### Opportunity H: Full Pipeline Architecture (IF → ID → EX → MEM → WB)
**Difficulty:** ★★★★★ (Very Hard) | **Impact:** Very High | **Category:** Long-Term

**Description:** Convert the multi-cycle FSM to a classic 5-stage pipeline. This is a major architectural change documented in `docs/research/pipelined-cpu-design-approaches.md`.

**Key challenges:**
- Data hazard detection and forwarding network
- Control hazard handling (branch prediction)
- Memory hazards (load-use stalls)
- Multi-cycle operations (mul/div/FPU) require pipeline stalling or out-of-order completion
- Atomic operations need pipeline drain/lock mechanism
- Compressed instruction handling across pipeline stages
- Separate instruction and data memory ports required

**This is explicitly a long-term goal** and should not be attempted until the near-term improvements are validated.

---

## 4. Summary: Ranked Recommendations

### Near-Term Improvements (1-2 days each, no architectural changes)

| Priority | Opportunity | Difficulty | Impact | Description |
|----------|------------|------------|--------|-------------|
| **1** | **A** | ★☆☆☆☆ | Medium | Skid buffer on CPU → external memory path (host_bus_mux → host_bus_interface) |
| **2** | **B** | ★☆☆☆☆ | Low-Med | Skid buffer on arbiter → system bus path |
| **3** | **F** | ★★☆☆☆ | Low-Med | Skid buffer on host_bus_interface TX → host_bus_tx path |
| **4** | **C** | ★★☆☆☆ | Medium | Register writeback mux output earlier in pipeline |

### Medium-Term Improvements (1-2 weeks each, localized changes)

| Priority | Opportunity | Difficulty | Impact | Description |
|----------|------------|------------|--------|-------------|
| **5** | **D** | ★★☆☆☆ | Medium | Register ALU output before memory interface chain |
| **6** | **E** | ★★★☆☆ | Medium | Pipeline the decoder (split into 2 stages if needed) |

### Long-Term Improvements (months, major architectural work)

| Priority | Opportunity | Difficulty | Impact | Description |
|----------|------------|------------|--------|-------------|
| **7** | **G** | ★★★☆☆ | Low | Further optimize branch decision path |
| **8** | **H** | ★★★★★ | Very High | Full 5-stage pipeline conversion |

---

## 5. Detailed Skid Buffer Insertion Points

The existing `skid_buffer.sv` primitive is ideal for these insertions because:
1. It breaks timing in **both** forward (valid/data) and backward (ready) directions
2. It maintains **full throughput** (no bubble insertion under steady-state flow)
3. It has **2 entries** of buffering, absorbing back-pressure glitches
4. It is already **verified** in the design (used in `skid_buffer_wrapper.sv` test wrapper)

### Insertion Point 1: CPU External Memory Path

**File to modify:** `top.sv` (lines 193-223)

**Signal bundle to buffer:**
| Signal | Width | Direction |
|--------|-------|-----------|
| addr | 32 | Forward |
| wdata | 32 | Forward |
| we | 1 | Forward |
| size | 2 | Forward |
| **Total forward** | **67** | |
| rdata | 32 | Backward (separate skid buffer or direct) |
| ready | 1 | Backward (built into skid buffer) |

**Implementation approach:**
Combine `{addr, wdata, we, size}` into a 67-bit data bundle for the skid buffer. The forward path carries the request; the `ready` signal provides back-pressure. The response path (`rdata`) would need a separate return path skid buffer or could remain combinational since `host_bus_interface` already registers its response.

### Insertion Point 2: Arbiter to System Bus

**File to modify:** `top.sv` (lines 260-317)

**Signal bundle:** Same 67-bit `{addr, wdata, we, size}` bundle. The arbiter output is already a mux; adding a skid buffer after it breaks the path to the address decoder in `bus.sv`.

### Insertion Point 3: TX Packet Interface

**File to modify:** `host_bus_interface.sv` (between TX logic and `host_bus_tx` instantiation)

**Signal bundle:** 89-bit TX beat stream (see Opportunity F above). The interface already uses valid/ready handshaking, making skid buffer insertion straightforward.

---

## 6. Module-by-Module Analysis

### CPU Core (`cpu.sv`) — 1,320 lines
**Role:** Central 12-state FSM controlling all instruction execution
**Timing characteristics:**
- Well-structured with registered pipeline stages between FSM states
- Pre-computed branch/jump targets (optimization already applied)
- BRAM register file with dedicated S_REG_READ state for latency
- Multi-cycle ALU/FPU operations properly handled with start/ready handshake
**Pipelining notes:** The FSM is the natural candidate for eventual 5-stage pipeline conversion (Opportunity H). For now, the registered staging approach works well.

### ALU (`alu.sv`) — 236 lines
**Role:** Arithmetic/logic unit with M-extension mul/div support
**Timing characteristics:**
- Single-cycle operations: ADD, SUB, AND, OR, XOR, shifts, comparisons
- Multi-cycle: multiplication (32 cycles), division (34 cycles)
- The 32-bit carry chain for ADD/SUB is ~4-5 ns on iCE40
- MIN/MAX comparisons add additional delay for A-extension atomics
**Pipelining notes:** The carry chain is the single largest timing contributor. A carry-select or carry-lookahead adder would help but adds complexity. The multi-cycle mul/div units are already properly pipelined internally.

### Decoder (`decoder.sv`) — 538 lines
**Role:** Instruction decode for all RV32IMACF opcodes
**Timing characteristics:** Pure combinational, ~20+ output signals. Large case statement with nested if/case for M and F extensions. Outputs are registered in S_DECODE.
**Pipelining notes:** Already well-handled by the DECODE → REG_READ state transition.

### Decompressor (`decompress.sv`) — 431 lines
**Role:** RV32C 16-bit to 32-bit instruction expansion
**Timing characteristics:** Pure combinational with three quadrant decode tasks. Output feeds into `ir_reg` during S_FETCH.
**Pipelining notes:** Could be pipelined if it becomes a bottleneck, but currently well within timing.

### Fetch Buffer (`fetch_buffer.sv`) — 156 lines
**Role:** Manages half-word buffering for compressed instructions
**Timing characteristics:** Simple registered buffer with combinational assembly logic. Well-structured.
**Pipelining notes:** No timing concerns.

### Register File (`regfile.sv`) — 71 lines
**Role:** Dual-banked BRAM providing 2-read, 1-write
**Timing characteristics:** 1-cycle read latency, handled by S_REG_READ state. Uses 4 BRAM blocks.
**Pipelining notes:** Excellent use of BRAM. No changes needed.

### Branch Unit (`branch_unit.sv`) — 34 lines
**Role:** Branch condition evaluation
**Timing characteristics:** Pure combinational, operates on registered operands. 32-bit comparisons.
**Pipelining notes:** Already optimized to avoid ALU dependency.

### Memory Interface (`mem_interface.sv`) — 79 lines
**Role:** Memory operation sizing and load data formatting
**Timing characteristics:** Pure combinational. Generates write data alignment and load sign-extension.
**Pipelining notes:** Short paths, no immediate concern.

### Writeback Mux (`writeback_mux.sv`) — 62 lines
**Role:** 8-way result source selection
**Timing characteristics:** Pure combinational priority mux. Wide fan-in creates deep LUT cascade.
**Pipelining notes:** Candidate for Opportunity C (register output earlier).

### Host Bus Mux (`host_bus_mux.sv`) — 56 lines
**Role:** Routes CPU memory requests to RTL peripherals or external host
**Timing characteristics:** Pure combinational 32-bit address comparison and 2:1 data mux.
**Pipelining notes:** Part of the critical path. Candidate for Opportunity A (skid buffer insertion).

### Bus Arbiter (`bus_arbiter.sv`) — 146 lines
**Role:** Fixed-priority arbitration between CPU and Host masters
**Timing characteristics:** 3-state FSM with registered state. Output mux is combinational.
**Pipelining notes:** Candidate for Opportunity B (skid buffer on output).

### System Bus (`bus.sv`) — 182 lines
**Role:** Address decode and peripheral routing
**Timing characteristics:** Pure combinational. Four 32-bit range comparisons plus 4:1 response mux.
**Pipelining notes:** Part of the critical path chain. Helped by Opportunity B.

### Host Bus Interface (`host_bus_interface.sv`) — 416 lines
**Role:** Routes host bus transactions between system bus, RX parser, and TX serializer
**Timing characteristics:** 6-state FSM with registered state. TX arbitration mux is combinational.
**Pipelining notes:** Candidate for Opportunity F (skid buffer on TX path).

### Host Bus RX (`host_bus_rx.sv`) — 244 lines
**Role:** Parses 8-byte metadata framing from RX byte stream
**Timing characteristics:** 9-state FSM, well-pipelined. Single-beat output buffering.
**Pipelining notes:** Already well-structured with registered output.

### Host Bus TX (`host_bus_tx.sv`) — 263 lines
**Role:** Serializes metadata framing and payload to TX byte stream
**Timing characteristics:** 14-state FSM with registered state. Input acceptance via handshake.
**Pipelining notes:** Well-pipelined internally.

### SRAM Peripheral (`sram_peripheral.sv`) — 301 lines
**Role:** 12KB memory-mapped SRAM with byte write masking
**Timing characteristics:** 1-cycle read latency (BRAM). Unaligned access handling adds states.
**Pipelining notes:** Well-designed with proper latency handling.

### LED Controller (`led_controller_peripheral.sv`) — 74 lines
**Role:** Simple 8-bit LED output register
**Timing characteristics:** Single-cycle, always ready. Minimal logic.
**Pipelining notes:** No concerns.

### Clock Peripheral (`clock_peripheral.sv`) — 227 lines
**Role:** Elapsed time counters (µs/ms/s)
**Timing characteristics:** Single-cycle reads, always ready. Cascaded counter design.
**Pipelining notes:** Well-optimized with FPGA-friendly cascaded counters.

### System Controller (`system_controller_peripheral.sv`) — 154 lines
**Role:** CPU boot, reset, halt control
**Timing characteristics:** Single-cycle, always ready. Simple register reads/writes.
**Pipelining notes:** No concerns.

### Primitives
- **`skid_buffer.sv`** (111 lines): Full 2-entry skid buffer. Ready for reuse.
- **`sync_fifo.sv`**: Synchronous FIFO. Used in UART.
- **`async_fifo.sv`**: Asynchronous FIFO for CDC.
- **`ff_sync.sv`**: Multi-stage flip-flop synchronizer.
- **`sync_dpram.sv`**: Simple dual-port RAM (inferred BRAM).
- **`sram.sv`**: Single-clock SRAM with byte write masking.

---

## 7. Implementation Roadmap

### Phase 1: Quick Wins with Skid Buffers (Near-Term, ~1-2 days)
1. Insert skid buffer between `host_bus_mux` and `host_bus_interface` (Opportunity A)
2. Insert skid buffer between `bus_arbiter` and `bus.sv` (Opportunity B)
3. Run synthesis and verify Fmax improvement
4. Run full test suite to verify functional correctness

### Phase 2: Internal Pipeline Registers (Near-Term, ~3-5 days)
1. Register writeback mux output earlier (Opportunity C)
2. Insert skid buffer on TX packet interface (Opportunity F)
3. Re-run synthesis and timing analysis

### Phase 3: Structural Improvements (Medium-Term, ~1-2 weeks)
1. Consider carry-select adder for ALU (if needed after Phase 1-2)
2. Evaluate decoder pipelining (Opportunity E)
3. Profile actual FPGA Fmax and identify new critical paths

### Phase 4: Full Pipeline (Long-Term, ~2-3 months)
1. Follow the plan in `docs/research/pipelined-cpu-design-approaches.md`
2. Requires separate instruction and data memory interfaces
3. Hazard detection, forwarding, and stalling logic
4. Branch prediction

---

## 8. Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Skid buffers increase latency by 1 cycle | Multi-cycle FSM already tolerates variable latency; the CPU waits for `ready` |
| Additional logic for skid buffers | Each skid buffer uses ~40-60 LUTs; plenty of headroom at 61% utilization |
| Functional regression | Comprehensive test suite (264 tests) will catch any issues |
| Skid buffer back-pressure deadlock | The CPU's FSM guarantees single outstanding requests; deadlock impossible |
| BRAM usage increase | Only if skid buffers are wide enough to infer BRAM; 67-89 bit widths will use LUTs |

---

## 9. Expected Outcomes

### After Phase 1 (Skid Buffers)
- **Fmax:** 37-40 MHz (from 34.91 MHz, ~6-14% improvement)
- **Logic utilization:** ~63-65% (small increase from skid buffer logic)
- **Functional behavior:** Identical (skid buffers are transparent when not stalling)

### After Phase 2 (Internal Registers)
- **Fmax:** 40-45 MHz (depends on new critical path)
- **Logic utilization:** ~64-66%
- **Cycle count:** May increase by 0-1 cycles for some instructions

### After Phase 4 (Full Pipeline)
- **Fmax:** 50-60+ MHz (depends on implementation quality)
- **Throughput:** ~4-5x improvement (1 instruction per cycle vs. 3-7 cycles)
- **Logic utilization:** 80-90% (significant increase for hazard logic)

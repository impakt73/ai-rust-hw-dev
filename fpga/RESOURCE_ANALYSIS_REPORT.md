# FPGA Resource Analysis Report

This report analyzes the resource consumption of each RTL module in the RISC-V CPU design when synthesized for the Alchitry Cu v1 board (iCE40-HX8K FPGA).

## Target Device: Lattice iCE40-HX8K-CB132

### Available Resources:
- **Logic Cells (LUTs):** 7,680
- **Flip-Flops (DFFs):** 7,680  
- **Block RAM (BRAM):** 32 blocks (4Kbit each = 16KB total)

---

## Executive Summary

**The full CPU design exceeds available FPGA resources by a significant margin.**

### Top Resource Consumers:

| Rank | Module | LUTs | % of Device | Issue | Status |
|------|--------|------|-------------|-------|--------|
| 1 | **ALU** (with div_unit) | ~~4,738~~ → 5,551 | ~~61.7%~~ → 72.3% | Single shared 64-bit multiplier + multi-cycle divider | ✅ **Hardware consolidated** |
| 2 | **FPU** (full) | ~~4,535~~ → 4,077 | ~~59.0%~~ → 53.1% | Single shared FMA unit + 48-bit divider | ✅ **Hardware consolidated** |
| 3 | **FPU_FMA** | 2,293 | 29.9% | Multiplier + Adder chain | Unchanged |
| 4 | **FPU_Multiplier** | 1,574 | 20.5% | 24x24 bit multiplier | Now shared via FMA |
| 5 | ~~**CSR File**~~ | ~~>7,680~~ → 193 | ~~>100%~~ → 2.5% | ~~4096x32-bit register array~~ | ✅ **FIXED!** |

**Critical Finding:** The ALU + FPU alone consume ~125% of available LUTs, making the design impossible to fit on the iCE40-HX8K without disabling extensions.

### Hardware Consolidation Summary (2026-01-28)

**ALU Changes:**
- Replaced 4 separate inline multiplications (MUL, MULH, MULHSU, MULHU) with a single shared 64×64 signed multiplier
- Operand preparation logic selects sign-extension or zero-extension based on instruction type
- Result selection extracts lower or upper 32 bits based on operation
- **Trade-off:** Slight LUT increase due to 64-bit multiplier, but now a single hardware instance that can be time-multiplexed in pipelined designs

**FPU Changes:**
- Replaced 7 arithmetic units (4 FMA + 2 adders + 1 multiplier) with a single shared FMA unit
- FPU_ADD routed as: `(fs1 × 1.0) + fs2`
- FPU_SUB routed as: `(fs1 × 1.0) - fs2`
- FPU_MUL routed as: `(fs1 × fs2) + 0.0`
- FMA operations use direct inputs with appropriate negate signals
- **Result:** 10% LUT reduction (4,535 → 4,077 LUTs) with single hardware instance

---

## Detailed Module Resource Summary

| Module | LUTs | DFFs | BRAM | % of Device | Status |
|--------|------|------|------|-------------|--------|
| branch_unit | 82 | 264 | 0 | 1.1% | ✅ OK |
| fpu_classifier | 33 | 255 | 0 | 0.4% | ✅ OK |
| fpu_comparator | 97 | 259 | 0 | 1.3% | ✅ OK |
| writeback_mux | 81 | 79 | 0 | 1.1% | ✅ OK |
| mem_interface | 35 | 79 | 0 | 0.5% | ✅ OK |
| fpu_sqrt | 32 | 266 | 0 | 0.4% | ✅ OK |
| fpu_div_setup | 61 | 258 | 0 | 0.8% | ✅ OK |
| fetch_buffer | 29 | 153 | 0 | 0.4% | ✅ OK |
| decoder | 74 | 162 | 0 | 1.0% | ✅ OK |
| decompress | 65 | 138 | 0 | 0.8% | ✅ OK |
| fpu_float_to_int | 217 | 277 | 0 | 2.8% | ✅ OK |
| fpu_int_to_float | 327 | 267 | 0 | 4.3% | ✅ OK |
| regfile | 409 | 335 | 0 | 5.3% | ⚡ Medium |
| fpu_div_assemble | 513 | 266 | 0 | 6.7% | ⚡ Medium |
| div_unit | 550 | 385 | 0 | 7.2% | ⚡ Medium |
| fp_regfile | 680 | 335 | 0 | 8.9% | ⚡ Medium |
| fpu_fma | 2,293 | 268 | 0 | 29.9% | ⚠️ HIGH (self-contained with inlined adder/multiplier) |
| fpu (full) | ~~4,535~~ → 4,077 | 442 | 0 | ~~59.0%~~ → 53.1% | 🟡 Improved (✅ consolidated) |
| alu (with div) | ~~4,738~~ → 5,551 | 411 | 0 | ~~61.7%~~ → 72.3% | 🟡 Consolidated (single multiplier) |
| csr_file | 193 | 231 | 0 | 2.5% | ✅ OK (FIXED!) |

**Note:** `fpu_adder.sv` and `fpu_multiplier.sv` have been deleted. Their logic is now inlined in `fpu_fma.sv`.

---

## Root Cause Analysis

### 1. ALU Module (5,551 LUTs - 72.3%) - ✅ Hardware Consolidated

**Previous Implementation:**
- Four inline 32x32→64 bit multiplications (MUL, MULH, MULHSU, MULHU)
- Synthesis created 4 separate multiplier instances

**Current Implementation (Consolidated):**
- Single shared 64×64 signed multiplier with operand preparation MUXes
- Operand extension logic selects sign/zero extension based on instruction
- Result selection extracts appropriate 32-bit portion

**Trade-off Analysis:**
- LUT increase from 4,738 to 5,551 (+17%) due to larger 64-bit multiplier
- However, now have **single hardware instance** that can be:
  - Time-multiplexed in multi-cycle designs
  - Shared with DSP blocks on FPGAs with hardware multipliers
  - Pipelined for higher throughput

**Remaining Recommendation:**
- **Make M extension configurable** via `ENABLE_M_EXT` parameter (already implemented)
- When disabled, eliminates the multiplier entirely (~5,000 LUT savings)

### 2. FPU Module (4,077 LUTs - 53.1%) - ✅ Hardware Consolidated

**Previous Implementation:**
- 4 FMA units (fpu_fma) for FMADD/FMSUB/FNMSUB/FNMADD
- 2 FPU adders (add + subtract)
- 1 FPU multiplier  
- 1 48-bit divider for FP division
- Multiple conversion units

**Current Implementation (Consolidated):**
- Single shared FMA unit with input multiplexing
- Routes all arithmetic operations through FMA:
  - ADD: `(fs1 × 1.0) + fs2`
  - SUB: `(fs1 × 1.0) - fs2`
  - MUL: `(fs1 × fs2) + 0.0`
  - MADD/MSUB/NMSUB/NMADD: Direct FMA with negate signals

**Result:**
- 10% LUT reduction (4,535 → 4,077 LUTs)
- Single hardware instance for all arithmetic operations

**Remaining Recommendation:**
- **Make F extension configurable** via `ENABLE_F_EXT` parameter
- When disabled, eliminates FPU entirely (~4,000 LUT savings)

### 3. CSR File (Fixed! Was: Synthesis Timeout - >100%)

**Original Problem:** The CSR file declared a 4096x32-bit register array:
```systemverilog
logic [31:0] csr_registers [0:4095];  // 131,072 flip-flops!
```

**Impact:** This required 131,072 flip-flops, but the iCE40-HX8K only has 7,680.

**Fix Applied:** Replaced with sparse implementation using individual registers for only the CSRs actually needed:
- MSTATUS, MISA, MEDELEG, MIDELEG, MIE, MTVEC, MSCRATCH, MEPC, MCAUSE, MTVAL
- Performance counters: CYCLE, INSTRET (64-bit)
- Read-only values: MVENDORID, MARCHID, MIMPID, MHARTID

**Result:** 193 LUTs (2.5% of device) - synthesis completes in ~1 second!

### 4. Register Files (409 + 680 = 1,089 LUTs)

**Problem:** Both integer and FP register files use LUT-based implementation.

**Recommendation:**
1. **Use BRAM** for register files (saves ~1,000 LUTs)
2. iCE40 has 32 BRAM blocks - use 2 for regfiles

---

## Recommendations Summary

### Fixes Applied ✅

1. **CSR File** - Replaced 4096-entry array with sparse implementation
   - Before: Synthesis timeout (>100% resources)
   - After: 193 LUTs (2.5% of device)
   - Status: **FIXED and tested**

2. **ALU Multiplier Consolidation** - Single shared 64×64 multiplier
   - Before: 4 separate inline multiplications
   - After: Single multiplier with operand preparation logic
   - LUT change: 4,738 → 5,551 (+17%, but now single hardware instance)
   - Status: **CONSOLIDATED and tested**

3. **FPU Hardware Consolidation** - Single shared FMA unit
   - Before: 4 FMA + 2 adders + 1 multiplier (7 units)
   - After: 1 FMA with input multiplexing
   - LUT change: 4,535 → 4,077 (-10%)
   - Status: **CONSOLIDATED and tested**

### Immediate Fixes Needed (Easy)

1. **Use BRAM for register files**
   - Expected savings: ~1,000 LUTs

### Architecture Changes (Medium)

2. **Make M extension optional** via `ENABLE_M_EXT` parameter
   - Expected savings: ~5,000 LUTs when disabled
   - Note: Parameter already implemented in ALU

3. **Make F extension optional** via `ENABLE_F_EXT` parameter
   - Expected savings: ~4,000 LUTs when disabled

### For Minimal FPGA Build (RV32I only)

| Component | LUTs (Estimated) |
|-----------|-----------------|
| ALU (no M ext) | ~500 |
| Decoder | 74 |
| Regfile (BRAM) | ~100 |
| CSR (minimal) | ~200 |
| Control FSM | ~300 |
| Memory interface | 35 |
| Other | ~300 |
| **Total** | **~1,500** |

This would use ~20% of iCE40-HX8K resources, leaving room for peripherals and memory.

---

## Proposed Configuration System

```systemverilog
module cpu #(
    parameter bit ENABLE_M_EXT = 1'b0,  // Multiply/Divide extension
    parameter bit ENABLE_F_EXT = 1'b0,  // Floating-point extension
    parameter bit ENABLE_C_EXT = 1'b1,  // Compressed instructions
    parameter bit ENABLE_A_EXT = 1'b0,  // Atomic extension
    parameter bit USE_BRAM_REGFILE = 1'b1  // Use BRAM for registers
) (
    // ... ports
);
```

---

## Files Created

- `fpga/resource_analysis/synth_harness.sv` - Test harness for module synthesis
- `fpga/resource_analysis/Makefile` - Build automation
- `fpga/resource_analysis/generate_report.sh` - Report generator script
- `fpga/RESOURCE_ANALYSIS_REPORT.md` - This report

---

## Next Steps

1. [x] Fix CSR file implementation (highest priority) - **DONE**
2. [x] Consolidate ALU multipliers - **DONE** (single 64×64 shared multiplier)
3. [x] Consolidate FPU hardware - **DONE** (single shared FMA unit)
4. [x] ENABLE_M_EXT parameter in ALU - **Already implemented**
5. [ ] Add ENABLE_F_EXT parameter to top module  
6. [ ] Convert register files to use BRAM
7. [ ] Re-synthesize full design with extensions disabled
8. [ ] Verify design fits in iCE40-HX8K

**Note:** The INSTRET counter is now properly implemented and increments on instruction completion.

---

*Report generated: 2026-01-28*
*Last updated: 2026-01-28 (Hardware consolidation: ALU multiplier + FPU FMA)*
*Synthesis tool: Yosys 0.33*
*Target: iCE40-HX8K-CB132 (Alchitry Cu v1)*

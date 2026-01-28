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
| 1 | **ALU** (with div_unit) | 4,738 | 61.7% | 64-bit multipliers + multi-cycle divider | Needs M extension disable option |
| 2 | **FPU** (full) | 4,535 | 59.0% | 4x FMA units + 48-bit divider | Needs F extension disable option |
| 3 | **FPU_FMA** | 2,293 | 29.9% | Multiplier + Adder chain | Part of FPU |
| 4 | **FPU_Multiplier** | 1,574 | 20.5% | 24x24 bit multiplier | Part of FPU |
| 5 | ~~**CSR File**~~ | ~~>7,680~~ → 193 | ~~>100%~~ → 2.5% | ~~4096x32-bit register array~~ | ✅ **FIXED!** |

**Critical Finding:** The ALU + FPU alone consume ~120% of available LUTs, making the design impossible to fit on the iCE40-HX8K without disabling extensions.

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
| fpu_adder | 599 | 267 | 0 | 7.8% | ⚡ Medium |
| fp_regfile | 680 | 335 | 0 | 8.9% | ⚡ Medium |
| fpu_multiplier | 1,574 | 258 | 0 | 20.5% | ⚠️ HIGH |
| fpu_fma | 2,293 | 268 | 0 | 29.9% | ⚠️ HIGH |
| fpu (full) | 4,535 | 442 | 0 | 59.0% | 🔴 CRITICAL |
| alu (with div) | 4,738 | 411 | 0 | 61.7% | 🔴 CRITICAL |
| csr_file | 193 | 231 | 0 | 2.5% | ✅ OK (FIXED!) |

---

## Root Cause Analysis

### 1. ALU Module (4,738 LUTs - 61.7%)

**Problem:** The ALU includes:
- Four 32x32→64 bit multipliers (MUL, MULH, MULHSU, MULHU)
- One 32-bit non-restoring divider (DIV/DIVU/REM/REMU)

**Impact:** A single 32x32 multiplier on iCE40 requires ~1,000-1,500 LUTs when implemented in fabric (no DSP blocks available).

**Recommendation:**
1. **Remove M extension** for FPGA targets (reduces to ~500 LUTs)
2. **Use iterative multiplier** (shift-add, ~200 LUTs but slower)
3. **Make M extension configurable** via parameter

### 2. FPU Module (4,535 LUTs - 59.0%)

**Problem:** The FPU instantiates:
- 4 FMA units (fpu_fma) for FMADD/FMSUB/FNMSUB/FNMADD
- 2 FPU adders (add + subtract)
- 1 FPU multiplier  
- 1 48-bit divider for FP division
- Multiple conversion units

**Impact:** Each FMA contains a multiplier (1,574 LUTs) + adder (599 LUTs).

**Recommendation:**
1. **Remove F extension** for FPGA targets (saves 4,535 LUTs)
2. **Share FMA hardware** - use 1 FMA instead of 4
3. **Use sequential FP operations** instead of parallel
4. **Make F extension configurable** via parameter

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

### Immediate Fixes Needed (Easy)

1. **Use BRAM for register files**
   - Expected savings: ~1,000 LUTs

### Architecture Changes (Medium)

2. **Make M extension optional** via `ENABLE_M_EXT` parameter
   - Expected savings: ~4,200 LUTs when disabled

3. **Make F extension optional** via `ENABLE_F_EXT` parameter
   - Expected savings: ~4,500 LUTs when disabled

4. **Share FPU hardware** - Single FMA/multiplier for all operations
   - Expected savings: ~2,000 LUTs

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
module top #(
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
2. [ ] Add ENABLE_M_EXT parameter to ALU
3. [ ] Add ENABLE_F_EXT parameter to top module  
4. [ ] Convert register files to use BRAM
5. [ ] Re-synthesize full design with extensions disabled
6. [ ] Verify design fits in iCE40-HX8K

**Note:** The INSTRET counter is now properly implemented and increments on instruction completion.

---

*Report generated: 2026-01-28*
*Synthesis tool: Yosys 0.33*
*Target: iCE40-HX8K-CB132 (Alchitry Cu v1)*

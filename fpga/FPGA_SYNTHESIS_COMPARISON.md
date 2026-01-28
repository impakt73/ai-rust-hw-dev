# FPGA Synthesis Before/After Comparison

This document compares the FPGA resource utilization of the RISC-V CPU design before and after the RTL optimizations.

## Target Device: Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)

### Available Resources:
- **Logic Cells (LUTs):** 7,680
- **Flip-Flops (DFFs):** 7,680
- **Block RAM (BRAM):** 32 blocks × 4Kbit = 128 Kbit (16 KB) total

---

## Synthesis Results Summary

### Configuration Comparison

| Metric | Original (RV32IMFC) | Optimized (RV32IAC) | Reduction |
|--------|---------------------|---------------------|-----------|
| **LUT4 Cells** | 17,073 | 4,039 | **-76.3%** |
| **DFF/Registers** | 3,307 | 1,750 | **-47.1%** |
| **Carry Chains** | 1,426 | 513 | **-64.0%** |
| **Block RAMs** | 16 | 16 | 0% |
| **Device Utilization** | 222%+ (overflow) | 74% | **FITS!** |
| **Synthesis Time** | ~140 sec | ~15 sec | **-89.3%** |

### Detailed Cell Count

| Cell Type | Original | Optimized | Notes |
|-----------|----------|-----------|-------|
| SB_LUT4 | 17,073 | 4,039 | Logic cells (7,680 available) |
| SB_CARRY | 1,426 | 513 | Carry chain elements |
| SB_DFF | 4 | 4 | Simple D flip-flops |
| SB_DFFE | 1,024 | 993 | DFFs with enable |
| SB_DFFER | 2,183 | 681 | DFFs with enable + reset |
| SB_DFFES | 7 | 7 | DFFs with enable + set |
| SB_DFFESR | 1 | 1 | DFFs with enable + sync reset |
| SB_DFFR | 87 | 64 | DFFs with reset |
| SB_DFFS | 1 | 0 | DFFs with set |
| SB_RAM40_4K | 16 | 16 | 4Kbit Block RAMs |

---

## What Changed

### 1. Register Files (Not Converted to BRAM)

The analysis found that **BRAM conversion is not feasible** for the register files on iCE40-HX8K:

| Requirement | iCE40 BRAM | Our Regfiles | Compatible? |
|-------------|------------|--------------|-------------|
| Minimum depth | 256 entries | 32 entries | ❌ No |
| Read style | Synchronous | Asynchronous | ❌ No |
| Read ports | 1 per BRAM | 2-3 ports | ❌ No |

**Outcome:** Register files remain LUT-based with documentation explaining the constraints. A `REGISTER_OUTPUTS` parameter was added for future timing optimization if needed.

**Resource impact:** ~1,089 LUTs (minimal change, ~5.3% of device)

### 2. M Extension Configuration (`ENABLE_M_EXT`)

The M extension (Multiply/Divide) was made configurable:

| Extension | Operations | Resource Impact |
|-----------|------------|-----------------|
| M enabled | MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU | ~4,200+ LUTs |
| M disabled | Returns 0 for all M operations | ~100 LUTs |

**Implementation:**
- `div_unit` module wrapped in `generate if (ENABLE_M_EXT)`
- Multiplier operations conditionally return 0 when disabled
- Division unit not instantiated when M disabled

### 3. F Extension Configuration (`ENABLE_F_EXT`)

The F extension (Single-Precision Floating-Point) was made configurable:

| Extension | Components | Resource Impact |
|-----------|------------|-----------------|
| F enabled | FPU, fp_regfile, FCSR, 4× FMA units | ~5,200+ LUTs |
| F disabled | No FPU components instantiated | ~0 LUTs |

**Implementation:**
- `fp_regfile` module wrapped in `generate if (ENABLE_F_EXT)`
- `fpu` module wrapped in `generate if (ENABLE_F_EXT)`
- FP operand registers conditionally generated
- FCSR logic conditionally generated

---

## Configuration Parameters

The design now supports compile-time configuration:

```systemverilog
module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F: Floating-Point (default: enabled)
) (
    input  logic       clk,
    input  logic       rst_n_btn,
    output logic [7:0] led
);
```

### Configuration Options

| ISA | ENABLE_M_EXT | ENABLE_F_EXT | Est. LUTs | Fits iCE40? |
|-----|--------------|--------------|-----------|-------------|
| RV32IMACF | 1 | 1 | ~17,000+ | ❌ No (222%) |
| RV32IMAC | 1 | 0 | ~11,800+ | ❌ No (154%) |
| RV32IAC | 0 | 0 | ~4,039 | ✅ Yes (53%) |
| RV32IC | 0 | 0 | ~4,039 | ✅ Yes (53%) |

---

## Place & Route Verification

The optimized design (RV32IAC, M and F disabled) was successfully placed and routed:

```
Device: iCE40-HX8K-CB132

Resource Utilization:
  ICESTORM_LC:   5703 / 7680  (74%)
  ICESTORM_RAM:    16 /   32  (50%)

Status: ✅ FITS ON DEVICE!
```

**Timing Notes:**
- At 100 MHz target: Timing failures (expected for iCE40)
- Recommended: Lower clock frequency to 10-25 MHz for timing closure
- The multi-cycle architecture helps meet timing on slower clocks

---

## Files Modified

### RTL Changes
| File | Change |
|------|--------|
| `rtl/regfile.sv` | Added `REGISTER_OUTPUTS` parameter + BRAM analysis docs |
| `rtl/fp_regfile.sv` | Added `REGISTER_OUTPUTS` parameter + BRAM analysis docs |
| `rtl/alu.sv` | Added `ENABLE_M_EXT` parameter, conditional M extension |
| `rtl/top.sv` | Added `ENABLE_M_EXT`, `ENABLE_F_EXT` parameters |
| `rtl/top_with_peripherals.sv` | Pass extension parameters through hierarchy |
| `fpga/fpga_top.sv` | Added extension parameters, pass to CPU |

### Documentation
| File | Content |
|------|---------|
| `fpga/BRAM_CONVERSION_ANALYSIS.md` | Detailed BRAM feasibility analysis |
| `fpga/FPGA_SYNTHESIS_COMPARISON.md` | This comparison document |
| `fpga/RESOURCE_ANALYSIS_REPORT.md` | Updated with optimization status |

---

## Recommendations

### For iCE40-HX8K (Alchitry Cu v1):

1. ✅ **Use RV32IAC configuration** (M and F disabled)
   - 74% device utilization leaves room for peripherals
   - All base integer, atomic, and compressed instructions supported

2. ⚠️ **Lower clock frequency** to 10-25 MHz
   - iCE40 fabric is slower than modern FPGAs
   - Multi-cycle architecture helps by spreading work across cycles

3. 📝 **Future optimization opportunities:**
   - Iterative multiply/divide (shift-add) for smaller M extension
   - Shared FPU hardware for smaller F extension
   - Registered regfile outputs for timing improvement

### For Larger FPGAs (Xilinx Artix-7, Intel Cyclone V, etc.):

- Keep full RV32IMACF configuration
- All features will fit with room to spare
- Use DSP blocks for hardware multiplication

---

## Synthesis Commands Used

### Baseline (Full RV32IMACF):
```bash
cd fpga
make all   # Uses Makefile defaults (all extensions enabled)
```

### Optimized (RV32IAC - M and F disabled):
```bash
cd fpga
yosys -p "read_verilog -sv ...; \
          hierarchy -top fpga_top -chparam ENABLE_M_EXT 0 -chparam ENABLE_F_EXT 0; \
          synth_ice40 -top fpga_top -json build/optimized.json"

nextpnr-ice40 --hx8k --package cb132 --json build/optimized.json \
              --pcf ice40hx8k.pcf --asc build/optimized.asc --freq 10
```

---

*Report generated: 2026-01-28*
*Synthesis tool: Yosys 0.33*
*Place & Route: nextpnr-ice40*
*Target: iCE40-HX8K-CB132 (Alchitry Cu v1)*

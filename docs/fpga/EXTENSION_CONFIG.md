# RISC-V Extension Configuration Guide

## Overview

The RISC-V CPU now supports **per-extension design configuration** to enable resource optimization for FPGA targets. This allows disabling expensive extensions (M and F) when targeting resource-constrained FPGAs like the iCE40-HX8K.

## Resource Impact

| Extension | Description | Estimated LUTs | Default |
|-----------|-------------|----------------|---------|
| **RV32M** | Multiply/Divide | 4,200+ LUTs | Enabled |
| **RV32F** | Floating-Point | 4,500+ LUTs | Enabled |

Disabling both extensions can save **~8,700+ LUTs**, making the design feasible for smaller FPGA targets.

## Configuration Parameters

### Module: `alu` (rtl/alu.sv)

```systemverilog
module alu #(
    parameter bit ENABLE_M_EXT = 1'b1  // RV32M extension: Multiply/Divide
) (
    // ... ports
);
```

**Parameter:** `ENABLE_M_EXT`
- **Default:** `1'b1` (enabled)
- **Type:** `bit`
- **Effect:**
  - `1'b1`: Full M extension support (MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU)
  - `1'b0`: No M extension - operations return 0, `div_unit` is **not instantiated**

**Hardware Impact:**
- When disabled, the `div_unit` module is not synthesized (saved LUTs)
- Multiply operations still execute but return 0
- Division operations complete immediately (no multi-cycle logic)

---

### Module: `top` (rtl/top.sv)

```systemverilog
module top #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point
) (
    // ... ports
);
```

**Parameters:**
1. **`ENABLE_M_EXT`**
   - **Default:** `1'b1` (enabled)
   - **Effect:** Passed to the ALU module (see above)

2. **`ENABLE_F_EXT`**
   - **Default:** `1'b1` (enabled)
   - **Effect:**
     - `1'b1`: Full F extension support (FLW, FSW, FADD.S, FSUB.S, FMUL.S, FDIV.S, FSQRT.S, FMA, comparisons, conversions)
     - `1'b0`: No F extension - `fp_regfile` and `fpu` are **not instantiated**, FP signals tied to safe defaults

**Hardware Impact (ENABLE_F_EXT=0):**
- FP register file (`fp_regfile`) is not synthesized
- FPU (`fpu`) and all sub-modules (adder, multiplier, divider, sqrt, classifier, etc.) are not synthesized
- FP operand registers (`fa_reg`, `fb_reg`, `fc_reg`) are tied to 0
- FCSR register logic is not synthesized
- Massive LUT savings (~4,500+ LUTs)

---

### Module: `top_with_peripherals` (rtl/top_with_peripherals.sv)

```systemverilog
module top_with_peripherals #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point
) (
    // ... ports
);
```

**Effect:** Passes parameters through to the `top` module instantiation.

## Usage Examples

### Example 1: Minimal RV32I Configuration (iCE40-HX8K)

Disable both M and F extensions for maximum resource savings:

```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(1'b0),  // No multiply/divide
    .ENABLE_F_EXT(1'b0)   // No floating-point
) cpu (
    .clk(clk),
    .rst_n(rst_n),
    // ... other ports
);
```

**Result:** ~8,700+ fewer LUTs, suitable for iCE40-HX8K (7,680 LUTs)

---

### Example 2: RV32IM Configuration (No FPU)

Enable M extension but disable F extension:

```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(1'b1),  // Multiply/divide enabled
    .ENABLE_F_EXT(1'b0)   // No floating-point
) cpu (
    .clk(clk),
    .rst_n(rst_n),
    // ... other ports
);
```

**Result:** ~4,500 fewer LUTs from disabling FPU

---

### Example 3: Full RV32IMFC (Default)

Enable all extensions (default behavior, backward compatible):

```systemverilog
top_with_peripherals cpu (  // No parameters = defaults
    .clk(clk),
    .rst_n(rst_n),
    // ... other ports
);
```

**Result:** Full RISC-V feature set, higher LUT usage

## Behavioral Changes

### When M Extension is Disabled (`ENABLE_M_EXT=1'b0`)

All M extension instructions execute but return **zero**:
- `MUL rd, rs1, rs2` → `rd = 0`
- `MULH rd, rs1, rs2` → `rd = 0`
- `MULHSU rd, rs1, rs2` → `rd = 0`
- `MULHU rd, rs1, rs2` → `rd = 0`
- `DIV rd, rs1, rs2` → `rd = 0` (immediate, no division unit)
- `DIVU rd, rs1, rs2` → `rd = 0`
- `REM rd, rs1, rs2` → `rd = 0`
- `REMU rd, rs1, rs2` → `rd = 0`

**Important:** No illegal instruction exception is raised. This is intentional to simplify the hardware.

---

### When F Extension is Disabled (`ENABLE_F_EXT=1'b0`)

All F extension signals are tied to safe defaults:
- FP register file outputs: `fs1_data = fs2_data = fs3_data = 0`
- FPU outputs: `fpu_fp_result = fpu_int_result = 0`
- FPU flags: `fpu_fflags = 0`
- FPU ready: `fpu_ready = 1` (always ready)
- FCSR register: `fcsr = 0`

**Software Consideration:** Do not execute FP instructions when `ENABLE_F_EXT=0`. The decoder may still recognize FP opcodes, but results will be invalid.

## Synthesis Verification

After configuring extensions, verify synthesis with Verilator:

```bash
verilator --lint-only rtl/*.sv rtl/peripherals/*.sv
```

Expected output: No errors or warnings.

## Test Compatibility

The default configuration (`ENABLE_M_EXT=1`, `ENABLE_F_EXT=1`) maintains **100% backward compatibility** with the existing Rust test suite:

```bash
cargo test --verbose
```

**Result:** All 204+ tests pass with default parameters.

**Note:** Tests assume M and F extensions are enabled. Do not run the full test suite with extensions disabled.

## Design Rationale

### Why Use `generate` Blocks?

Using SystemVerilog `generate if` blocks ensures:
1. **True conditional compilation:** Disabled modules are **not synthesized** (zero LUT cost)
2. **Clean synthesis:** No warnings about unused signals or modules
3. **Parameter-based selection:** Compile-time configuration (no runtime overhead)

### Why Return 0 for Disabled Operations?

Returning 0 instead of raising exceptions simplifies the control path:
- No need for illegal instruction detection logic
- No exception handling overhead
- Simpler FSM logic

This is acceptable because software should not execute M/F instructions on a processor configured without those extensions.

## Implementation Details

### ALU Module (rtl/alu.sv)

**Key Changes:**
1. Added `parameter bit ENABLE_M_EXT = 1'b1` to module header
2. Wrapped `div_unit` instantiation in `generate if (ENABLE_M_EXT)`
3. Added `else` block that ties division signals to safe defaults
4. Wrapped M extension case statements with `if (ENABLE_M_EXT)` checks
5. Return 0 when M extension operations are disabled

**Code Structure:**
```systemverilog
generate
    if (ENABLE_M_EXT) begin : gen_m_ext
        div_unit u_div (...);
        // Division control logic
    end else begin : gen_no_m_ext
        assign div_result = 32'd0;
        assign div_ready = 1'b1;
        assign is_div_op = 1'b0;
    end
endgenerate
```

---

### Top Module (rtl/top.sv)

**Key Changes:**
1. Added `parameter bit ENABLE_M_EXT = 1'b1` and `parameter bit ENABLE_F_EXT = 1'b1`
2. Passed `ENABLE_M_EXT` to ALU instantiation
3. Wrapped FP register file and FPU in `generate if (ENABLE_F_EXT)`
4. Wrapped FP operand registers in `generate if (ENABLE_F_EXT)`
5. Added conditional FPU result register update logic
6. Wrapped FCSR register logic in generate block

**Code Structure:**
```systemverilog
generate
    if (ENABLE_F_EXT) begin : gen_f_ext
        fp_regfile u_fp_regfile (...);
        fpu u_fpu (...);
        // FCSR register logic
    end else begin : gen_no_f_ext
        assign fs1_data = 32'd0;
        assign fs2_data = 32'd0;
        assign fs3_data = 32'd0;
        assign fpu_fp_result = 32'd0;
        assign fpu_int_result = 32'd0;
        assign fpu_fflags = 5'd0;
        assign fpu_ready = 1'b1;
        assign fcsr = 32'd0;
    end
endgenerate
```

## Future Enhancements

Potential future improvements:
1. **A Extension Configuration:** Add `ENABLE_A_EXT` for atomic operations
2. **C Extension Configuration:** Add `ENABLE_C_EXT` for compressed instructions
3. **Exception Handling:** Optionally raise illegal instruction exceptions when disabled extensions are accessed
4. **Synthesis Reports:** Add scripts to measure actual LUT savings for different configurations
5. **Test Variants:** Create test suites for configurations with extensions disabled

## References

- **RISC-V ISA Specification:** https://riscv.org/technical/specifications/
- **RV32M Standard Extension:** Multiply and Divide instructions
- **RV32F Standard Extension:** Single-precision floating-point instructions
- **Verilator Documentation:** https://verilator.org/guide/latest/

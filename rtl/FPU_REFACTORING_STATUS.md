# FPU Refactoring Status

## Objective
Refactor the FPU to eliminate all function calls, making it compatible with Yosys synthesis (even v0.61 has limitations with functions called from always blocks with non-constant arguments).

## Approach
Convert each function into a separate SystemVerilog module with combinational logic (always_comb), then instantiate these modules in the top-level FPU.

## Completed Submodules (7/13 operations)

### 1. `fpu_classifier.sv`
- **Replaces:** `is_nan()`, `is_snan()`, `is_inf()`, `is_zero()`, `is_subnormal()` functions
- **Interface:** Takes one 32-bit float input, outputs 5 classification signals
- **Status:** ✅ Complete

### 2. `fpu_comparator.sv`
- **Replaces:** `fp_less_than()` function
- **Interface:** Takes two 32-bit floats, outputs less_than signal
- **Status:** ✅ Complete

### 3. `fpu_adder.sv`
- **Replaces:** `fp_add_sub()` function
- **Key Change:** Replaced `while` loop with priority encoder for normalization
- **Interface:** Inputs: a, b, is_sub; Outputs: result, flags
- **Status:** ✅ Complete

### 4. `fpu_multiplier.sv`
- **Replaces:** `fp_mul()` function
- **Interface:** Inputs: a, b; Outputs: result, flags
- **Status:** ✅ Complete

### 5. `fpu_int_to_float.sv`
- **Replaces:** `int_to_float()` function
- **Interface:** Inputs: val, is_signed; Output: result
- **Status:** ✅ Complete

### 6. `fpu_float_to_int.sv`
- **Replaces:** `float_to_int()` function
- **Interface:** Inputs: val, is_signed; Outputs: result, invalid
- **Status:** ✅ Complete

### 7. `fpu_sqrt.sv`
- **Replaces:** `fp_sqrt()` function
- **Note:** Simplified implementation (approximation)
- **Status:** ✅ Complete

## Remaining Work

### 8. FMA Module (4 operations)
- **Needs:** `fp_fmadd()` → FPU_MADD, FPU_MSUB, FPU_NMSUB, FPU_NMADD
- **Approach:** Chain fpu_multiplier → fpu_adder with sign control
- **Status:** ⏳ Pending

### 9. Division Modules (2 operations)
- **Needs:** `fp_div_setup()` and `fp_div_assemble()`
- **Challenge:** Multi-cycle operation, state machine interaction
- **Approach:** Preserve existing div_unit integration structure
- **Status:** ⏳ Pending

### 10. New Top-Level FPU
- **Task:** Create refactored `fpu.sv` that:
  1. Instantiates all submodules
  2. Wires them in always_comb based on fpu_op
  3. Preserves division state machine
  4. Maintains same external interface
- **Status:** ⏳ Pending

## Testing Plan

1. **Per-Module:** Each submodule uses only combinational logic (synthesizable)
2. **Integration:** New fpu.sv must pass all 25 FPU tests
3. **Synthesis:** Verify Yosys can synthesize without function call errors

## Key Design Decisions

1. **No `while` loops:** Replaced with priority encoders or bounded iteration
2. **No `function` keyword:** All logic in modules with always_comb
3. **Modular:** Each operation is self-contained for maintainability
4. **Interface-preserving:** Top-level FPU keeps same ports as original

## Files Created

- `fpu_classifier.sv` (713 bytes)
- `fpu_comparator.sv` (1,292 bytes)
- `fpu_adder.sv` (5,357 bytes)
- `fpu_multiplier.sv` (2,500 bytes)
- `fpu_int_to_float.sv` (1,513 bytes)
- `fpu_float_to_int.sv` (2,105 bytes)
- `fpu_sqrt.sv` (1,743 bytes)
- `fpu.sv.original` (backup of original)

**Total new code:** ~15,000 bytes across 7 modules

## Estimated Remaining Work

- FMA module creation: ~1-2 hours
- Division module refactoring: ~2-3 hours (complex)
- New top-level FPU assembly: ~2-3 hours  
- Testing and debugging: ~2-4 hours
- **Total:** ~7-12 hours of focused development

## Notes

- Original FPU: 734 lines, 13 functions
- Refactored approach: ~7-10 modules + 1 top-level integrator
- Benefit: Full Yosys synthesis compatibility
- Trade-off: More files, but better modularity and maintainability

---
**Last Updated:** 2026-01-28
**Status:** ~54% complete (7/13 operations modularized)

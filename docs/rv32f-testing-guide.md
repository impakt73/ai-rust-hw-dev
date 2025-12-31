# RV32F Testing Guide for CPU-Sim

## Overview

This document provides detailed guidance for adding comprehensive floating point tests to the `cpu-sim` project to verify the RV32F (single-precision floating point) extension implementation.

## Test File Organization

### New Test Files to Create

1. **`tests/src/fp_regfile_test.rs`** - FP register file unit tests
2. **`tests/src/fpu_test.rs`** - FPU unit tests
3. **`tests/src/cpu_fp_test.rs`** - CPU-level FP integration tests

### Update Existing Files

1. **`tests/src/lib.rs`** - Add module declarations:
```rust
#[cfg(test)]
mod fp_regfile_test;

#[cfg(test)]
mod fpu_test;

#[cfg(test)]
mod cpu_fp_test;
```

2. **`tests/Cargo.toml`** - No changes needed (uses existing dependencies)

---

## FP Register File Tests (`fp_regfile_test.rs`)

### Test Template

```rust
use marlin::runtime::create_runtime;

#[test]
fn test_fp_regfile_basic_read_write() {
    let mut runtime = create_runtime();
    let mut dut = runtime.get_module("fp_regfile");
    
    // Reset
    dut.set("rst", 1);
    dut.set("clk", 0);
    dut.eval();
    dut.set("clk", 1);
    dut.eval();
    dut.set("rst", 0);
    dut.set("clk", 0);
    dut.eval();
    
    // Write to register f5
    dut.set("rd", 5);
    dut.set("rd_data", 0x40490FDB);  // 3.14159265 in IEEE 754
    dut.set("wr_en", 1);
    dut.set("clk", 1);
    dut.eval();
    dut.set("clk", 0);
    dut.set("wr_en", 0);
    dut.eval();
    
    // Read from register f5
    dut.set("rs1", 5);
    dut.eval();
    
    let rs1_data: u32 = dut.get("rs1_data");
    assert_eq!(rs1_data, 0x40490FDB, "FP register f5 read mismatch");
}
```

### Required Tests

1. **test_fp_regfile_basic_read_write** - Write and read single register
2. **test_fp_regfile_all_registers** - Verify all 32 registers work
3. **test_fp_regfile_simultaneous_reads** - Test rs1, rs2, rs3 at once
4. **test_fp_regfile_write_read_same_cycle** - Timing test
5. **test_fp_regfile_reset** - Verify all registers reset to 0x00000000

### Test Data Examples

```rust
// IEEE 754 single-precision test values
const POS_ZERO: u32 = 0x00000000;        // +0.0
const NEG_ZERO: u32 = 0x80000000;        // -0.0
const ONE: u32 = 0x3F800000;             // 1.0
const TWO: u32 = 0x40000000;             // 2.0
const THREE_POINT_14: u32 = 0x40490FDB;  // 3.14159265
const POS_INF: u32 = 0x7F800000;         // +infinity
const NEG_INF: u32 = 0xFF800000;         // -infinity
const QNAN: u32 = 0x7FC00000;            // Quiet NaN
```

---

## FPU Tests (`fpu_test.rs`)

### Test Template

```rust
use marlin::runtime::create_runtime;

#[test]
fn test_fpu_add_basic() {
    let mut runtime = create_runtime();
    let mut dut = runtime.get_module("fpu");
    
    // Test: 1.0 + 2.0 = 3.0
    dut.set("fs1", 0x3F800000);  // 1.0
    dut.set("fs2", 0x40000000);  // 2.0
    dut.set("fs3", 0);
    dut.set("int_src", 0);
    dut.set("fpu_op", 0);  // FPU_ADD
    dut.set("rm", 0);      // RNE rounding
    dut.eval();
    
    let fp_result: u32 = dut.get("fp_result");
    let expected: u32 = 0x40400000;  // 3.0
    assert_eq!(fp_result, expected, "1.0 + 2.0 should equal 3.0");
    
    let fflags: u8 = dut.get("fflags");
    // No exceptions expected for simple addition
    assert_eq!(fflags, 0, "No exception flags should be set");
}
```

### FPU Operation Codes

```rust
const FPU_ADD: u32 = 0;
const FPU_SUB: u32 = 1;
const FPU_MUL: u32 = 2;
const FPU_DIV: u32 = 3;
const FPU_SQRT: u32 = 4;
const FPU_MIN: u32 = 5;
const FPU_MAX: u32 = 6;
const FPU_MADD: u32 = 7;
const FPU_MSUB: u32 = 8;
const FPU_NMSUB: u32 = 9;
const FPU_NMADD: u32 = 10;
const FPU_SGNJ: u32 = 11;
const FPU_SGNJN: u32 = 12;
const FPU_SGNJX: u32 = 13;
const FPU_CVTWS: u32 = 14;
const FPU_CVTWUS: u32 = 15;
const FPU_CVTSW: u32 = 16;
const FPU_CVTSWU: u32 = 17;
const FPU_FEQ: u32 = 18;
const FPU_FLT: u32 = 19;
const FPU_FLE: u32 = 20;
const FPU_FCLASS: u32 = 21;
const FPU_MVXW: u32 = 22;
const FPU_MVWX: u32 = 23;
```

### Required Test Categories

#### 1. Arithmetic Tests
- **test_fpu_add_basic** - Simple addition
- **test_fpu_add_special** - Add with infinity, NaN, zero
- **test_fpu_sub_basic** - Simple subtraction
- **test_fpu_mul_basic** - Simple multiplication
- **test_fpu_mul_overflow** - Test overflow to infinity
- **test_fpu_div_basic** - Simple division
- **test_fpu_div_by_zero** - Division by zero handling
- **test_fpu_sqrt_basic** - Square root
- **test_fpu_sqrt_negative** - sqrt(-1) = NaN

#### 2. Comparison Tests
- **test_fpu_feq** - Floating point equal
- **test_fpu_flt** - Floating point less than
- **test_fpu_fle** - Floating point less than or equal
- **test_fpu_compare_nan** - NaN comparison behavior

#### 3. Conversion Tests
- **test_fpu_fcvt_w_s** - Float to signed int
- **test_fpu_fcvt_wu_s** - Float to unsigned int
- **test_fpu_fcvt_s_w** - Signed int to float
- **test_fpu_fcvt_s_wu** - Unsigned int to float

#### 4. Fused Operation Tests
- **test_fpu_fmadd** - Fused multiply-add
- **test_fpu_fmsub** - Fused multiply-sub
- **test_fpu_nmsub** - Fused negate-multiply-sub
- **test_fpu_nmadd** - Fused negate-multiply-add

#### 5. Sign Injection Tests
- **test_fpu_fsgnj** - Sign injection
- **test_fpu_fsgnjn** - Negate sign injection
- **test_fpu_fsgnjx** - XOR sign injection

#### 6. Classification Test
- **test_fpu_fclass** - Classify FP values

### Test Helper Functions

```rust
fn float_to_bits(f: f32) -> u32 {
    f.to_bits()
}

fn bits_to_float(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn assert_fp_eq(actual: u32, expected: u32, message: &str) {
    if actual != expected {
        let actual_f = bits_to_float(actual);
        let expected_f = bits_to_float(expected);
        panic!("{}: expected 0x{:08X} ({:.6}), got 0x{:08X} ({:.6})",
               message, expected, expected_f, actual, actual_f);
    }
}
```

---

## CPU Integration Tests (`cpu_fp_test.rs`)

### Test Template

```rust
use marlin::runtime::create_runtime;
use std::collections::HashMap;

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.set("clk", 0);
        $dut.eval();
        $dut.set("clk", 1);
        $dut.eval();
    };
}

#[test]
fn test_cpu_flw_fsw() {
    let mut runtime = create_runtime();
    let mut dut = runtime.get_module("top");
    
    // Memory maps
    let mut imem: HashMap<u32, u32> = HashMap::new();
    let mut dmem: HashMap<u32, u32> = HashMap::new();
    
    // Program: Load FP value, store it elsewhere
    // Address 0x1000: flw f0, 0(x1)
    // Address 0x1004: fsw f0, 4(x1)
    imem.insert(0x1000, encode_flw(0, 1, 0));
    imem.insert(0x1004, encode_fsw(0, 1, 4));
    
    // Data: float at address 0x2000
    dmem.insert(0x2000, 0x40490FDB);  // 3.14159
    
    // Reset CPU
    dut.set("rst", 1);
    clock_cycle!(dut);
    dut.set("rst", 0);
    
    // Provide PC and instruction
    dut.set("pc", 0x1000);
    dut.set("imem_rdata", imem[&0x1000]);
    
    // Set x1 = 0x2000 (base address)
    // (would need to load this first in real test)
    
    clock_cycle!(dut);
    
    // Check that FP load occurred
    // FP register f0 should now contain 3.14159
    
    // Next instruction: FSW
    dut.set("pc", 0x1004);
    dut.set("imem_rdata", imem[&0x1004]);
    clock_cycle!(dut);
    
    // Check that memory at 0x2004 now contains 3.14159
    let dmem_addr: u32 = dut.get("dmem_addr");
    assert_eq!(dmem_addr, 0x2004);
    
    let dmem_wdata: u32 = dut.get("dmem_wdata");
    assert_eq!(dmem_wdata, 0x40490FDB);
}
```

### Instruction Encoding Helpers

```rust
fn encode_flw(fd: u32, rs1: u32, imm: i32) -> u32 {
    let imm_bits = (imm as u32) & 0xFFF;
    (imm_bits << 20) | (rs1 << 15) | (0b010 << 12) | (fd << 7) | 0b0000111
}

fn encode_fsw(fs2: u32, rs1: u32, imm: i32) -> u32 {
    let imm_bits = imm as u32;
    let imm_11_5 = (imm_bits >> 5) & 0x7F;
    let imm_4_0 = imm_bits & 0x1F;
    (imm_11_5 << 25) | (fs2 << 20) | (rs1 << 15) | (0b010 << 12) | (imm_4_0 << 7) | 0b0100111
}

fn encode_fadd_s(fd: u32, fs1: u32, fs2: u32, rm: u32) -> u32 {
    (0b0000000 << 25) | (fs2 << 20) | (fs1 << 15) | (rm << 12) | (fd << 7) | 0b1010011
}

fn encode_fsub_s(fd: u32, fs1: u32, fs2: u32, rm: u32) -> u32 {
    (0b0000100 << 25) | (fs2 << 20) | (fs1 << 15) | (rm << 12) | (fd << 7) | 0b1010011
}

fn encode_fmul_s(fd: u32, fs1: u32, fs2: u32, rm: u32) -> u32 {
    (0b0001000 << 25) | (fs2 << 20) | (fs1 << 15) | (rm << 12) | (fd << 7) | 0b1010011
}

fn encode_fdiv_s(fd: u32, fs1: u32, fs2: u32, rm: u32) -> u32 {
    (0b0001100 << 25) | (fs2 << 20) | (fs1 << 15) | (rm << 12) | (fd << 7) | 0b1010011
}

fn encode_flt_s(rd: u32, fs1: u32, fs2: u32) -> u32 {
    (0b1010000 << 25) | (fs2 << 20) | (fs1 << 15) | (0b001 << 12) | (rd << 7) | 0b1010011
}

fn encode_fcvt_w_s(rd: u32, fs1: u32, rm: u32) -> u32 {
    (0b1100000 << 25) | (0b00000 << 20) | (fs1 << 15) | (rm << 12) | (rd << 7) | 0b1010011
}

fn encode_fcvt_s_w(fd: u32, rs1: u32, rm: u32) -> u32 {
    (0b1101000 << 25) | (0b00000 << 20) | (rs1 << 15) | (rm << 12) | (fd << 7) | 0b1010011
}
```

### Required CPU Integration Tests

1. **test_cpu_flw_fsw** - FP load/store
2. **test_cpu_fp_arithmetic_sequence** - Sequence of FP operations
3. **test_cpu_fp_comparison_branch** - FP compare with branch
4. **test_cpu_fp_int_conversion** - Convert int→float→int
5. **test_cpu_fcsr_read_write** - Test FCSR access
6. **test_cpu_fflags_accumulation** - Test exception flag accumulation
7. **test_cpu_frm_rounding** - Test different rounding modes
8. **test_cpu_fmadd** - Fused multiply-add in CPU
9. **test_cpu_fp_register_independence** - FP and int regs independent
10. **test_cpu_fp_edge_cases** - NaN, infinity, zero in CPU

---

## Test Execution

### Run Specific Test File

```bash
# Run FP register file tests only
cargo test --package cpu_verifier -- fp_regfile_test

# Run FPU tests only
cargo test --package cpu_verifier -- fpu_test

# Run CPU FP integration tests only
cargo test --package cpu_verifier -- cpu_fp_test

# Run all FP tests
cargo test --package cpu_verifier -- fp
```

### Run with Verbose Output

```bash
cargo test --package cpu_verifier -- fp_regfile_test --nocapture
```

### Run Single Test

```bash
cargo test --package cpu_verifier -- test_fpu_add_basic --nocapture
```

---

## Expected Test Count

After implementing all FP tests:

- **FP Register File Tests:** 5 tests
- **FPU Tests:** 20-25 tests
- **CPU FP Integration Tests:** 10-15 tests

**Total New FP Tests:** ~35-45 tests
**Combined with Existing:** 84 + 35-45 = **119-129 tests**

---

## Debugging Tips

### Enable Test Output

```rust
println!("fp_result = 0x{:08X} ({:.6})", fp_result, bits_to_float(fp_result));
```

### Check Exception Flags

```rust
let fflags: u8 = dut.get("fflags");
println!("Exception flags: NV={} DZ={} OF={} UF={} NX={}",
         (fflags >> 4) & 1,
         (fflags >> 3) & 1,
         (fflags >> 2) & 1,
         (fflags >> 1) & 1,
         fflags & 1);
```

### Verify Rounding

Test with different rounding modes:
```rust
for rm in 0..5 {
    dut.set("rm", rm);
    dut.eval();
    let result: u32 = dut.get("fp_result");
    println!("Rounding mode {}: result = 0x{:08X}", rm, result);
}
```

---

## References

- Main implementation plan: `docs/rv32f-upgrade-plan.md`
- RISC-V F extension spec: Chapter 11 of RISC-V Unprivileged ISA
- IEEE 754-2008: Standard for floating-point arithmetic

---

**Document Status:** Ready for use during Phase 2-6 of RV32F implementation


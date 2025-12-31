# RV32IM Upgrade Plan

## Executive Summary

This document outlines a comprehensive plan to upgrade the current **RV32I** single-cycle RISC-V CPU implementation to **RV32IM**, which adds the **M (Integer Multiplication and Division)** extension. The upgrade involves RTL modifications, comprehensive testing, and build configuration updates for all test programs.

## Table of Contents

1. [Overview of RV32IM Extension](#overview-of-rv32im-extension)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [RTL Modifications Required](#rtl-modifications-required)
4. [Testing Strategy](#testing-strategy)
5. [Build Configuration Updates](#build-configuration-updates)
6. [Implementation Phases](#implementation-phases)
7. [Risk Assessment](#risk-assessment)
8. [Validation Criteria](#validation-criteria)

---

## Overview of RV32IM Extension

### What is RV32IM?

RV32IM = RV32I (base integer instruction set) + M (integer multiplication and division extension)

### M Extension Instructions

The M extension adds **8 new instructions** for integer multiplication and division:

| Instruction | Opcode | Funct3 | Funct7 | Description |
|-------------|--------|--------|--------|-------------|
| **MUL**     | 0110011 | 000 | 0000001 | Multiply (lower 32 bits) |
| **MULH**    | 0110011 | 001 | 0000001 | Multiply High (signed × signed, upper 32 bits) |
| **MULHSU**  | 0110011 | 010 | 0000001 | Multiply High (signed × unsigned, upper 32 bits) |
| **MULHU**   | 0110011 | 011 | 0000001 | Multiply High (unsigned × unsigned, upper 32 bits) |
| **DIV**     | 0110011 | 100 | 0000001 | Divide (signed) |
| **DIVU**    | 0110011 | 101 | 0000001 | Divide (unsigned) |
| **REM**     | 0110011 | 110 | 0000001 | Remainder (signed) |
| **REMU**    | 0110011 | 111 | 0000001 | Remainder (unsigned) |

**Key Characteristics:**
- All M instructions use **R-type encoding** (same as ADD, SUB, etc.)
- Distinguished by `funct7 = 0000001` (decimal 1)
- Share the same opcode (0110011) as other R-type ALU operations
- Require 32×32 → 64-bit multiplication and 32÷32 → 32-bit division hardware

### Special Cases and Edge Conditions

#### Division by Zero
- **DIV/DIVU by zero:** Result = all 1's (0xFFFFFFFF)
- **REM/REMU by zero:** Result = dividend (unchanged)

#### Overflow Conditions
- **Signed division overflow:** `-2^31 ÷ -1` = `-2^31` (result = dividend)
- **Signed remainder overflow:** `-2^31 % -1` = `0`

---

## Current Architecture Analysis

### Existing RTL Modules

```
top.sv (CPU top-level)
├── decoder.sv (Instruction decoder)
├── alu.sv (ALU - arithmetic/logic/shift operations)
└── regfile.sv (32×32-bit register file)
```

### Current ALU Operations (RV32I only)

The ALU currently supports 10 operations:

```systemverilog
ALU_ADD  = 4'b0000  // Addition
ALU_SUB  = 4'b0001  // Subtraction
ALU_AND  = 4'b0010  // Bitwise AND
ALU_OR   = 4'b0011  // Bitwise OR
ALU_XOR  = 4'b0100  // Bitwise XOR
ALU_SLL  = 4'b0101  // Shift Left Logical
ALU_SRL  = 4'b0110  // Shift Right Logical
ALU_SRA  = 4'b0111  // Shift Right Arithmetic
ALU_SLT  = 4'b1000  // Set Less Than (signed)
ALU_SLTU = 4'b1001  // Set Less Than (unsigned)
```

**Current alu_op width:** 4 bits (supports up to 16 operations)

### Current Decoder Logic

The decoder recognizes R-type operations:
```systemverilog
OP_REG: begin
    alu_src = 1'b0;  // Use rs2
    reg_write = 1'b1;
    case (funct3)
        3'b000: alu_op = (funct7[5]) ? ALU_SUB : ALU_ADD;
        // ... other operations
    endcase
end
```

**Gap:** Currently only checks `funct7[5]` to distinguish SUB from ADD. Does not check for `funct7 = 0000001` which identifies M extension instructions.

---

## RTL Modifications Required

### 1. ALU Module (`rtl/alu.sv`)

#### Changes Needed:

**A. Expand alu_op encoding (4 bits → 4 bits, currently sufficient)**

Add 8 new operation codes:
```systemverilog
// Existing RV32I operations (0-9)
localparam logic [3:0] ALU_ADD  = 4'b0000;
localparam logic [3:0] ALU_SUB  = 4'b0001;
// ... existing operations ...

// New RV32IM multiplication operations (10-13)
localparam logic [3:0] ALU_MUL    = 4'b1010;  // Multiply (lower 32 bits)
localparam logic [3:0] ALU_MULH   = 4'b1011;  // Multiply High (signed×signed)
localparam logic [3:0] ALU_MULHSU = 4'b1100;  // Multiply High (signed×unsigned)
localparam logic [3:0] ALU_MULHU  = 4'b1101;  // Multiply High (unsigned×unsigned)

// New RV32IM division operations (14-15, then wrap to available codes)
localparam logic [3:0] ALU_DIV    = 4'b1110;  // Divide (signed)
localparam logic [3:0] ALU_DIVU   = 4'b1111;  // Divide (unsigned)

// NOTE: REM and REMU need encoding - options:
// Option 1: Expand to 5 bits for alu_op
// Option 2: Use currently unused codes in 4-bit space
// Recommendation: Keep 4-bit, use careful encoding
```

**Alternative Encoding Strategy (Recommended):**
Since we only have 16 codes in 4 bits and need 18 total operations (10 existing + 8 new), we should optimize:

```systemverilog
// Keep existing 0-9 for RV32I operations
// Use 10-15 for M extension (6 slots)
// Combine MUL/MULH/MULHSU/MULHU into a parameterized multiplier
// Or expand to 5 bits for future extensions (RV32IM + RV32F, etc.)
```

**Recommended: Expand to 5-bit alu_op encoding** to accommodate all operations cleanly and allow for future extensions.

**B. Implement multiplication logic**

```systemverilog
// 32×32 → 64-bit multiplication
logic [63:0] mul_result;

always_comb begin
    case (alu_op)
        ALU_MUL: begin
            mul_result = $signed(a) * $signed(b);
            result = mul_result[31:0];  // Lower 32 bits
        end
        ALU_MULH: begin
            mul_result = $signed(a) * $signed(b);
            result = mul_result[63:32];  // Upper 32 bits (signed×signed)
        end
        ALU_MULHSU: begin
            mul_result = $signed(a) * $unsigned(b);
            result = mul_result[63:32];  // Upper 32 bits (signed×unsigned)
        end
        ALU_MULHU: begin
            mul_result = $unsigned(a) * $unsigned(b);
            result = mul_result[63:32];  // Upper 32 bits (unsigned×unsigned)
        end
        // ... existing operations ...
    endcase
end
```

**C. Implement division logic**

```systemverilog
// Division and remainder operations
logic [31:0] div_result;
logic [31:0] rem_result;

always_comb begin
    case (alu_op)
        ALU_DIV: begin
            // Signed division with special cases
            if (b == 32'd0) begin
                result = 32'hFFFFFFFF;  // Division by zero
            end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
                result = 32'h80000000;  // Overflow case
            end else begin
                result = $signed(a) / $signed(b);
            end
        end
        ALU_DIVU: begin
            // Unsigned division
            if (b == 32'd0) begin
                result = 32'hFFFFFFFF;  // Division by zero
            end else begin
                result = $unsigned(a) / $unsigned(b);
            end
        end
        ALU_REM: begin
            // Signed remainder
            if (b == 32'd0) begin
                result = a;  // Division by zero
            end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
                result = 32'd0;  // Overflow case
            end else begin
                result = $signed(a) % $signed(b);
            end
        end
        ALU_REMU: begin
            // Unsigned remainder
            if (b == 32'd0) begin
                result = a;  // Division by zero
            end else begin
                result = $unsigned(a) % $unsigned(b);
            end
        end
        // ... existing operations ...
    endcase
end
```

**Implementation Note:** SystemVerilog synthesis tools typically infer hardware dividers for `/` and `%` operators. For single-cycle implementation, this will synthesize combinational division logic, which may have long critical paths. Multi-cycle division could be considered in future optimizations.

#### Modified ALU Interface:

```systemverilog
module alu (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,  // CHANGED: 4 bits → 5 bits
    output logic [31:0] result,
    output logic        zero
);
```

### 2. Decoder Module (`rtl/decoder.sv`)

#### Changes Needed:

**A. Update alu_op output width**

```systemverilog
module decoder (
    input  logic [31:0] instruction,
    // ... existing inputs/outputs ...
    output logic [4:0]  alu_op,  // CHANGED: 4 bits → 5 bits
    // ... existing outputs ...
);
```

**B. Add M extension detection logic**

```systemverilog
// Inside OP_REG case:
OP_REG: begin
    alu_src = 1'b0;
    reg_write = 1'b1;
    
    // Check for M extension (funct7 = 0000001)
    if (funct7 == 7'b0000001) begin
        // M extension instructions
        case (funct3)
            3'b000: alu_op = ALU_MUL;     // MUL
            3'b001: alu_op = ALU_MULH;    // MULH
            3'b010: alu_op = ALU_MULHSU;  // MULHSU
            3'b011: alu_op = ALU_MULHU;   // MULHU
            3'b100: alu_op = ALU_DIV;     // DIV
            3'b101: alu_op = ALU_DIVU;    // DIVU
            3'b110: alu_op = ALU_REM;     // REM
            3'b111: alu_op = ALU_REMU;    // REMU
            default: alu_op = ALU_ADD;
        endcase
    end else begin
        // Standard RV32I R-type instructions
        case (funct3)
            3'b000: alu_op = (funct7[5]) ? ALU_SUB : ALU_ADD;
            3'b111: alu_op = ALU_AND;
            3'b110: alu_op = ALU_OR;
            3'b100: alu_op = ALU_XOR;
            3'b001: alu_op = ALU_SLL;
            3'b101: alu_op = (funct7[5]) ? ALU_SRA : ALU_SRL;
            3'b010: alu_op = ALU_SLT;
            3'b011: alu_op = ALU_SLTU;
            default: alu_op = ALU_ADD;
        endcase
    end
end
```

#### Updated Parameter Definitions:

```systemverilog
// ALU operations (must match alu.sv)
localparam logic [4:0] ALU_ADD  = 5'b00000;
localparam logic [4:0] ALU_SUB  = 5'b00001;
// ... all existing operations updated to 5 bits ...
localparam logic [4:0] ALU_MUL    = 5'b01010;
localparam logic [4:0] ALU_MULH   = 5'b01011;
localparam logic [4:0] ALU_MULHSU = 5'b01100;
localparam logic [4:0] ALU_MULHU  = 5'b01101;
localparam logic [4:0] ALU_DIV    = 5'b01110;
localparam logic [4:0] ALU_DIVU   = 5'b01111;
localparam logic [4:0] ALU_REM    = 5'b10000;
localparam logic [4:0] ALU_REMU   = 5'b10001;
```

### 3. Top Module (`rtl/top.sv`)

**Minimal changes required** - only interface updates:

```systemverilog
// Internal signal width update
logic [4:0] alu_op;  // CHANGED: 4 bits → 5 bits

// Decoder instantiation - no changes needed (uses implicit connection)
// ALU instantiation - no changes needed (uses implicit connection)
```

### 4. Register File (`rtl/regfile.sv`)

**No changes required** - M extension instructions use standard R-type format and don't affect register file behavior.

---

## Testing Strategy

### Test Categories

#### 1. Unit Tests (ALU Level)

**File:** `tests/src/alu_test.rs`

Add new test module: `test_alu_m_extension()`

**Test cases:**

```rust
#[test]
fn test_alu_multiplication() {
    // Test MUL instruction
    // - Positive × Positive
    // - Negative × Negative
    // - Positive × Negative
    // - Zero multiplication
    // - Large numbers
    
    // Test MULH (signed × signed, upper 32 bits)
    // - Various signed combinations
    // - Overflow scenarios
    
    // Test MULHSU (signed × unsigned)
    // - Negative signed × positive unsigned
    // - Edge cases
    
    // Test MULHU (unsigned × unsigned)
    // - Large unsigned values
    // - Maximum values (0xFFFFFFFF × 0xFFFFFFFF)
}

#[test]
fn test_alu_division() {
    // Test DIV (signed division)
    // - Normal division: 20 ÷ 3 = 6
    // - Negative dividend: -20 ÷ 3 = -6
    // - Negative divisor: 20 ÷ -3 = -6
    // - Both negative: -20 ÷ -3 = 6
    // - Division by zero: returns 0xFFFFFFFF
    // - Overflow: 0x80000000 ÷ -1 = 0x80000000
    
    // Test DIVU (unsigned division)
    // - Normal division
    // - Large numbers
    // - Division by zero: returns 0xFFFFFFFF
}

#[test]
fn test_alu_remainder() {
    // Test REM (signed remainder)
    // - Normal: 20 % 3 = 2
    // - Negative dividend: -20 % 3 = -2
    // - Negative divisor: 20 % -3 = 2
    // - Both negative: -20 % -3 = -2
    // - Modulo by zero: returns dividend
    // - Overflow: 0x80000000 % -1 = 0
    
    // Test REMU (unsigned remainder)
    // - Normal remainder
    // - Large numbers
    // - Modulo by zero: returns dividend
}

#[test]
fn test_alu_m_extension_edge_cases() {
    // Comprehensive edge case testing
    // - All operations with 0
    // - All operations with 1
    // - All operations with -1
    // - All operations with 0x80000000 (most negative)
    // - All operations with 0xFFFFFFFF
}
```

**Estimated test count:** 5 new test functions, ~30+ individual test cases

#### 2. Integration Tests (CPU Level)

**File:** `tests/src/cpu_test.rs`

Add new test module: `test_cpu_m_extension()`

**Test cases:**

```rust
#[test]
fn test_cpu_mul_instructions() {
    // Execute MUL, MULH, MULHSU, MULHU in CPU
    // Verify results written to registers
    // Test instruction sequencing
}

#[test]
fn test_cpu_div_instructions() {
    // Execute DIV, DIVU with various operands
    // Test division by zero handling
    // Test overflow handling
}

#[test]
fn test_cpu_rem_instructions() {
    // Execute REM, REMU with various operands
    // Test modulo by zero handling
}

#[test]
fn test_cpu_m_extension_program() {
    // Complete program using M extension
    // Example: Calculate factorial using MUL
    // Example: Integer division algorithm using DIV
    // Verify multi-instruction sequences work correctly
}
```

**Estimated test count:** 4+ new test functions

#### 3. System-Level Tests (ELF Programs)

**Directory:** `test_programs/`

**New assembly test:** `m_extension_test.s`

```assembly
.section .text
.global _start

_start:
    # Test MUL - Multiply
    addi x1, x0, 10
    addi x2, x0, 20
    mul x3, x1, x2      # x3 = 200

    # Test MULH - Multiply high signed
    addi x4, x0, -1     # x4 = 0xFFFFFFFF
    addi x5, x0, -1     # x5 = 0xFFFFFFFF
    mulh x6, x4, x5     # x6 = upper 32 bits of (-1) × (-1)

    # Test DIV - Signed division
    addi x7, x0, 100
    addi x8, x0, 7
    div x9, x7, x8      # x9 = 100 ÷ 7 = 14

    # Test REM - Signed remainder
    rem x10, x7, x8     # x10 = 100 % 7 = 2

    # Test division by zero
    addi x11, x0, 0
    div x12, x7, x11    # x12 = 0xFFFFFFFF
    rem x13, x7, x11    # x13 = 100 (dividend)

    # Test overflow case
    lui x14, 0x80000    # x14 = 0x80000000 (most negative)
    addi x15, x0, -1    # x15 = 0xFFFFFFFF
    div x16, x14, x15   # x16 = 0x80000000
    rem x17, x14, x15   # x17 = 0

    # Halt
    lui x18, 0xFFFF0
    addi x18, x18, -16
    sw x0, 0(x18)
```

**New Rust test program:** Add M extension tests to `rust-test-program`

```rust
// In rust-test-program/src/m_extension_test.rs
#![no_std]
#![no_main]

#[no_mangle]
fn _start() -> ! {
    // Use inline assembly or compiler-generated M instructions
    let a: i32 = 123;
    let b: i32 = 456;
    
    // Multiplication (compiler will generate MUL)
    let product = a * b;
    
    // Division (compiler will generate DIV)
    let quotient = a / b;
    let remainder = a % b;
    
    // Write results to memory for verification
    // ... (similar to existing test programs)
    
    loop {}
}
```

#### 4. Verilator Linting

**Command:**
```bash
verilator --lint-only rtl/*.sv
```

Ensure no lint errors or warnings after modifications.

---

## Build Configuration Updates

### 1. Rust Test Programs

**Files to modify:**
- `rust-test-program/.cargo/config.toml`
- `rust-test-program/Cargo.toml`

**Current target:** `riscv32i-unknown-none-elf`
**New target:** `riscv32im-unknown-none-elf` or `riscv32imc-unknown-none-elf`

#### Option A: Use Custom Target Specification

Create `riscv32im-unknown-none-elf.json`:

```json
{
  "llvm-target": "riscv32",
  "target-pointer-width": "32",
  "target-c-int-width": "32",
  "data-layout": "e-m:e-p:32:32-i64:64-n32-S128",
  "arch": "riscv32",
  "cpu": "generic-rv32",
  "features": "+m,+a,+c",
  "max-atomic-width": 32,
  "llvm-abiname": "ilp32",
  "emit-debug-gdb-scripts": false,
  "eh-frame-header": false,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "executables": true,
  "panic-strategy": "abort",
  "relocation-model": "static"
}
```

#### Option B: Use Built-in Target (Recommended)

**Modify `.cargo/config.toml`:**

```toml
[build]
target = "riscv32im-unknown-none-elf"  # CHANGED: rv32i → rv32im
```

**Note:** As of Rust 1.92+, `riscv32im-unknown-none-elf` is a tier 2 target with host tools support.

**Verify target availability:**
```bash
rustup target list | grep riscv32im
```

**Add target if needed:**
```bash
rustup target add riscv32im-unknown-none-elf
```

#### Update Build Commands

**Old:**
```bash
cargo build --release --target riscv32i-unknown-none-elf
```

**New:**
```bash
cargo build --release --target riscv32im-unknown-none-elf
```

### 2. Assembly Test Programs

**Files to modify:**
- Update build commands in `test_programs/README.md`
- Create new build scripts if needed

**Current assembly command:**
```bash
riscv64-unknown-elf-as -march=rv32i -mabi=ilp32 -o test.o test.s
```

**Updated command:**
```bash
riscv64-unknown-elf-as -march=rv32im -mabi=ilp32 -o test.o test.s
```

**Linker command (no change needed):**
```bash
riscv64-unknown-elf-ld -T linker.ld -m elf32lriscv -o test.elf test.o
```

### 3. CPU Simulator

**Files to check:**
- `cpu-sim/src/main.rs` - May need updates if it validates instruction set
- `cpu-sim/README.md` - Update documentation to reflect RV32IM support

**Potential code changes:**

If the simulator checks ISA version:
```rust
// OLD
const ISA: &str = "rv32i";

// NEW
const ISA: &str = "rv32im";
```

### 4. CI/CD Pipeline

**Files to modify:**
- `.github/workflows/ci.yml`
- `.github/workflows/copilot-setup-steps.yml`

**Changes needed:**

#### Update copilot-setup-steps.yml (REQUIRED)

The copilot setup workflow currently installs `riscv32i-unknown-none-elf` (line 46) and needs to be updated to install `riscv32im-unknown-none-elf` instead:

```yaml
# OLD (line 45-46)
- name: Install RISC-V Rust Target
  run: rustup target add riscv32i-unknown-none-elf

# NEW
- name: Install RISC-V Rust Target
  run: rustup target add riscv32im-unknown-none-elf
```

Also update the verification step (line 48-49):

```yaml
# OLD
- name: Verify RISC-V Rust target
  run: rustup target list --installed | grep riscv32i-unknown-none-elf

# NEW
- name: Verify RISC-V Rust target
  run: rustup target list --installed | grep riscv32im-unknown-none-elf
```

And update the summary output (line 73):

```yaml
# OLD
echo "RISC-V Rust target: $(rustup target list --installed | grep riscv32i-unknown-none-elf)"

# NEW
echo "RISC-V Rust target: $(rustup target list --installed | grep riscv32im-unknown-none-elf)"
```

#### Update ci.yml (optional additions)

The CI workflow doesn't currently install the RISC-V target, but you may want to add it for building test programs:

```yaml
- name: Install RISC-V Rust Target
  run: rustup target add riscv32im-unknown-none-elf

- name: Build RV32IM test programs
  run: |
    cd rust-test-program
    cargo build --release --target riscv32im-unknown-none-elf
```

---

## Implementation Phases

### Phase 1: RTL Implementation (Estimated: 3-5 days)

**Tasks:**
1. [ ] Update `rtl/alu.sv`
   - Expand `alu_op` from 4 bits to 5 bits
   - Add ALU operation constants for M extension (MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU)
   - Implement multiplication logic (32×32 → 64-bit)
   - Implement division and remainder logic with edge case handling
   - Update always_comb block to handle new operations

2. [ ] Update `rtl/decoder.sv`
   - Expand `alu_op` output from 4 bits to 5 bits
   - Update ALU operation constants to 5 bits
   - Add M extension detection (check `funct7 == 7'b0000001`)
   - Implement funct3-based operation selection for M instructions
   - Keep existing RV32I decoding logic intact

3. [ ] Update `rtl/top.sv`
   - Update internal `alu_op` signal width to 5 bits
   - Verify all module connections are correct
   - No other changes required (decoder and ALU handle the rest)

4. [ ] Lint RTL changes
   ```bash
   verilator --lint-only rtl/*.sv
   ```

5. [ ] Fix any lint errors or warnings

**Validation:**
- RTL compiles without errors
- Verilator linting passes
- Existing RV32I tests still pass (regression testing)

### Phase 2: Unit Testing (Estimated: 2-3 days)

**Tasks:**
1. [ ] Create `tests/src/alu_test.rs` additions
   - `test_alu_multiplication()` - Test MUL, MULH, MULHSU, MULHU
   - `test_alu_division()` - Test DIV, DIVU with edge cases
   - `test_alu_remainder()` - Test REM, REMU with edge cases
   - `test_alu_m_extension_edge_cases()` - Comprehensive edge case testing

2. [ ] Add helper functions in `tests/src/cpu_test.rs`
   - Update `encode_r_type()` to support M extension instructions
   - Add constants for M opcodes and funct7 values

3. [ ] Run ALU tests in isolation
   ```bash
   cargo test --package cpu_verifier -- alu_test
   ```

4. [ ] Debug and fix any issues

**Validation:**
- All new ALU tests pass
- All existing ALU tests still pass (regression)

### Phase 3: Integration Testing (Estimated: 2-3 days)

**Tasks:**
1. [ ] Add CPU-level tests in `tests/src/cpu_test.rs`
   - `test_cpu_mul_instructions()` - Execute MUL variants in CPU context
   - `test_cpu_div_instructions()` - Execute DIV/DIVU with memory operations
   - `test_cpu_rem_instructions()` - Execute REM/REMU
   - `test_cpu_m_extension_program()` - Multi-instruction program using M extension

2. [ ] Run CPU integration tests
   ```bash
   cargo test --package cpu_verifier -- cpu_test
   ```

3. [ ] Verify all 28 existing tests still pass
   ```bash
   cargo test --verbose
   ```

4. [ ] Debug and fix any failures

**Validation:**
- All new CPU tests pass
- All existing tests pass (no regressions)
- Test count increases from 28 to ~35+ tests

### Phase 4: Build Configuration Updates (Estimated: 1-2 days)

**Tasks:**
1. [ ] Update Rust test program target
   - Modify `rust-test-program/.cargo/config.toml`
   - Change target from `riscv32i-unknown-none-elf` to `riscv32im-unknown-none-elf`
   - Verify target is available in rustup
   - Add target if needed: `rustup target add riscv32im-unknown-none-elf`

2. [ ] Update assembly build commands
   - Modify `test_programs/README.md`
   - Update `-march=rv32i` to `-march=rv32im` in all build instructions

3. [ ] Create new M extension test programs
   - Write `test_programs/m_extension_test.s` (assembly)
   - Add `rust-test-program/src/m_extension_test.rs` (Rust)
   - Update `rust-test-program/Cargo.toml` to include new binary

4. [ ] Build all test programs with new configuration
   ```bash
   cd rust-test-program
   cargo build --release --target riscv32im-unknown-none-elf
   ```

5. [ ] Update CI/CD workflows
   - Modify `.github/workflows/copilot-setup-steps.yml`
   - Change `rustup target add riscv32i-unknown-none-elf` to `riscv32im-unknown-none-elf` (line 46)
   - Update target verification to check for `riscv32im-unknown-none-elf` (line 49)
   - Update summary output to show `riscv32im-unknown-none-elf` (line 73)
   - Optionally add RISC-V target installation to `.github/workflows/ci.yml`

6. [ ] Update documentation
   - Modify `README.md` to reflect RV32IM support
   - Update `AGENTS.md` with M extension information
   - Add M extension instructions to supported instruction list

**Validation:**
- All test programs build successfully with RV32IM target
- No build errors or warnings
- CI/CD workflows install correct target
- Documentation accurately reflects new capabilities

### Phase 5: System-Level Testing (Estimated: 1-2 days)

**Tasks:**
1. [ ] Test CPU simulator with M extension programs
   ```bash
   cargo run --package cpu-sim -- test_programs/m_extension_test.elf --verbose
   ```

2. [ ] Verify ELF programs execute correctly
   - Check register values match expected results
   - Verify division by zero handling
   - Verify overflow handling

3. [ ] Add simulator integration tests in `cpu-sim/src/main.rs`
   - Test M extension program execution
   - Verify halt behavior

4. [ ] Run complete test suite
   ```bash
   cargo test --verbose
   cargo test --package cpu-sim
   ```

**Validation:**
- All simulator tests pass
- ELF programs execute correctly
- No crashes or unexpected behavior

### Phase 6: Final Validation and Documentation (Estimated: 1 day)

**Tasks:**
1. [ ] Run complete CI/CD pipeline locally
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo build --verbose
   cargo test --verbose
   verilator --lint-only rtl/*.sv
   ```

2. [ ] Update all documentation
   - [x] Create this implementation plan (`docs/rv32im-upgrade-plan.md`)
   - [ ] Update `README.md` to advertise RV32IM support
   - [ ] Update `AGENTS.md` with new instructions and test count
   - [ ] Update `test_programs/README.md` with M extension examples
   - [ ] Update `cpu-sim/README.md` with RV32IM notes

3. [ ] Create comprehensive PR description
   - Summary of changes
   - List of new instructions supported
   - Test coverage summary
   - Breaking changes (target architecture change)

4. [ ] Submit for code review

**Validation:**
- All CI checks pass (build, test, format, clippy, lint)
- Documentation is complete and accurate
- PR is ready for review

---

## Risk Assessment

### High-Risk Areas

#### 1. Combinational Division Logic Timing

**Risk:** Division operations create long critical paths in combinational logic, potentially violating timing constraints in single-cycle design.

**Mitigation:**
- Initial implementation uses synthesized division (`/` and `%` operators)
- Monitor synthesis reports for timing violations
- If necessary, consider multi-cycle division in future iterations
- Alternative: Use iterative division algorithms with state machines

**Impact:** High
**Likelihood:** Medium

#### 2. Test Coverage Gaps

**Risk:** Edge cases in M extension instructions may not be fully covered by tests, leading to undetected bugs.

**Mitigation:**
- Comprehensive edge case testing (division by zero, overflow, etc.)
- Use RISC-V compliance test suite (riscv-tests) for validation
- Test all signed/unsigned combinations
- Verify results against reference implementations (QEMU, Spike)

**Impact:** Medium
**Likelihood:** Low

#### 3. Backwards Compatibility

**Risk:** Changes to RTL or build configurations break existing RV32I functionality.

**Mitigation:**
- Run all existing 28 tests after each change (regression testing)
- Keep RV32I instruction decoding logic unchanged
- Only add new paths for M extension
- Verify all existing test programs still work

**Impact:** High
**Likelihood:** Low

### Medium-Risk Areas

#### 4. Build Configuration Drift

**Risk:** Different test programs use different targets, causing inconsistency.

**Mitigation:**
- Document target changes clearly in README files
- Update all build scripts and CI/CD consistently
- Verify all test programs build with same target

**Impact:** Medium
**Likelihood:** Low

#### 5. Synthesis Tool Variations

**Risk:** Different synthesis tools may handle multiplication/division differently, leading to inconsistent results.

**Mitigation:**
- Use Verilator consistently for all testing
- Document expected behavior clearly
- Test with multiple Verilator versions if possible

**Impact:** Low
**Likelihood:** Low

---

## Validation Criteria

### Functional Validation

**RTL Level:**
- [ ] All 8 M extension instructions decode correctly
- [ ] ALU produces correct results for all M operations
- [ ] Edge cases handled correctly (div by zero, overflow)
- [ ] Multiplication produces correct 64-bit results
- [ ] Division produces correct quotient and remainder

**CPU Level:**
- [ ] M extension instructions execute in single cycle
- [ ] Results written to correct destination registers
- [ ] Register x0 remains hardwired to zero
- [ ] Multi-instruction sequences work correctly
- [ ] Memory operations unaffected by M extension

**System Level:**
- [ ] Assembly programs using M instructions execute correctly
- [ ] Rust programs using `*`, `/`, `%` operators work
- [ ] CPU simulator runs M extension ELF files
- [ ] All halt conditions function properly

### Quality Validation

**Code Quality:**
- [ ] `cargo fmt -- --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `verilator --lint-only rtl/*.sv` passes
- [ ] No compiler warnings in Rust or SystemVerilog

**Testing:**
- [ ] Test count increases from 28 to 35+ tests
- [ ] All new tests pass
- [ ] All existing tests pass (no regressions)
- [ ] Code coverage includes all new instructions

**Documentation:**
- [ ] README.md updated with RV32IM support
- [ ] AGENTS.md updated with new instruction list
- [ ] Test programs documented with examples
- [ ] Build configuration changes documented

### CI/CD Validation

**Automated Checks:**
- [ ] GitHub Actions CI passes all jobs
- [ ] Build job completes successfully
- [ ] Test job runs all 35+ tests successfully
- [ ] Format check passes
- [ ] Clippy check passes

**Manual Review:**
- [ ] Code review completed
- [ ] Architecture changes approved
- [ ] Documentation reviewed
- [ ] Test coverage deemed sufficient

---

## Appendix A: RISC-V M Extension Reference

### Instruction Encoding

All M extension instructions use **R-type encoding**:

```
31       25 24    20 19    15 14   12 11     7 6      0
┌──────────┬────────┬────────┬───────┬────────┬────────┐
│  funct7  │   rs2  │   rs1  │funct3 │   rd   │ opcode │
└──────────┴────────┴────────┴───────┴────────┴────────┘
```

**For M extension:**
- `opcode = 0110011` (same as other R-type instructions)
- `funct7 = 0000001` (distinguishes M from other R-type)
- `funct3` selects the specific M operation (000-111)

### Instruction Semantics

```c
// MUL: rd = (rs1 * rs2)[31:0]
rd = (rs1 * rs2) & 0xFFFFFFFF;

// MULH: rd = ((signed)rs1 * (signed)rs2)[63:32]
int64_t result = (int64_t)(int32_t)rs1 * (int64_t)(int32_t)rs2;
rd = (result >> 32) & 0xFFFFFFFF;

// MULHSU: rd = ((signed)rs1 * (unsigned)rs2)[63:32]
int64_t result = (int64_t)(int32_t)rs1 * (uint64_t)rs2;
rd = (result >> 32) & 0xFFFFFFFF;

// MULHU: rd = ((unsigned)rs1 * (unsigned)rs2)[63:32]
uint64_t result = (uint64_t)rs1 * (uint64_t)rs2;
rd = (result >> 32) & 0xFFFFFFFF;

// DIV: rd = (signed)rs1 / (signed)rs2
if (rs2 == 0)
    rd = 0xFFFFFFFF;
else if (rs1 == 0x80000000 && rs2 == 0xFFFFFFFF)
    rd = 0x80000000;
else
    rd = (int32_t)rs1 / (int32_t)rs2;

// DIVU: rd = rs1 / rs2 (unsigned)
if (rs2 == 0)
    rd = 0xFFFFFFFF;
else
    rd = rs1 / rs2;

// REM: rd = (signed)rs1 % (signed)rs2
if (rs2 == 0)
    rd = rs1;
else if (rs1 == 0x80000000 && rs2 == 0xFFFFFFFF)
    rd = 0;
else
    rd = (int32_t)rs1 % (int32_t)rs2;

// REMU: rd = rs1 % rs2 (unsigned)
if (rs2 == 0)
    rd = rs1;
else
    rd = rs1 % rs2;
```

---

## Appendix B: Estimated Timeline

**Total Estimated Time:** 10-16 days

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: RTL Implementation | 3-5 days | None |
| Phase 2: Unit Testing | 2-3 days | Phase 1 complete |
| Phase 3: Integration Testing | 2-3 days | Phase 2 complete |
| Phase 4: Build Config Updates | 1-2 days | Phase 1 complete (parallel with 2/3) |
| Phase 5: System Testing | 1-2 days | Phases 3 & 4 complete |
| Phase 6: Final Validation | 1 day | All phases complete |

**Critical Path:** Phase 1 → Phase 2 → Phase 3 → Phase 5 → Phase 6

**Parallel Activities:** Phase 4 can run concurrently with Phases 2 and 3

---

## Appendix C: Resources

### RISC-V Specifications

- [RISC-V Unprivileged ISA Specification v20191213](https://riscv.org/wp-content/uploads/2019/12/riscv-spec-20191213.pdf)
  - Chapter 7: "M" Standard Extension for Integer Multiplication and Division

### Testing Resources

- [RISC-V Compliance Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)
- [RISC-V Tests Repository](https://github.com/riscv-software-src/riscv-tests)

### Toolchain Documentation

- [Rust Embedded Book - RISC-V](https://docs.rust-embedded.org/book/)
- [RISC-V GNU Toolchain](https://github.com/riscv-collab/riscv-gnu-toolchain)

### Verilator Documentation

- [Verilator User Guide](https://verilator.org/guide/latest/)
- [Verilator Synthesis Constructs](https://verilator.org/guide/latest/exe_verilator.html)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-31 | GitHub Copilot | Initial draft |

---

**Document Status:** ✅ **Ready for Implementation**

This plan provides a comprehensive roadmap for upgrading the RV32I CPU to RV32IM. All phases are clearly defined with specific tasks, validation criteria, and estimated timelines. The plan should be reviewed and approved before beginning implementation.

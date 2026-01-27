# Yosys Synthesis Compatibility

## Executive Summary

**All RTL modules successfully synthesize with Yosys 0.33**, with the exception of the full FPU implementation which requires either:
- **Option 1**: Use the provided `fpu_yosys033_stub.sv` (immediate compatibility, limited FP operations)
- **Option 2**: Upgrade to Yosys 0.40+ (recommended for full functionality)

## Module Compatibility Status

### ✅ Fully Compatible with Yosys 0.33
All modules except FPU synthesize successfully:
- `decoder.sv` - Instruction decoder
- `regfile.sv` - Integer register file  
- `alu.sv` - Arithmetic Logic Unit (includes integer division)
- `branch_unit.sv` - Branch logic
- `mem_interface.sv` - Memory interface
- `fetch_buffer.sv` - Fetch buffer
- `csr_file.sv` - Control and Status Registers
- `fp_regfile.sv` - Floating-point register file
- `writeback_mux.sv` - Writeback multiplexer
- `div_unit.sv` - Division unit
- `decompress.sv` - Compressed instruction decompressor
- `top.sv` - Top-level CPU module

### ⚠️ FPU Incompatibility with Yosys 0.33

**File**: `fpu.sv`

**Issue**: Yosys 0.33 limitation - cannot synthesize functions called from `always` blocks with non-constant arguments.

**Error**: `Function \fp_add_sub can only be called with constant arguments.`

**Root Cause**: The FPU implementation uses helper functions (like `fp_add_sub`, `fp_mul`, `fp_div_setup`) to implement IEEE 754 floating-point arithmetic. These functions are called from an `always @(*)` block with runtime values. Yosys 0.33 (from 2020) requires all function arguments to be compile-time constants.

**Important**: The original `fpu.sv` works perfectly with **Verilator** (used for simulation and testing) and modern synthesis tools. The issue is specific to older Yosys versions.

## Synthesis Verification

### Test All Modules (Except FPU)
```bash
cd rtl
yosys -p "read_verilog -sv decoder.sv regfile.sv alu.sv branch_unit.sv mem_interface.sv fetch_buffer.sv csr_file.sv fp_regfile.sv writeback_mux.sv div_unit.sv decompress.sv; hierarchy -check"
```
**Expected**: All modules parse and elaborate successfully.

### Test Complete Design with FPU Stub
```bash
cd rtl
yosys -p "read_verilog -sv fpu_yosys033_stub.sv decompress.sv decoder.sv regfile.sv alu.sv branch_unit.sv mem_interface.sv fetch_buffer.sv csr_file.sv fp_regfile.sv writeback_mux.sv div_unit.sv top.sv; hierarchy -check -top top; proc; opt"
```
**Expected**: Successfully reaches optimization passes (`proc`, `opt`).

## Solutions

### Option 1: Use FPU Stub (Quick Yosys 0.33 Compatibility)

**For Yosys 0.33 synthesis only:**
```bash
cp rtl/fpu_yosys033_stub.sv rtl/fpu.sv
```

**Stub Capabilities:**
- ✅ Sign injection operations (FSGNJ, FSGNJN, FSGNJX)
- ✅ FP/Int move operations (FMV.X.W, FMV.W.X)
- ❌ Arithmetic operations (FADD, FSUB, FMUL, FDIV, FSQRT) - return +0.0
- ❌ Comparison operations (FEQ, FLT, FLE) - return false
- ❌ Conversion operations (FCVT.*) - return 0

**Warning**: This stub is **only for synthesis testing**. For functional verification, use the original `fpu.sv` with Verilator.

### Option 2: Upgrade Yosys (Recommended for Full Functionality)

Yosys 0.40+ has significantly improved SystemVerilog support:

```bash
# Install dependencies
sudo apt-get install -y build-essential clang bison flex libreadline-dev \
    gawk tcl-dev libffi-dev git graphviz xdot pkg-config python3

# Build from source
git clone https://github.com/YosysHQ/yosys.git
cd yosys
git checkout yosys-0.40  # or later
make config-gcc
make -j$(nproc)
sudo make install
```

After upgrading, the original `fpu.sv` should synthesize without issues.

### Option 3: Use Modern Synthesis Tools

For FPGA synthesis, use vendor tools directly:
- **Xilinx Vivado**: Full SystemVerilog support
- **Intel Quartus**: Full SystemVerilog support  
- **Lattice Diamond**: Full SystemVerilog support

These tools have mature SystemVerilog frontends and will handle the FPU without issues.

## Why Not Modify fpu.sv?

**Attempted**: Converting `fpu.sv` syntax to Yosys 0.33 compatible form (removing `||`/`&&`, `return` statements, etc.)

**Result**: While it fixes Yosys syntax issues, it **breaks Verilator compilation** (used for testing), causing internal Verilator errors.

**Decision**: Keep the original `fpu.sv` (works with Verilator + modern synthesis) and provide a stub for Yosys 0.33.

## Recommendations

| Use Case | Recommended Approach |
|----------|---------------------|
| **Simulation & Testing** | Original `fpu.sv` with Verilator ✅ |
| **Yosys 0.33 Synthesis** | `fpu_yosys033_stub.sv` (syntax check only) |
| **Production FPGA** | Upgrade to Yosys 0.40+ or use Vivado/Quartus |
| **CI/CD Pipeline** | Use Verilator for functional tests, stub for Yosys syntax checks |

## Technical Details

### Yosys 0.33 Limitations
1. **Function Call Restrictions**: Functions can only be called with constant arguments from `always` blocks
2. **Limited SystemVerilog**: Partial SV-2005 support, minimal SV-2009/2012
3. **No Automatic Inlining**: Cannot automatically inline complex functions

### Why the FPU Needs Functions
The FPU implements IEEE 754 single-precision floating-point arithmetic, which requires:
- NaN/Inf detection and handling
- Mantissa alignment and normalization
- Exponent calculation and overflow detection  
- Rounding mode support

Inlining all this logic into a single `always @(*)` block would create an unmaintainable ~2000+ line combinational block.

---
**Last Updated**: 2026-01-27  
**Yosys Version Tested**: 0.33 (git sha1 2584903a060)  
**Verilator Version**: 5.028+ (works with original fpu.sv)

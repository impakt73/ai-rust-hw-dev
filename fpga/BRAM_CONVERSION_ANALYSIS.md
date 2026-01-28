# BRAM Conversion Analysis for Register Files

**Date:** 2026-01-28  
**Target:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tool:** Yosys 0.33

---

## Executive Summary

**TLDR: The register files CANNOT be converted to use Block RAM on iCE40 FPGAs due to architectural constraints.**

The integer and FP register files (`regfile.sv` and `fp_regfile.sv`) were analyzed for BRAM conversion to save LUT resources. While the resource analysis report identified these modules as candidates (~1,089 LUTs combined), **true BRAM inference is not possible** for the following reasons:

1. **Memory depth too small**: 32 entries (< 256 minimum for iCE40 BRAM inference)
2. **Asynchronous read requirement**: Multi-cycle CPU architecture requires zero-latency reads
3. **Multiple read ports**: FP regfile needs 3 simultaneous read ports

**Outcome:** Added `REGISTER_OUTPUTS` parameter to both modules for **future architectural changes**, but current recommendation is to keep it disabled (`REGISTER_OUTPUTS = 0`).

---

## Background: iCE40 BRAM Architecture

### BRAM Block Specifications

The iCE40-HX8K contains 32 BRAM blocks, each configured as:
- **256x16-bit** (4Kbit total)
- Synchronous read (1 cycle latency)
- Synchronous write
- Single read port, single write port (1R1W)

### Yosys BRAM Inference Requirements

For Yosys to infer BRAM on iCE40, the RTL must meet:

1. **Depth ≥ 256 entries** - Smaller arrays use distributed LUT RAM
2. **Synchronous reads** - Read address must be registered, data available next cycle
3. **Synchronous writes** - Write occurs on clock edge (both regfiles already have this)

**Multi-port reads require multiple BRAM blocks** - e.g., 2 read ports = 2x BRAM usage.

---

## Analysis: Integer Register File (`regfile.sv`)

### Current Implementation
```systemverilog
module regfile (
    input  logic        clk,
    input  logic        we,
    input  logic [4:0]  rs1_addr,    // Async read port 1
    input  logic [4:0]  rs2_addr,    // Async read port 2
    input  logic [4:0]  rd_addr,     // Sync write port
    input  logic [31:0] rd_data,
    output logic [31:0] rs1_data,    // Combinational output
    output logic [31:0] rs2_data     // Combinational output
);
    logic [31:0] registers [31:0];  // 32 entries (depth too small!)
    
    // Asynchronous reads (required by CPU DECODE state)
    always_comb begin
        rs1_data = (rs1_addr == 0) ? 32'd0 : registers[rs1_addr];
        rs2_data = (rs2_addr == 0) ? 32'd0 : registers[rs2_addr];
    end
```

### Resource Usage (Current)
- **LUTs:** ~409 (5.3% of iCE40-HX8K)
- **BRAM blocks:** 0

### Why BRAM Inference Fails

| Requirement | Current Implementation | Status |
|-------------|----------------------|--------|
| Depth ≥ 256 | 32 entries | ❌ FAIL |
| Sync reads | Async reads (always_comb) | ❌ FAIL |
| Sync writes | ✅ Synchronous write | ✅ PASS |

**Conclusion:** Only 1 out of 3 requirements met. Yosys will synthesize this as distributed LUT RAM.

### What Would Be Required for BRAM Inference

To use BRAM, the CPU architecture would need:

1. **Increase depth to 256 entries**
   - Wastes 224 register slots (only 32 used)
   - Inefficient use of BRAM capacity

2. **Pipeline DECODE stage to tolerate 1-cycle read latency**
   ```systemverilog
   // BRAM-compatible synchronous read (adds 1 cycle latency)
   always_ff @(posedge clk) begin
       rs1_data <= (rs1_addr == 0) ? 32'd0 : registers[rs1_addr];
       rs2_data <= (rs2_addr == 0) ? 32'd0 : registers[rs2_addr];
   end
   ```

3. **Add bypass/forwarding logic**
   - Handle read-after-write (RAW) hazards
   - Detect when `rd_addr == rs1_addr` or `rd_addr == rs2_addr`
   - Forward `rd_data` directly if addresses match

4. **Use 2 BRAM blocks for dual-port reads**
   - BRAM has 1R1W architecture
   - Need duplicate storage for simultaneous rs1/rs2 reads
   - Increases BRAM usage to 2 blocks

**Estimated effort:** Major architectural change (several days of work + verification)

**Estimated savings:** ~400 LUTs saved, but 2 BRAM blocks consumed (6.25% of BRAM budget)

**Cost-benefit:** **NOT RECOMMENDED** - Small depth makes this highly inefficient.

---

## Analysis: FP Register File (`fp_regfile.sv`)

### Current Implementation
```systemverilog
module fp_regfile (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        we,
    input  logic [4:0]  rs1_addr,    // Async read port 1
    input  logic [4:0]  rs2_addr,    // Async read port 2
    input  logic [4:0]  rs3_addr,    // Async read port 3 (for FMADD)
    input  logic [4:0]  rd_addr,     // Sync write port
    input  logic [31:0] rd_data,
    output logic [31:0] rs1_data,    // Combinational output
    output logic [31:0] rs2_data,    // Combinational output
    output logic [31:0] rs3_data     // Combinational output (3rd port!)
);
    logic [31:0] fp_registers [31:0];  // 32 entries
```

### Resource Usage (Current)
- **LUTs:** ~680 (8.9% of iCE40-HX8K)
- **BRAM blocks:** 0

### Why BRAM Inference Fails

| Requirement | Current Implementation | Status |
|-------------|----------------------|--------|
| Depth ≥ 256 | 32 entries | ❌ FAIL |
| Sync reads | Async reads (always_comb) | ❌ FAIL |
| Sync writes | ✅ Synchronous write | ✅ PASS |
| **3-port reads** | **3 simultaneous reads!** | ⚠️ **WORSE** |

**Additional Problem:** FP regfile requires **3 simultaneous read ports** (rs1, rs2, rs3) for FMA operations:
```
FMADD: rd = (rs1 × rs2) + rs3
```

To implement this with BRAM:
- Need **3 separate BRAM blocks** (1R1W × 3 = 3 blocks)
- Each BRAM stores a complete copy of the register file
- Triples BRAM usage!

### What Would Be Required for BRAM Inference

Same as integer regfile, PLUS:

5. **Use 3 BRAM blocks for tri-port reads**
   - 3 copies of the 256x32-bit register file
   - Synchronized writes to all 3 copies
   - 9.4% of total BRAM budget (3 out of 32 blocks)

**Estimated effort:** Major architectural change + triplicated BRAM management

**Estimated savings:** ~680 LUTs saved, but **3 BRAM blocks consumed** (9.4% of BRAM budget)

**Cost-benefit:** **STRONGLY NOT RECOMMENDED** - Extremely inefficient use of BRAM.

---

## Implementation: Configurable Output Registering

Both register files were updated with a `REGISTER_OUTPUTS` parameter:

```systemverilog
module regfile #(
    parameter bit REGISTER_OUTPUTS = 1'b0  // Default: async reads (LUT-based)
) (
    // ... ports unchanged ...
);

    logic [31:0] registers [31:0];
    logic [31:0] rs1_data_int, rs2_data_int;

    // Internal reads (always async from array)
    always_comb begin
        rs1_data_int = (rs1_addr == 0) ? 32'd0 : registers[rs1_addr];
        rs2_data_int = (rs2_addr == 0) ? 32'd0 : registers[rs2_addr];
    end

    // Output path: configurable
    generate
        if (REGISTER_OUTPUTS) begin : gen_registered_outputs
            // Registered outputs (adds 1 cycle latency, improves Fmax)
            // NOTE: Does NOT infer BRAM on iCE40 (depth < 256)
            always_ff @(posedge clk) begin
                rs1_data <= rs1_data_int;
                rs2_data <= rs2_data_int;
            end
        end else begin : gen_async_outputs
            // Direct combinational outputs (zero latency)
            always_comb begin
                rs1_data = rs1_data_int;
                rs2_data = rs2_data_int;
            end
        end
    endgenerate
endmodule
```

### What `REGISTER_OUTPUTS = 1` Actually Does

**IMPORTANT CLARIFICATION:** Setting `REGISTER_OUTPUTS = 1` does **NOT** infer BRAM on iCE40 due to the depth constraint (32 < 256).

What it **actually** does:
- Registers the read outputs (adds flip-flops after LUT RAM)
- Improves timing (Fmax) by breaking combinational paths
- Adds 1 cycle latency to register reads
- Storage is **still in LUTs** (distributed RAM)

**Use case for `REGISTER_OUTPUTS = 1`:**
- If timing closure fails due to long paths through regfile
- As a pipeline stage insertion point
- NOT for saving LUT resources (actually increases register usage!)

---

## Verification

All existing tests pass with the updated modules (using default `REGISTER_OUTPUTS = 0`):

### Integer Register File Tests
```bash
$ cargo test --test regfile_test
test test_regfile_write_read ... ok
test test_regfile_x0_hardwired ... ok
test test_regfile_simultaneous_read ... ok
test test_regfile_write_enable ... ok
test test_regfile_all_registers ... ok
test test_regfile_overwrite ... ok

test result: ok. 6 passed; 0 failed
```

### FP Register File Tests
```bash
$ cargo test --test fp_regfile_test
test test_fp_regfile_write_read ... ok
test test_fp_regfile_f0_writable ... ok
test test_fp_regfile_three_port_read ... ok
test test_fp_regfile_write_enable ... ok
test test_fp_regfile_all_registers ... ok
test test_fp_regfile_reset ... ok
test test_fp_regfile_overwrite ... ok

test result: ok. 7 passed; 0 failed
```

**Backward compatibility:** All instantiations remain unchanged (parameter defaults to 0).

---

## Alternative Optimization Strategies

Since BRAM conversion is not viable, consider these alternatives:

### 1. Remove F Extension (Highest Impact)
- **Savings:** ~4,500 LUTs (59% of device)
- **Impact:** Removes entire FP regfile (680 LUTs) + FPU
- **Recommendation:** ✅ Best option for fitting on iCE40-HX8K

### 2. Remove M Extension
- **Savings:** ~4,200 LUTs (55% of device)
- **Impact:** Keeps RV32I base ISA functional
- **Recommendation:** ✅ Good option for minimal CPU

### 3. Register File Optimization (Minimal Impact)
Since we can't use BRAM, other register file optimizations:

**a) Reduce register count (NOT RECOMMENDED)**
- Use 16 registers instead of 32 (violates RISC-V spec!)
- Savings: ~200-340 LUTs
- **Problem:** Breaks RISC-V ABI compatibility

**b) Share integer and FP register files**
- Merge into single 32x32-bit array
- Savings: ~0 LUTs (need muxing logic)
- **Problem:** Complicates ISA implementation

**c) Accept LUT-based implementation**
- ✅ **RECOMMENDED:** Keep current design
- 1,089 LUTs is acceptable (~14% of device)
- Only problematic if F/M extensions are enabled

---

## Recommendations

### For Current iCE40-HX8K Build

1. ✅ **Keep `REGISTER_OUTPUTS = 0`** (async reads, LUT-based storage)
2. ✅ **Disable F extension** (`ENABLE_F_EXT = 0`) - saves 4,500 LUTs
3. ✅ **Disable M extension** (`ENABLE_M_EXT = 0`) - saves 4,200 LUTs
4. ✅ **Build RV32IC only** (base + compressed) - target ~1,500 LUTs total

### For Future ASIC or Larger FPGAs

If targeting larger devices (e.g., Xilinx 7-Series, Intel Cyclone):

1. Consider BRAM for register files if:
   - BRAM blocks support smaller depths (e.g., Xilinx distributed RAM)
   - Multi-port BRAMs available (avoid triplication)
   - BRAM is more abundant than LUTs

2. Or keep LUT-based implementation:
   - Simplifies timing closure
   - Zero-latency reads
   - Less complex design

---

## Files Modified

1. **`rtl/regfile.sv`**
   - Added `REGISTER_OUTPUTS` parameter
   - Added comprehensive BRAM inference analysis comments
   - Implemented configurable output path (async/sync)
   - Maintained backward compatibility (default `REGISTER_OUTPUTS = 0`)

2. **`rtl/fp_regfile.sv`**
   - Added `REGISTER_OUTPUTS` parameter
   - Added comprehensive BRAM inference analysis comments
   - Implemented configurable output path (async/sync)
   - Handled 3-port read complexity
   - Maintained backward compatibility (default `REGISTER_OUTPUTS = 0`)

3. **`fpga/BRAM_CONVERSION_ANALYSIS.md`** (this document)
   - Detailed technical analysis
   - Architectural requirements documentation
   - Alternative optimization strategies

---

## Conclusion

**BRAM conversion for register files is NOT FEASIBLE on iCE40-HX8K due to:**
1. ❌ Memory depth too small (32 < 256 minimum)
2. ❌ Asynchronous read requirement
3. ❌ Multi-port read complexity (especially FP regfile)

**Savings would be minimal and cost would be high:**
- LUT savings: ~1,000 LUTs
- BRAM cost: 5 blocks (15.6% of BRAM budget)
- Architectural complexity: High
- Verification effort: Days

**Better alternatives:**
- ✅ Disable F extension → save 4,500 LUTs
- ✅ Disable M extension → save 4,200 LUTs
- ✅ Accept LUT-based register files (~14% device usage)

The `REGISTER_OUTPUTS` parameter remains available for future architectural exploration, but current recommendation is to **keep it disabled**.

---

*Analysis completed: 2026-01-28*  
*All register file tests pass with updated modules*  
*Backward compatibility maintained*

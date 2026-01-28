# BRAM Conversion Task - Summary

## Task Completion Status: ✅ COMPLETE

### Objective
Analyze and implement BRAM conversion for integer and FP register files to reduce LUT usage on iCE40-HX8K FPGA.

### Key Findings

**BRAM conversion is NOT FEASIBLE on iCE40-HX8K** due to architectural constraints:

1. ✗ **Memory depth too small**: 32 entries (< 256 minimum for iCE40 BRAM inference)
2. ✗ **Async reads required**: Multi-cycle CPU needs zero-latency register access
3. ✗ **Multi-port reads**: FP regfile needs 3 simultaneous read ports

### Changes Implemented

#### 1. Updated RTL Modules
- **`rtl/regfile.sv`**: Added REGISTER_OUTPUTS parameter and comprehensive documentation
- **`rtl/fp_regfile.sv`**: Added REGISTER_OUTPUTS parameter and comprehensive documentation

#### 2. Documentation Created
- **`fpga/BRAM_CONVERSION_ANALYSIS.md`**: 12KB detailed technical analysis
- **`fpga/RESOURCE_ANALYSIS_REPORT.md`**: Updated with findings and realistic estimates

#### 3. Key Features
- ✅ `REGISTER_OUTPUTS` parameter for output registering (timing improvement, not BRAM)
- ✅ Comprehensive inline documentation in RTL
- ✅ Backward compatible (parameter defaults to 0)
- ✅ All tests pass (13 total: 6 regfile + 7 fp_regfile)
- ✅ Synthesis verified (409 LUTs + 680 LUTs = 1,089 LUTs total)

### Verification Results

**Tests:**
```
cargo test --test regfile_test     → 6 passed
cargo test --test fp_regfile_test  → 7 passed
```

**Synthesis:**
```
make synth-regfile     → 409 LUTs (unchanged)
make synth-fp_regfile  → 680 LUTs (unchanged)
```

### Recommendations

For iCE40-HX8K builds:

1. ✅ **Keep REGISTER_OUTPUTS = 0** (LUT-based storage, async reads)
2. ✅ **Disable F extension** to save 4,500 LUTs (59% of device)
3. ✅ **Disable M extension** to save 4,200 LUTs (55% of device)
4. ✅ **Target RV32IC only** (~1,500 LUTs total, 20% of device)

### What REGISTER_OUTPUTS Actually Does

**Setting `REGISTER_OUTPUTS = 1` does NOT use BRAM.** It only:
- Registers the read outputs (adds flip-flops)
- Improves timing (breaks combinational paths)
- Adds 1 cycle latency
- Storage remains in LUTs (distributed RAM)

**Use case:** Timing closure failures, not resource optimization.

### Cost-Benefit Analysis

To actually use BRAM would require:
- **Effort**: Major CPU architecture redesign (several days)
- **BRAM cost**: 3-5 blocks (15% of BRAM budget)
- **LUT savings**: ~1,000 LUTs
- **Downsides**: 1-cycle latency, complex bypass logic, wasted BRAM capacity

**Conclusion:** NOT WORTH IT. Better to disable F/M extensions.

### Files Modified

```
fpga/BRAM_CONVERSION_ANALYSIS.md    (new, 12KB documentation)
fpga/RESOURCE_ANALYSIS_REPORT.md    (updated with findings)
rtl/regfile.sv                       (added parameter + docs)
rtl/fp_regfile.sv                    (added parameter + docs)
```

### Git Commits

1. `aab63ef` - Initial analysis and implementation
2. `7491f34` - Code review fixes (parameter rename, simplifications)

### Next Steps (Not Part of This Task)

1. [ ] Add ENABLE_M_EXT parameter to ALU
2. [ ] Add ENABLE_F_EXT parameter to top module
3. [ ] Re-synthesize full design with extensions disabled
4. [ ] Verify RV32IC fits in iCE40-HX8K

---

**Task completed**: 2026-01-28  
**Status**: Ready for PR review  
**Backward compatible**: Yes (all tests pass, synthesis verified)

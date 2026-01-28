# Testing Extension Configuration

## Quick Verification

To verify that the extension configuration parameters are working correctly:

### 1. Syntax Check (Verilator Lint)

```bash
cd /home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev
verilator --lint-only rtl/*.sv rtl/peripherals/*.sv
```

**Expected:** No errors or warnings (exit code 0)

### 2. Default Configuration Test (All Extensions Enabled)

```bash
cargo test --verbose
```

**Expected:** All 204+ tests pass
- This verifies backward compatibility with `ENABLE_M_EXT=1`, `ENABLE_F_EXT=1`

### 3. Synthesis Verification

The following synthesizable configurations are supported:

#### Configuration A: Full RV32IMFC (Default)
```systemverilog
top_with_peripherals cpu (
    // No parameters = defaults (ENABLE_M_EXT=1, ENABLE_F_EXT=1)
    .clk(clk),
    .rst_n(rst_n),
    .boot_addr(32'h80000000),
    // ... other ports
);
```

#### Configuration B: RV32IM (No Floating-Point)
```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(1'b1),
    .ENABLE_F_EXT(1'b0)
) cpu (
    .clk(clk),
    .rst_n(rst_n),
    .boot_addr(32'h80000000),
    // ... other ports
);
```
**Savings:** ~4,500 LUTs (no FPU)

#### Configuration C: RV32I (Minimal)
```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(1'b0),
    .ENABLE_F_EXT(1'b0)
) cpu (
    .clk(clk),
    .rst_n(rst_n),
    .boot_addr(32'h80000000),
    // ... other ports
);
```
**Savings:** ~8,700+ LUTs (no M extension, no FPU)

All configurations should:
- ✅ Pass Verilator linting
- ✅ Synthesize without errors
- ✅ Produce no synthesis warnings

## Manual Testing (Optional)

To test a specific configuration interactively:

1. **Create a test module** in `rtl/test_config.sv`:

```systemverilog
// Test configuration: RV32I (minimal)
module test_config (
    input  logic clk,
    input  logic rst_n
);

    // Instantiate CPU with minimal configuration
    top_with_peripherals #(
        .ENABLE_M_EXT(1'b0),  // Disable M extension
        .ENABLE_F_EXT(1'b0)   // Disable F extension
    ) cpu (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(32'h80000000),
        
        // Tie off memory interfaces
        .imem_addr(),
        .imem_data(32'h00000013),  // NOP instruction
        .imem_req(),
        .imem_ready(1'b1),
        
        .ext_mem_addr(),
        .ext_mem_wdata(),
        .ext_mem_rdata(32'h0),
        .ext_mem_we(),
        .ext_mem_re(),
        .ext_mem_size(),
        .ext_mem_req(),
        .ext_mem_ready(1'b1),
        
        .led_out(),
        .halted(),
        .instr_complete(),
        .debug_rs1_data(),
        .debug_rs2_data(),
        .debug_rd_data(),
        .debug_pc(),
        .debug_instruction(),
        .debug_current_pc(),
        .debug_current_instruction(),
        .debug_fsm_state()
    );

endmodule
```

2. **Lint the test module:**

```bash
verilator --lint-only rtl/test_config.sv rtl/*.sv rtl/peripherals/*.sv
```

3. **Clean up:**

```bash
rm rtl/test_config.sv
```

## Expected Behavior

### When M Extension is Disabled

The following instructions will **execute but return 0**:
- `MUL` → result = 0
- `MULH` → result = 0
- `MULHSU` → result = 0
- `MULHU` → result = 0
- `DIV` → result = 0 (immediate, no multi-cycle delay)
- `DIVU` → result = 0
- `REM` → result = 0
- `REMU` → result = 0

**Note:** No illegal instruction exception is raised.

### When F Extension is Disabled

All floating-point signals return safe defaults:
- FP register outputs: 0
- FPU results: 0
- FPU flags: 0
- FPU ready: 1 (always ready)

**Note:** Do not execute FP instructions when F extension is disabled.

## CI/CD Integration

The CI pipeline should test the default configuration only:

```yaml
- name: Verilator Lint
  run: verilator --lint-only rtl/*.sv rtl/peripherals/*.sv

- name: Rust Test Suite
  run: cargo test --verbose
```

**Rationale:** Tests assume M and F extensions are enabled. Alternative configurations should be tested in synthesis-only workflows.

## Future Test Coverage

Planned enhancements:
1. Add testbenches that verify disabled extensions return expected defaults
2. Create synthesis reports for different configurations
3. Add resource utilization comparison tests
4. Test edge cases (e.g., what happens when M-extension instruction is decoded with `ENABLE_M_EXT=0`)

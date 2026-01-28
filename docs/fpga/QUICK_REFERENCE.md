# Quick Reference: Extension Configuration

## TL;DR

Disable expensive RISC-V extensions to save FPGA resources:

```systemverilog
// Minimal RV32I configuration (saves ~8,700 LUTs)
top_with_peripherals #(
    .ENABLE_M_EXT(1'b0),  // No multiply/divide
    .ENABLE_F_EXT(1'b0)   // No floating-point
) cpu (
    .clk(clk),
    .rst_n(rst_n),
    .boot_addr(32'h80000000),
    // ... connect all other ports
);
```

## Parameters

| Parameter | Default | Description | LUT Savings |
|-----------|---------|-------------|-------------|
| `ENABLE_M_EXT` | `1'b1` | RV32M (Multiply/Divide) | ~4,200 LUTs |
| `ENABLE_F_EXT` | `1'b1` | RV32F (Floating-Point) | ~4,500 LUTs |

## Where to Use

Available in these modules:
- `alu` (rtl/alu.sv) - accepts `ENABLE_M_EXT`
- `top` (rtl/top.sv) - accepts both parameters
- `top_with_peripherals` (rtl/top_with_peripherals.sv) - accepts both parameters

## Common Configurations

### Full Feature Set (Default)
```systemverilog
top_with_peripherals cpu (...);  // No parameters needed
```
**ISA:** RV32IMFC  
**LUTs:** ~15,000+ (estimated)

### No Floating-Point
```systemverilog
top_with_peripherals #(.ENABLE_F_EXT(1'b0)) cpu (...);
```
**ISA:** RV32IM  
**LUTs:** ~10,500+ (saves 4,500)

### Minimal
```systemverilog
top_with_peripherals #(
    .ENABLE_M_EXT(1'b0),
    .ENABLE_F_EXT(1'b0)
) cpu (...);
```
**ISA:** RV32I  
**LUTs:** ~6,300+ (saves 8,700)  
**Target:** iCE40-HX8K (7,680 LUTs)

## Behavior When Disabled

### M Extension Off
- `MUL/MULH/MULHSU/MULHU` → returns 0
- `DIV/DIVU/REM/REMU` → returns 0 (immediate, no delay)
- **No illegal instruction exception**

### F Extension Off
- All FP operations → returns 0
- FPU always reports "ready"
- **No illegal instruction exception**

## Verification

```bash
# Check syntax
verilator --lint-only rtl/*.sv rtl/peripherals/*.sv

# Run tests (default config only)
cargo test --verbose
```

## Documentation

- **Full Guide:** [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md)
- **Testing:** [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md)
- **Summary:** [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md)

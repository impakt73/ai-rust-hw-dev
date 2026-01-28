# Extension Configuration Architecture

## Module Hierarchy and Parameter Flow

```
┌─────────────────────────────────────────────────────────────┐
│  top_with_peripherals                                        │
│  ┌──────────────────────────────────────────────┐           │
│  │  Parameters:                                  │           │
│  │    ENABLE_M_EXT = 1'b1  (default)            │           │
│  │    ENABLE_F_EXT = 1'b1  (default)            │           │
│  └─────────────────┬────────────────────────────┘           │
│                    │                                         │
│                    │ Pass parameters down                    │
│                    ▼                                         │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  top (CPU Core)                                      │    │
│  │  ┌──────────────────────────────────────────┐       │    │
│  │  │  Parameters:                              │       │    │
│  │  │    ENABLE_M_EXT = 1'b1                   │       │    │
│  │  │    ENABLE_F_EXT = 1'b1                   │       │    │
│  │  └────┬──────────────────────┬───────────────┘       │    │
│  │       │                      │                       │    │
│  │       │ Pass M param        │ Use F param           │    │
│  │       ▼                      ▼                       │    │
│  │  ┌─────────┐    ┌──────────────────────────┐        │    │
│  │  │   ALU   │    │  FP Units (Conditional)  │        │    │
│  │  │  param: │    │                          │        │    │
│  │  │ M_EXT   │    │  generate if (F_EXT):    │        │    │
│  │  │         │    │    - fp_regfile          │        │    │
│  │  │generate │    │    - fpu                 │        │    │
│  │  │ if(M):  │    │    - FCSR register       │        │    │
│  │  │-div_unit│    │    - FP operand regs     │        │    │
│  │  └─────────┘    └──────────────────────────┘        │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Resource Allocation by Configuration

### Full RV32IMFC (Default)
```
┌─────────────────────────────────────┐
│  CPU Core (~15,000+ LUTs)          │
│  ┌──────────┐  ┌──────────────┐   │
│  │   RV32I  │  │  M Extension  │   │
│  │  ~6,300  │  │  ~4,200 LUTs  │   │
│  │   LUTs   │  │  ┌─────────┐  │   │
│  │          │  │  │ div_unit │  │   │
│  │          │  │  └─────────┘  │   │
│  └──────────┘  └──────────────┘   │
│                                    │
│  ┌────────────────────────────┐   │
│  │    F Extension             │   │
│  │    ~4,500 LUTs             │   │
│  │  ┌──────────┐  ┌────────┐ │   │
│  │  │fp_regfile│  │  fpu   │ │   │
│  │  └──────────┘  └────────┘ │   │
│  └────────────────────────────┘   │
└─────────────────────────────────────┘
```

### RV32IM Configuration
```
┌─────────────────────────────────────┐
│  CPU Core (~10,500+ LUTs)          │
│  ┌──────────┐  ┌──────────────┐   │
│  │   RV32I  │  │  M Extension  │   │
│  │  ~6,300  │  │  ~4,200 LUTs  │   │
│  │   LUTs   │  │  ┌─────────┐  │   │
│  │          │  │  │ div_unit │  │   │
│  │          │  │  └─────────┘  │   │
│  └──────────┘  └──────────────┘   │
│                                    │
│  ┌────────────────────────────┐   │
│  │    F Extension: DISABLED   │   │
│  │    (Saved ~4,500 LUTs)     │   │
│  │    FP signals → 0          │   │
│  └────────────────────────────┘   │
└─────────────────────────────────────┘
```

### RV32I Minimal Configuration (iCE40-HX8K Target)
```
┌─────────────────────────────────────┐
│  CPU Core (~6,300 LUTs)            │
│  ┌──────────────────────────────┐  │
│  │   RV32I Base                 │  │
│  │   ~6,300 LUTs                │  │
│  │                              │  │
│  │   Fits in iCE40-HX8K         │  │
│  │   (7,680 LUTs available)     │  │
│  └──────────────────────────────┘  │
│                                    │
│  ┌────────────────────────────┐   │
│  │  M Extension: DISABLED     │   │
│  │  (Saved ~4,200 LUTs)       │   │
│  │  MUL/DIV ops → 0           │   │
│  └────────────────────────────┘   │
│                                    │
│  ┌────────────────────────────┐   │
│  │  F Extension: DISABLED     │   │
│  │  (Saved ~4,500 LUTs)       │   │
│  │  FP signals → 0            │   │
│  └────────────────────────────┘   │
└─────────────────────────────────────┘
```

## Conditional Compilation Flow

### M Extension Generation

```systemverilog
// In alu.sv
generate
    if (ENABLE_M_EXT) begin : gen_m_ext
        // ✓ Synthesize division unit
        div_unit #(.WIDTH(32)) u_div (...);
        
        // ✓ Full M extension support
        assign is_div_op = (alu_op == DIV) || ...;
    end else begin : gen_no_m_ext
        // ✗ No division unit (saves LUTs)
        assign div_result = 32'd0;
        assign div_ready = 1'b1;
        assign is_div_op = 1'b0;
    end
endgenerate
```

### F Extension Generation

```systemverilog
// In top.sv
generate
    if (ENABLE_F_EXT) begin : gen_f_ext
        // ✓ Synthesize FP hardware
        fp_regfile u_fp_regfile (...);
        fpu u_fpu (...);
        
        // ✓ FCSR register logic
        always_ff @(posedge clk) begin
            // Floating-point control/status
        end
    end else begin : gen_no_f_ext
        // ✗ No FP hardware (saves LUTs)
        assign fs1_data = 32'd0;
        assign fs2_data = 32'd0;
        assign fpu_fp_result = 32'd0;
        assign fpu_ready = 1'b1;
        assign fcsr = 32'd0;
    end
endgenerate
```

## Design Benefits

### ✅ True Conditional Compilation
- Disabled modules are **not synthesized** at all
- Zero LUT cost for disabled features
- No wasted FPGA resources

### ✅ Clean Synthesis
- No warnings about unused signals
- No dangling ports or unconnected modules
- Optimized place-and-route

### ✅ Backward Compatible
- Default parameters enable all extensions
- Existing code works without changes
- Gradual migration path

### ✅ Flexible Configuration
- Choose exact feature set needed
- Optimize for different FPGA targets
- Parameter-based (compile-time decision)

## Target Device Comparison

| FPGA Device | Available LUTs | Recommended Config | Fit? |
|-------------|----------------|-------------------|------|
| iCE40-HX1K | 1,280 | RV32I (minimal) | ❌ No |
| iCE40-HX4K | 3,520 | RV32I (minimal) | ❌ No |
| iCE40-HX8K | **7,680** | **RV32I (minimal)** | ✅ **Yes** |
| iCE40-UP5K | 5,280 | RV32I (minimal) | ❌ Tight |
| Artix-7 XC7A35T | 20,800 | RV32IMFC (full) | ✅ Yes |
| Cyclone IV EP4CE22 | 21,800 | RV32IMFC (full) | ✅ Yes |

**Key Insight:** The minimal RV32I configuration (~6,300 LUTs) enables this CPU to target the popular iCE40-HX8K FPGA, opening up low-cost FPGA deployment options.

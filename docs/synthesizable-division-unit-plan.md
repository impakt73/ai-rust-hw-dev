# Synthesizable Division Unit Implementation Plan

## Executive Summary

**Goal:** Replace the current non-synthesizable division and remainder operations (`/`, `%` operators) in the ALU with a **hardware-synthesizable non-restoring division algorithm** that executes in multiple clock cycles.

**Strategy:**
- Implement a dedicated division unit using the non-restoring division algorithm (32 iterations max)
- Extend ALU to support multi-cycle operations with variable timing using ready/valid handshaking
- Integrate with existing multi-cycle CPU FSM in `top.sv` using the same handshaking pattern as memory operations
- Maintain full compatibility with existing tests (no test changes required - tests already validate division/remainder behavior)

**Impact:** Division and remainder operations will take 33-35 cycles (start + 32 iterations + done) instead of combinational logic. All other ALU operations remain single-cycle. FPGA synthesis becomes possible.

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Non-Restoring Division Algorithm](#non-restoring-division-algorithm)
3. [Architecture Overview](#architecture-overview)
4. [Division Unit Design](#division-unit-design)
5. [ALU Modifications](#alu-modifications)
6. [Top-Level FSM Integration](#top-level-fsm-integration)
7. [RTL Implementation Details](#rtl-implementation-details)
8. [Timing Diagrams](#timing-diagrams)
9. [Testing Strategy](#testing-strategy)
10. [Implementation Phases](#implementation-phases)
11. [Performance Analysis](#performance-analysis)
12. [Risk Assessment](#risk-assessment)

---

## Problem Statement

### Current Implementation Issues

The current ALU implementation in `rtl/alu.sv` uses SystemVerilog division and remainder operators:

```systemverilog
ALU_DIV: begin
    if (b == 32'd0) begin
        result = 32'hFFFFFFFF;
    end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
        result = 32'h80000000;
    end else begin
        result = $signed(a) / $signed(b);  // ❌ NOT SYNTHESIZABLE
    end
end

ALU_REM: begin
    if (b == 32'd0) begin
        result = a;
    end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
        result = 32'd0;
    end else begin
        result = $signed(a) % $signed(b);  // ❌ NOT SYNTHESIZABLE
    end
end
```

**Problems:**
1. **Combinational division is NOT synthesizable** for FPGAs (most synthesis tools do not support `/` and `%`)
2. Even if synthesizable, combinational dividers have **extremely long critical paths** (>32 logic levels)
3. Dividers consume significant **area** when implemented combinationally
4. Current design assumes all ALU operations complete in 1 cycle

### Requirements for Solution

✅ **Must be FPGA-synthesizable** - Use only basic logic gates, adders, shifters, and registers

✅ **Must produce identical results** - Match RISC-V specification for DIV, DIVU, REM, REMU including edge cases

✅ **Must support signed and unsigned** - Handle both signed (DIV, REM) and unsigned (DIVU, REMU) operations

✅ **Must be multi-cycle** - Allow variable execution time (division takes longer than add/sub)

✅ **Must integrate with existing FSM** - Use same handshaking pattern as memory operations

✅ **Must pass existing tests** - All 16 ALU tests and 28+ CPU tests must continue to pass without modification

---

## Non-Restoring Division Algorithm

### Algorithm Overview

The **non-restoring division algorithm** is a classical hardware division method that computes quotient and remainder through iterative subtract-and-shift operations. It is **fully synthesizable** and commonly used in hardware implementations.

**Key Properties:**
- Requires N iterations for N-bit operands (32 iterations for 32-bit division)
- Uses only addition, subtraction, and shift operations (all synthesizable)
- Produces both quotient and remainder simultaneously
- Works for both signed and unsigned division (with sign handling wrapper)

### Mathematical Foundation

Given dividend `A` and divisor `B`, compute:
- **Quotient** `Q = A / B`
- **Remainder** `R = A % B`

The algorithm maintains an **invariant** at each iteration:
```
A = Q × B + R
```

### Algorithm Steps (Unsigned Version)

```
Inputs:
  A = 32-bit dividend (unsigned)
  B = 32-bit divisor (unsigned)

Outputs:
  Q = 32-bit quotient
  R = 32-bit remainder

Registers:
  P = 64-bit partial remainder (P_hi:P_lo)
  D = 32-bit divisor (shifted left by 32 bits → 64-bit aligned divisor)
  Q = 32-bit quotient accumulator
  i = iteration counter

Initialization:
  P[63:32] = 0              // Upper 32 bits
  P[31:0]  = A              // Lower 32 bits (dividend)
  D        = {B, 32'b0}     // Divisor in upper 32 bits
  Q        = 0
  i        = 0

Iteration (repeat 32 times):
  1. Shift P left by 1 bit: P = P << 1
  
  2. If P >= D (compare 64-bit values):
       P = P - D
       Q = (Q << 1) | 1    // Quotient bit = 1
     Else:
       Q = Q << 1          // Quotient bit = 0
  
  3. i = i + 1

Final Result:
  Quotient  = Q
  Remainder = P[63:32]  // Upper 32 bits after 32 shifts
```

### Non-Restoring Optimization

The classical algorithm above is "restoring" (it tests and conditionally subtracts). The **non-restoring** variant optimizes by:

1. **Always shifting and subtracting/adding** (no conditional skip)
2. **Tracking sign of partial remainder** to decide add vs. subtract
3. **Final correction step** to ensure positive remainder

**Non-Restoring Pseudo-Code:**

```
Initialization:
  P = {32'b0, A}    // 64-bit: {remainder, dividend}
  D = B << 32       // Divisor aligned to MSB
  Q = 0
  i = 0

Iteration (32 times):
  1. Shift P left by 1: P = P << 1
  
  2. If P >= 0:           // Partial remainder is positive
       P = P - D
       Q = (Q << 1) | 1
     Else:                // Partial remainder is negative
       P = P + D
       Q = Q << 1
  
  3. i = i + 1

Correction (if final P < 0):
  P = P + D
  Q = Q - 1

Final Result:
  Quotient  = Q
  Remainder = P >> 32
```

**Advantage:** Eliminates the comparison and conditional logic, making it faster in hardware.

### Signed Division Handling

For **signed division** (DIV, REM):

1. **Extract signs** of dividend and divisor
2. **Convert to absolute values** (unsigned)
3. **Run unsigned non-restoring division**
4. **Apply sign correction** to quotient and remainder

**Sign Rules (RISC-V Specification):**

```
Quotient Sign:  sign(A) XOR sign(B)
Remainder Sign: sign(A)

Examples:
  20 / 3   →  Q=6,  R=2
  -20 / 3  →  Q=-6, R=-2
  20 / -3  →  Q=-6, R=2
  -20 / -3 →  Q=6,  R=-2
```

### Hardware State Machine for Division Unit

```systemverilog
typedef enum logic [2:0] {
    DIV_IDLE     = 3'b000,  // Waiting for start
    DIV_INIT     = 3'b001,  // Initialize registers
    DIV_ITER     = 3'b010,  // Perform 32 iterations
    DIV_CORRECT  = 3'b011,  // Final correction if needed
    DIV_DONE     = 3'b100   // Result ready
} div_state_t;
```

**State Transitions:**

```
IDLE → (start=1) → INIT → ITER (32 cycles) → CORRECT → DONE → IDLE
```

---

## Architecture Overview

### System Block Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                         top.sv (CPU)                        │
│                                                             │
│  ┌─────────────┐      ┌──────────────────────────────┐    │
│  │   FSM       │      │          ALU                 │    │
│  │ (S_EXECUTE) │─────>│  ┌────────────────────────┐  │    │
│  │             │ start││  │ Combinational Logic  │  │    │
│  │             │<─────││  │ (ADD, SUB, AND, OR)  │  │    │
│  │  Wait for   │ ready││  └────────────────────────┘  │    │
│  │  alu_ready  │      │                               │    │
│  └─────────────┘      │  ┌────────────────────────┐  │    │
│                       │  │   Division Unit        │  │    │
│                       │  │  (Non-Restoring FSM)   │  │    │
│                       │  │   ┌───────────┐        │  │    │
│                       │  │   │  State    │        │  │    │
│                       │  │   │  Machine  │        │  │    │
│                       │  │   └───────────┘        │  │    │
│                       │  │   ┌───────────┐        │  │    │
│                       │  │   │ 64-bit P  │        │  │    │
│                       │  │   │ 64-bit D  │        │  │    │
│                       │  │   │ 32-bit Q  │        │  │    │
│                       │  │   └───────────┘        │  │    │
│                       │  └────────────────────────┘  │    │
│                       └──────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Signal Interface

**New ALU Ports:**

```systemverilog
module alu (
    input  logic        clk,          // NEW: Clock for division unit
    input  logic        rst_n,        // NEW: Reset for division unit
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    input  logic        alu_start,    // NEW: Start operation (pulse)
    output logic [31:0] result,
    output logic        zero,
    output logic        alu_ready     // NEW: Operation complete (stays high until new start)
);
```

**Handshaking Protocol:**

1. **Top.sv asserts `alu_start` for 1 cycle** in S_EXECUTE state
2. **ALU deasserts `alu_ready`** when division starts
3. **ALU performs multi-cycle division** (32-35 cycles)
4. **ALU asserts `alu_ready`** when division completes
5. **Top.sv waits in S_EXECUTE** until `alu_ready` is high, then proceeds to S_WRITEBACK

**For non-division operations:**
- `alu_ready` is immediately high (combinational result available in same cycle)
- No waiting required

### Integration with Existing FSM

The existing multi-cycle CPU FSM in `top.sv` already supports variable-latency operations via the memory handshaking protocol. We will use **the same pattern** for ALU operations:

**Current Memory Operation Pattern:**
```systemverilog
S_MEM_READ: begin
    dmem_req = 1'b1;
    if (dmem_ready) begin
        mdr_write = 1'b1;
        next_state = S_WRITEBACK;
    end else begin
        next_state = S_MEM_READ;  // Wait
    end
end
```

**New ALU Operation Pattern:**
```systemverilog
S_EXECUTE: begin
    alu_start = 1'b1;  // Start on entry
    if (alu_ready) begin
        alu_out_write = 1'b1;
        next_state = S_WRITEBACK;
    end else begin
        next_state = S_EXECUTE;  // Wait for multi-cycle ops
    end
end
```

**Key Insight:** This is a **minimal change** - we just add a wait loop to S_EXECUTE identical to the memory states.

---

## Division Unit Design

### Module Interface

```systemverilog
module div_unit (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control interface
    input  logic        start,        // Start division (pulse)
    input  logic        is_signed,    // 1=signed (DIV/REM), 0=unsigned (DIVU/REMU)
    input  logic        rem_sel,      // 1=remainder, 0=quotient
    
    // Data interface
    input  logic [31:0] dividend,     // A
    input  logic [31:0] divisor,      // B
    output logic [31:0] result,       // Quotient or Remainder
    output logic        ready         // Result valid
);
```

### Internal Registers

```systemverilog
// State machine
div_state_t state, next_state;

// Division working registers
logic [63:0] P;           // Partial remainder (64-bit)
logic [63:0] D;           // Divisor aligned (64-bit)
logic [31:0] Q;           // Quotient accumulator
logic [5:0]  iter_count;  // Iteration counter (0-32)

// Sign tracking
logic        dividend_neg;
logic        divisor_neg;
logic        quotient_neg;
logic        remainder_neg;

// Edge case flags
logic        div_by_zero;
logic        overflow;    // -2^31 / -1 case

// Intermediate values
logic [31:0] abs_dividend;
logic [31:0] abs_divisor;
```

### State Machine Implementation

```systemverilog
// State register
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        state <= DIV_IDLE;
    else
        state <= next_state;
end

// Next state logic
always_comb begin
    next_state = state;
    
    case (state)
        DIV_IDLE: begin
            if (start)
                next_state = DIV_INIT;
        end
        
        DIV_INIT: begin
            // Check for special cases
            if (div_by_zero || overflow)
                next_state = DIV_DONE;  // Skip iterations
            else
                next_state = DIV_ITER;
        end
        
        DIV_ITER: begin
            if (iter_count == 6'd32)
                next_state = DIV_CORRECT;
            else
                next_state = DIV_ITER;
        end
        
        DIV_CORRECT: begin
            next_state = DIV_DONE;
        end
        
        DIV_DONE: begin
            if (!start)  // Wait for start to deassert
                next_state = DIV_IDLE;
        end
        
        default: next_state = DIV_IDLE;
    endcase
end
```

### DIV_INIT State Logic

```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        P <= 64'h0;
        D <= 64'h0;
        Q <= 32'h0;
        iter_count <= 6'd0;
        dividend_neg <= 1'b0;
        divisor_neg <= 1'b0;
        div_by_zero <= 1'b0;
        overflow <= 1'b0;
    end else if (state == DIV_INIT) begin
        // Check for division by zero
        div_by_zero <= (divisor == 32'd0);
        
        // Check for overflow (-2^31 / -1)
        overflow <= is_signed && 
                    (dividend == 32'h80000000) && 
                    (divisor == 32'hFFFFFFFF);
        
        if (!div_by_zero && !overflow) begin
            // Handle sign conversion for signed division
            if (is_signed) begin
                dividend_neg <= dividend[31];
                divisor_neg <= divisor[31];
                
                // Convert to absolute values
                abs_dividend = dividend_neg ? (~dividend + 32'd1) : dividend;
                abs_divisor  = divisor_neg  ? (~divisor  + 32'd1) : divisor;
            end else begin
                dividend_neg <= 1'b0;
                divisor_neg <= 1'b0;
                abs_dividend = dividend;
                abs_divisor  = divisor;
            end
            
            // Initialize division registers
            P <= {32'h0, abs_dividend};  // {remainder, dividend}
            D <= {abs_divisor, 32'h0};   // Divisor in upper 32 bits
            Q <= 32'h0;
            iter_count <= 6'd0;
        end
    end else if (state == DIV_ITER) begin
        // Perform one iteration
        logic [63:0] P_shifted;
        logic P_sign;
        
        P_shifted = P << 1;  // Shift left by 1
        
        P_sign = P_shifted[63];  // Check sign of shifted partial remainder
        
        if (!P_sign) begin
            // Positive: subtract divisor, set quotient bit
            P <= P_shifted - D;
            Q <= {Q[30:0], 1'b1};
        end else begin
            // Negative: add divisor, clear quotient bit
            P <= P_shifted + D;
            Q <= {Q[30:0], 1'b0};
        end
        
        iter_count <= iter_count + 6'd1;
    end
end
```

### DIV_CORRECT State Logic

```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        // Reset handled in INIT
    end else if (state == DIV_CORRECT) begin
        // If final P is negative, correction needed
        if (P[63]) begin
            P <= P + D;
            Q <= Q - 32'd1;
        end
        
        // Apply sign corrections for signed division
        if (is_signed) begin
            quotient_neg <= dividend_neg ^ divisor_neg;
            remainder_neg <= dividend_neg;
            
            if (quotient_neg)
                Q <= ~Q + 32'd1;  // Negate quotient
            
            if (remainder_neg)
                P <= ~P + 64'd1;  // Negate remainder
        end
    end
end
```

### DIV_DONE State and Output Logic

```systemverilog
// Output logic
always_comb begin
    ready = (state == DIV_DONE);
    
    if (state == DIV_DONE) begin
        if (div_by_zero) begin
            // Division by zero handling (RISC-V spec)
            if (rem_sel)
                result = dividend;  // REM/REMU: return dividend
            else
                result = 32'hFFFFFFFF;  // DIV/DIVU: return all 1's
        end else if (overflow) begin
            // Overflow handling: -2^31 / -1 (RISC-V spec)
            if (rem_sel)
                result = 32'd0;  // REM: return 0
            else
                result = 32'h80000000;  // DIV: return -2^31
        end else begin
            if (rem_sel)
                result = P[63:32];  // Remainder is upper 32 bits
            else
                result = Q;  // Quotient
        end
    end else begin
        result = 32'h0;  // Default
    end
end
```

---

## ALU Modifications

### Updated ALU Module

```systemverilog
module alu (
    input  logic        clk,          // NEW
    input  logic        rst_n,        // NEW
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    input  logic        alu_start,    // NEW
    output logic [31:0] result,
    output logic        zero,
    output logic        alu_ready     // NEW
);

    // ... existing ALU operation constants ...
    
    // Division unit signals
    logic        div_start;
    logic        div_is_signed;
    logic        div_rem_sel;
    logic [31:0] div_result;
    logic        div_ready;
    
    // Instantiate division unit
    div_unit u_div (
        .clk(clk),
        .rst_n(rst_n),
        .start(div_start),
        .is_signed(div_is_signed),
        .rem_sel(div_rem_sel),
        .dividend(a),
        .divisor(b),
        .result(div_result),
        .ready(div_ready)
    );
    
    // Detect division operations
    logic is_div_op;
    assign is_div_op = (alu_op == ALU_DIV)  || 
                       (alu_op == ALU_DIVU) || 
                       (alu_op == ALU_REM)  || 
                       (alu_op == ALU_REMU);
    
    // Start division when requested
    assign div_start = alu_start && is_div_op;
    
    // Configure division unit
    always_comb begin
        case (alu_op)
            ALU_DIV: begin
                div_is_signed = 1'b1;
                div_rem_sel = 1'b0;  // Quotient
            end
            ALU_DIVU: begin
                div_is_signed = 1'b0;
                div_rem_sel = 1'b0;  // Quotient
            end
            ALU_REM: begin
                div_is_signed = 1'b1;
                div_rem_sel = 1'b1;  // Remainder
            end
            ALU_REMU: begin
                div_is_signed = 1'b0;
                div_rem_sel = 1'b1;  // Remainder
            end
            default: begin
                div_is_signed = 1'b0;
                div_rem_sel = 1'b0;
            end
        endcase
    end
    
    // ALU ready signal
    assign alu_ready = is_div_op ? div_ready : 1'b1;  // Immediate for non-div ops
    
    // Multiplication intermediate results (64-bit)
    logic [63:0] mul_result;
    logic signed [63:0] mulhsu_a_ext;
    logic [63:0] mulhsu_b_ext;

    // Result multiplexer
    always_comb begin
        // Default initialization to avoid latches
        mul_result = 64'd0;
        result = 32'd0;
        mulhsu_a_ext = 64'sd0;
        mulhsu_b_ext = 64'd0;
        
        case (alu_op)
            // RV32I operations (combinational - single cycle)
            ALU_ADD:  result = a + b;
            ALU_SUB:  result = a - b;
            ALU_AND:  result = a & b;
            ALU_OR:   result = a | b;
            ALU_XOR:  result = a ^ b;
            ALU_SLL:  result = a << b[4:0];
            ALU_SRL:  result = a >> b[4:0];
            ALU_SRA:  result = $signed(a) >>> b[4:0];
            ALU_SLT:  result = ($signed(a) < $signed(b)) ? 32'd1 : 32'd0;
            ALU_SLTU: result = (a < b) ? 32'd1 : 32'd0;
            
            // M Extension - Multiplication operations (combinational - single cycle)
            ALU_MUL: begin
                mul_result = $signed(a) * $signed(b);
                result = mul_result[31:0];
            end
            ALU_MULH: begin
                mul_result = $signed(a) * $signed(b);
                result = mul_result[63:32];
            end
            ALU_MULHSU: begin
                mulhsu_a_ext = {{32{a[31]}}, a};
                mulhsu_b_ext = {32'b0, b};
                mul_result = $signed(mulhsu_a_ext) * $signed(mulhsu_b_ext);
                result = mul_result[63:32];
            end
            ALU_MULHU: begin
                mul_result = $unsigned(a) * $unsigned(b);
                result = mul_result[63:32];
            end
            
            // M Extension - Division operations (multi-cycle via division unit)
            ALU_DIV,
            ALU_DIVU,
            ALU_REM,
            ALU_REMU: begin
                result = div_result;  // Comes from division unit
            end
            
            default: result = 32'd0;
        endcase
    end

    assign zero = (result == 32'd0);

endmodule
```

**Key Changes:**
1. Added `clk`, `rst_n`, `alu_start`, `alu_ready` ports
2. Instantiated `div_unit` module
3. Removed combinational division logic (`/`, `%` operators)
4. Added multiplexing to select division unit output for DIV/DIVU/REM/REMU
5. `alu_ready` is high immediately for all ops except division (which waits for `div_ready`)

---

## Top-Level FSM Integration

### Modified S_EXECUTE State

**Current Implementation (from multi-cycle-cpu-implementation-plan.md):**

```systemverilog
S_EXECUTE: begin
    alu_out_write = 1'b1;
    next_state = S_WRITEBACK;
end
```

**New Implementation (with ALU handshaking):**

```systemverilog
// Control signal declarations (add these to top.sv)
logic alu_start;

// In FSM output logic:
always_comb begin
    // ... other defaults ...
    alu_start = 1'b0;
    
    case (current_state)
        // ... other states ...
        
        S_EXECUTE: begin
            alu_start = 1'b1;  // Start ALU operation
            
            if (alu_ready) begin
                alu_out_write = 1'b1;
                next_state = S_WRITEBACK;
            end else begin
                next_state = S_EXECUTE;  // Wait for multi-cycle division
            end
        end
        
        // ... other states ...
    endcase
end
```

**Explanation:**
- On first cycle in S_EXECUTE, `alu_start` pulses high for 1 cycle
- For combinational ops (ADD, SUB, MUL, etc.), `alu_ready` is immediately high → proceeds to S_WRITEBACK
- For division ops, `alu_ready` is low → FSM stays in S_EXECUTE until division completes
- When division finishes, `alu_ready` goes high → captures result and proceeds to S_WRITEBACK

### ALU Instantiation Update

**Current instantiation:**

```systemverilog
alu u_alu (
    .a(alu_a),
    .b(alu_b),
    .alu_op(alu_op_reg),
    .result(alu_result),
    .zero(alu_zero)
);
```

**New instantiation:**

```systemverilog
alu u_alu (
    .clk(clk),              // NEW
    .rst_n(rst_n),          // NEW
    .a(alu_a),
    .b(alu_b),
    .alu_op(alu_op_reg),
    .alu_start(alu_start),  // NEW
    .result(alu_result),
    .zero(alu_zero),
    .alu_ready(alu_ready)   // NEW
);
```

### Signal Declarations

Add to `rtl/top.sv`:

```systemverilog
// ALU control signals
logic alu_start;
logic alu_ready;
```

---

## RTL Implementation Details

### File Structure

**New Files:**
- `rtl/div_unit.sv` - Division unit module (new)

**Modified Files:**
- `rtl/alu.sv` - Add clock, reset, handshaking, instantiate div_unit
- `rtl/top.sv` - Modify S_EXECUTE state for ALU handshaking

**Unchanged Files:**
- `rtl/decoder.sv` - No changes (already decodes DIV/DIVU/REM/REMU)
- `rtl/regfile.sv` - No changes
- `rtl/branch_unit.sv` - No changes
- `rtl/mem_interface.sv` - No changes
- `rtl/csr_file.sv` - No changes
- `rtl/writeback_mux.sv` - No changes

### Complete Division Unit Code

See [Division Unit Design](#division-unit-design) section above for detailed implementation.

**Full module with all states:**

```systemverilog
// rtl/div_unit.sv
// Hardware Division Unit using Non-Restoring Algorithm
// Implements 32-bit signed and unsigned division and remainder

module div_unit (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control interface
    input  logic        start,        // Start division (pulse)
    input  logic        is_signed,    // 1=signed, 0=unsigned
    input  logic        rem_sel,      // 1=remainder, 0=quotient
    
    // Data interface
    input  logic [31:0] dividend,     // Dividend (A)
    input  logic [31:0] divisor,      // Divisor (B)
    output logic [31:0] result,       // Quotient or Remainder
    output logic        ready         // Result valid
);

    // State machine
    typedef enum logic [2:0] {
        DIV_IDLE     = 3'b000,
        DIV_INIT     = 3'b001,
        DIV_ITER     = 3'b010,
        DIV_CORRECT  = 3'b011,
        DIV_DONE     = 3'b100
    } div_state_t;
    
    div_state_t state, next_state;
    
    // Division working registers
    logic [63:0] P;           // Partial remainder
    logic [63:0] D;           // Divisor aligned
    logic [31:0] Q;           // Quotient
    logic [5:0]  iter_count;  // 0-32
    
    // Sign tracking
    logic        dividend_neg;
    logic        divisor_neg;
    
    // Special cases
    logic        div_by_zero;
    logic        overflow;
    
    // Intermediate values
    logic [31:0] abs_dividend;
    logic [31:0] abs_divisor;
    logic [31:0] final_quotient;
    logic [31:0] final_remainder;
    
    // State register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            state <= DIV_IDLE;
        else
            state <= next_state;
    end
    
    // Next state logic
    always_comb begin
        next_state = state;
        
        case (state)
            DIV_IDLE: begin
                if (start)
                    next_state = DIV_INIT;
            end
            
            DIV_INIT: begin
                if (div_by_zero || overflow)
                    next_state = DIV_DONE;
                else
                    next_state = DIV_ITER;
            end
            
            DIV_ITER: begin
                if (iter_count == 6'd31)  // After 32 iterations (0-31)
                    next_state = DIV_CORRECT;
                else
                    next_state = DIV_ITER;
            end
            
            DIV_CORRECT: begin
                next_state = DIV_DONE;
            end
            
            DIV_DONE: begin
                if (!start)
                    next_state = DIV_IDLE;
            end
            
            default: next_state = DIV_IDLE;
        endcase
    end
    
    // Datapath registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            P <= 64'h0;
            D <= 64'h0;
            Q <= 32'h0;
            iter_count <= 6'd0;
            dividend_neg <= 1'b0;
            divisor_neg <= 1'b0;
            div_by_zero <= 1'b0;
            overflow <= 1'b0;
        end else begin
            case (state)
                DIV_INIT: begin
                    // Check special cases
                    div_by_zero <= (divisor == 32'd0);
                    overflow <= is_signed && 
                                (dividend == 32'h80000000) && 
                                (divisor == 32'hFFFFFFFF);
                    
                    if (divisor != 32'd0) begin
                        // Handle sign conversion
                        if (is_signed) begin
                            dividend_neg <= dividend[31];
                            divisor_neg <= divisor[31];
                            abs_dividend = dividend[31] ? (~dividend + 32'd1) : dividend;
                            abs_divisor  = divisor[31]  ? (~divisor  + 32'd1) : divisor;
                        end else begin
                            dividend_neg <= 1'b0;
                            divisor_neg <= 1'b0;
                            abs_dividend = dividend;
                            abs_divisor  = divisor;
                        end
                        
                        // Initialize
                        P <= {32'h0, abs_dividend};
                        D <= {abs_divisor, 32'h0};
                        Q <= 32'h0;
                        iter_count <= 6'd0;
                    end
                end
                
                DIV_ITER: begin
                    // Non-restoring division iteration
                    logic [63:0] P_shifted;
                    
                    P_shifted = P << 1;
                    
                    if (!P_shifted[63]) begin
                        // Positive: subtract
                        P <= P_shifted - D;
                        Q <= {Q[30:0], 1'b1};
                    end else begin
                        // Negative: add
                        P <= P_shifted + D;
                        Q <= {Q[30:0], 1'b0};
                    end
                    
                    iter_count <= iter_count + 6'd1;
                end
                
                DIV_CORRECT: begin
                    // Final correction if P is negative
                    if (P[63]) begin
                        P <= P + D;
                        Q <= Q - 32'd1;
                    end
                end
                
                default: begin
                    // Hold values
                end
            endcase
        end
    end
    
    // Sign correction and output
    always_comb begin
        // Apply signs to quotient and remainder
        if (is_signed && !div_by_zero && !overflow) begin
            // Quotient sign: dividend_sign XOR divisor_sign
            if (dividend_neg ^ divisor_neg)
                final_quotient = ~Q + 32'd1;
            else
                final_quotient = Q;
            
            // Remainder sign: same as dividend
            if (dividend_neg)
                final_remainder = ~P[63:32] + 32'd1;
            else
                final_remainder = P[63:32];
        end else begin
            final_quotient = Q;
            final_remainder = P[63:32];
        end
    end
    
    // Output logic
    always_comb begin
        ready = (state == DIV_DONE);
        
        if (state == DIV_DONE) begin
            if (div_by_zero) begin
                // RISC-V spec: division by zero
                if (rem_sel)
                    result = dividend;
                else
                    result = 32'hFFFFFFFF;
            end else if (overflow) begin
                // RISC-V spec: -2^31 / -1 overflow
                if (rem_sel)
                    result = 32'd0;
                else
                    result = 32'h80000000;
            end else begin
                if (rem_sel)
                    result = final_remainder;
                else
                    result = final_quotient;
            end
        end else begin
            result = 32'h0;
        end
    end

endmodule
```

---

## Timing Diagrams

### Combinational Operation (ADD)

```
Clock:     ___/‾‾‾\___/‾‾‾\___/‾‾‾\___
State:       S_EXECUTE  S_WBACK   S_FETCH
alu_start: ___/‾\_______________
alu_ready: ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾
result:    ────<VALID>────────────

Total: 1 cycle in S_EXECUTE
```

### Division Operation (DIV)

```
Clock:     ___/‾‾‾\___/‾‾‾\___/‾‾‾\___/‾‾‾\___  (... 32 more cycles) ...  ___/‾‾‾\___/‾‾‾\___
State:       S_EXECUTE  S_EXECUTE  S_EXECUTE  S_EXECUTE  ...                S_EXECUTE  S_WBACK
alu_start: ___/‾\________________________________________________________________________________________
alu_ready: _______________________________________________________________________________/‾‾‾‾‾‾‾‾‾‾‾‾
div_state:     IDLE       INIT       ITER       ITER       ... (32 total)  CORRECT    DONE      IDLE
result:    ─────────────────────────────────────────────────────────────────────────────<VALID>─────

Total: 35 cycles in S_EXECUTE (1 cycle start + 1 cycle init + 32 iterations + 1 correction + hold in DONE until S_WRITEBACK)
```

### Complete Instruction Timing

**ADD Instruction (Combinational):**
```
Cycle 1: S_FETCH (wait for imem_ready)
Cycle 2: S_DECODE
Cycle 3: S_EXECUTE (alu_ready=1 immediately)
Cycle 4: S_WRITEBACK
Cycle 5: S_FETCH (next instruction)

Total: 4 cycles minimum (+ memory latency)
```

**DIV Instruction (Multi-Cycle):**
```
Cycle 1:    S_FETCH (wait for imem_ready)
Cycle 2:    S_DECODE
Cycle 3:    S_EXECUTE (start division, alu_ready=0)
Cycle 4:    S_EXECUTE (division in progress)
...
Cycle 37:   S_EXECUTE (division complete, alu_ready=1)
Cycle 38:   S_WRITEBACK
Cycle 39:   S_FETCH (next instruction)

Total: 37 cycles minimum (+ memory latency)
```

---

## Testing Strategy

### Existing Tests (No Changes Required)

**Good News:** All existing tests should pass without modification!

**Why?**
1. Tests already validate correct division behavior (quotient, remainder, edge cases)
2. Tests use the existing `execute_instruction!` macro which waits for `instr_complete`
3. `instr_complete` only asserts when instruction finishes (regardless of cycle count)
4. Division unit produces identical results to current `/` and `%` operators

**Existing ALU Tests (16 tests in `tests/src/alu_test.rs`):**
- `test_alu_add()`, `test_alu_sub()`, etc. - All pass (combinational ops unchanged)
- `test_alu_mul()`, `test_alu_mulh()`, etc. - All pass (multiplication unchanged)
- **`test_alu_div()`** - Will pass with new division unit (produces same results)
- **`test_alu_divu()`** - Will pass
- **`test_alu_rem()`** - Will pass
- **`test_alu_remu()`** - Will pass

**Existing CPU Tests (28+ tests in `tests/src/cpu_test.rs`):**
- All tests use `execute_instruction!` macro which handles multi-cycle execution
- Division edge cases already tested: division by zero, overflow (-2^31 / -1)
- Tests will take slightly longer to run (35 cycles vs 1 cycle for division)

### New Tests (Optional - For Verification)

While not strictly required, these tests can validate the division unit in isolation:

**Unit Tests for Division Unit:**

```rust
// tests/src/div_unit_test.rs (new file)

#[test]
fn test_div_unit_unsigned_basic() {
    let mut dut = marlin::get_dut::<div_unit::DUT>("div_unit");
    dut.reset();
    
    // Test: 20 / 3 = 6 remainder 2
    dut.set_dividend(20);
    dut.set_divisor(3);
    dut.set_is_signed(0);
    dut.set_rem_sel(0);  // Quotient
    dut.set_start(1);
    dut.eval();
    clock_cycle!(dut);
    dut.set_start(0);
    
    // Wait for ready
    for _ in 0..100 {
        dut.eval();
        if dut.ready() == 1 {
            break;
        }
        clock_cycle!(dut);
    }
    
    assert_eq!(dut.ready(), 1);
    assert_eq!(dut.result(), 6);
}

#[test]
fn test_div_unit_unsigned_remainder() {
    // Similar test with rem_sel=1
    // Expected: 20 % 3 = 2
}

#[test]
fn test_div_unit_signed() {
    // Test -20 / 3 = -6
}

#[test]
fn test_div_unit_div_by_zero() {
    // Test division by zero handling
}

#[test]
fn test_div_unit_overflow() {
    // Test -2^31 / -1 overflow case
}
```

**Cycle Count Verification:**

```rust
#[test]
fn test_division_cycle_count() {
    // Verify division takes expected number of cycles (32-35)
    let mut cycle_count = 0;
    // ... (similar to test_div_unit_unsigned_basic but count cycles)
    assert!(cycle_count >= 32 && cycle_count <= 36);
}
```

### Verification Checklist

**Functional Verification:**
- [ ] All existing ALU tests pass (16 tests)
- [ ] All existing CPU tests pass (28+ tests)
- [ ] Division produces correct quotient (DIV, DIVU)
- [ ] Division produces correct remainder (REM, REMU)
- [ ] Division by zero handled correctly
- [ ] Overflow case (-2^31 / -1) handled correctly
- [ ] Signed division works (positive/negative combinations)
- [ ] Unsigned division works

**Timing Verification:**
- [ ] Combinational ops complete in 1 cycle (alu_ready=1 immediately)
- [ ] Division ops complete in 33-36 cycles
- [ ] FSM correctly waits in S_EXECUTE for division
- [ ] No deadlocks or infinite loops

**Code Quality:**
- [ ] Verilator lint passes: `verilator --lint-only rtl/*.sv`
- [ ] No latches inferred (division unit uses flip-flops only)
- [ ] Rust tests compile: `cargo test --no-run`
- [ ] Clippy passes: `cargo clippy -- -D warnings`

---

## Implementation Phases

### Phase 1: Division Unit Module (2-3 days)

**Tasks:**
1. [ ] Create `rtl/div_unit.sv` with complete implementation
   - State machine (5 states)
   - Datapath registers (P, D, Q, iter_count)
   - Sign handling logic
   - Edge case detection (div by zero, overflow)
   - Output multiplexing

2. [ ] Verify module in isolation
   ```bash
   verilator --lint-only rtl/div_unit.sv
   ```

3. [ ] Write basic unit tests for division unit
   - Test unsigned division
   - Test signed division
   - Test edge cases

**Validation:**
- Module compiles without errors
- Lint passes with no warnings
- No latches inferred

### Phase 2: ALU Integration (1-2 days)

**Tasks:**
1. [ ] Modify `rtl/alu.sv`
   - Add `clk`, `rst_n`, `alu_start`, `alu_ready` ports
   - Instantiate `div_unit` module
   - Remove combinational division logic (delete `/`, `%` operators)
   - Add result multiplexing
   - Implement `alu_ready` logic

2. [ ] Update module interface in `rtl/top.sv`
   - Add `alu_start` and `alu_ready` signals to ALU instantiation
   - Connect clock and reset

3. [ ] Verify ALU compilation
   ```bash
   verilator --lint-only rtl/alu.sv
   ```

**Validation:**
- ALU compiles without errors
- Lint passes
- No synthesis warnings

### Phase 3: Top-Level FSM Modification (1 day)

**Tasks:**
1. [ ] Modify `rtl/top.sv` S_EXECUTE state
   - Add `alu_start` signal generation
   - Add wait loop on `alu_ready`
   - Update next-state logic

2. [ ] Add signal declarations
   - Declare `alu_start` and `alu_ready` in top.sv

3. [ ] Verify complete CPU compilation
   ```bash
   verilator --lint-only rtl/*.sv
   ```

**Validation:**
- Full CPU compiles
- No lint errors
- Marlin builds successfully: `cargo build`

### Phase 4: Testing and Validation (2-3 days)

**Tasks:**
1. [ ] Run existing ALU tests
   ```bash
   cargo test --package cpu_verifier -- alu_test
   ```

2. [ ] Run existing CPU tests
   ```bash
   cargo test --package cpu_verifier -- cpu_test
   ```

3. [ ] Run full test suite
   ```bash
   cargo test --verbose
   ```

4. [ ] Add optional division unit unit tests
   ```bash
   cargo test --package cpu_verifier -- div_unit_test
   ```

5. [ ] Debug any failures
   - Use waveform dumps (VCD) for debugging
   - Check state machine transitions
   - Verify handshaking signals

**Validation:**
- All 112+ existing tests pass
- No regressions
- Division cycle count in expected range (32-36 cycles)

### Phase 5: Code Quality and Documentation (1 day)

**Tasks:**
1. [ ] Run code quality checks
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   verilator --lint-only rtl/*.sv
   ```

2. [ ] Add inline comments to division unit
   - Explain algorithm steps
   - Document state transitions
   - Clarify sign handling

3. [ ] Update documentation
   - Update `AGENTS.md` with multi-cycle ALU info
   - Update `README.md` if needed
   - Add comments to this plan document

**Validation:**
- All code quality checks pass
- Documentation is clear and accurate

---

## Performance Analysis

### Cycle Counts

**RV32I Operations (Unchanged):**
| Operation | Cycles | Notes |
|-----------|--------|-------|
| ADD, SUB, AND, OR, XOR | 1 | Combinational |
| SLL, SRL, SRA | 1 | Combinational |
| SLT, SLTU | 1 | Combinational |

**RV32M Multiplication (Unchanged):**
| Operation | Cycles | Notes |
|-----------|--------|-------|
| MUL, MULH, MULHSU, MULHU | 1 | Combinational (uses `*` operator) |

**RV32M Division (NEW - Multi-Cycle):**
| Operation | Cycles | Notes |
|-----------|--------|-------|
| DIV, DIVU | 33-36 | Non-restoring algorithm (32 iterations) |
| REM, REMU | 33-36 | Same algorithm, different output |

**Breakdown:**
- 1 cycle: DIV_IDLE → DIV_INIT (start pulse)
- 1 cycle: DIV_INIT (sign conversion, setup)
- 32 cycles: DIV_ITER (32 iterations)
- 1 cycle: DIV_CORRECT (final correction)
- 1 cycle: DIV_DONE (output valid)
- **Total: 36 cycles** (stays in DONE until next start)

**Edge Cases (Fast Path):**
- Division by zero: 3 cycles (IDLE → INIT → DONE)
- Overflow (-2^31 / -1): 3 cycles (IDLE → INIT → DONE)

### Instruction Execution Time

**Complete Instruction Timing (Including FSM Overhead):**

| Instruction Type | Min Cycles | Breakdown |
|------------------|------------|-----------|
| R-type ADD | 4 + mem | FETCH + DECODE + EXECUTE(1) + WRITEBACK |
| R-type MUL | 4 + mem | FETCH + DECODE + EXECUTE(1) + WRITEBACK |
| R-type DIV | 37 + mem | FETCH + DECODE + EXECUTE(33) + WRITEBACK |
| Load | 5 + 2×mem | FETCH + DECODE + MEM_ADDR + MEM_READ + WRITEBACK |
| Store | 4 + 2×mem | FETCH + DECODE + MEM_ADDR + MEM_WRITE |

**"+ mem"** indicates waiting for memory ready signal (variable latency)

### Performance Impact

**Best Case (All Adds):**
- Before: 4 cycles/instruction
- After: 4 cycles/instruction
- **Impact: 0%** (no change)

**Worst Case (All Divisions):**
- Before: 4 cycles/instruction (broken - not synthesizable)
- After: 37 cycles/instruction
- **Impact: 9.25× slower** (but now synthesizable!)

**Realistic Workload (Estimate: 95% non-div, 5% div):**
- Average cycles = 0.95 × 4 + 0.05 × 37 = 3.8 + 1.85 = 5.65 cycles/instruction
- **Impact: ~41% slower overall** (acceptable for synthesizability)

**Note:** These are rough estimates. Actual impact depends on workload. Division is typically rare in embedded code.

### Area Impact

**FPGA Resource Estimates (Conservative):**

| Component | LUTs | FFs | Notes |
|-----------|------|-----|-------|
| Division Unit | ~800 | ~200 | State machine + 64-bit datapath |
| ALU (before) | ~500 | 0 | Combinational only |
| ALU (after) | ~500 | 0 | Combinational part unchanged |
| **Total Increase** | **+800** | **+200** | ~16% of small FPGA |

**Notes:**
- Estimates based on typical Xilinx Artix-7 or Intel Cyclone V synthesis
- Actual area depends on synthesis tool optimizations
- Most FPGAs have 50k-200k LUTs, so this is a small fraction

---

## Risk Assessment

### High-Risk Areas

#### 1. Sign Handling Bugs

**Risk:** Incorrect sign conversion for signed division may produce wrong results.

**Mitigation:**
- Follow RISC-V spec exactly for sign rules
- Test all combinations: +/+, +/-, -/+, -/-
- Use existing comprehensive tests (already cover edge cases)
- Reference implementation: compare with QEMU/Spike

**Impact:** High (functional correctness)  
**Likelihood:** Medium (sign logic is tricky)

#### 2. FSM Deadlocks

**Risk:** Division unit FSM gets stuck in a state, hanging the CPU.

**Mitigation:**
- Careful state machine design (all states have exit paths)
- Add timeout counter in testbench (MAX_CYCLES_PER_INSTR)
- Thorough testing of all states
- Ensure `ready` signal always asserts eventually

**Impact:** High (system hang)  
**Likelihood:** Low (simple FSM)

#### 3. Handshaking Protocol Bugs

**Risk:** Mismatch between `alu_start` and `alu_ready` timing causes incorrect operation.

**Mitigation:**
- Follow same pattern as proven memory handshaking
- Clear protocol documentation
- Test with waveform inspection
- Verify `alu_start` is pulsed for exactly 1 cycle

**Impact:** High (functional failure)  
**Likelihood:** Low (proven pattern)

### Medium-Risk Areas

#### 4. Performance Regression

**Risk:** Multi-cycle division slows down workloads significantly.

**Mitigation:**
- Accept that division is slower (necessary for synthesis)
- Document performance characteristics
- Consider future optimization (early termination for small divisors)
- Division is rare in typical embedded code

**Impact:** Medium (performance)  
**Likelihood:** High (expected behavior)

#### 5. Edge Case Handling

**Risk:** Special cases (div by zero, overflow) not handled correctly.

**Mitigation:**
- Implement exact RISC-V spec behavior
- Existing tests already validate these cases
- Fast-path through FSM for edge cases (3 cycles instead of 36)

**Impact:** Medium (functional)  
**Likelihood:** Low (well-tested)

### Low-Risk Areas

#### 6. Synthesis Tool Compatibility

**Risk:** Some FPGA synthesis tools may not handle the design correctly.

**Mitigation:**
- Use only standard synthesizable constructs
- Avoid latches (use `always_ff` only)
- Test with Verilator (strict synthesizability checking)
- No vendor-specific primitives

**Impact:** Medium (portability)  
**Likelihood:** Very Low (conservative design)

---

## Appendix A: RISC-V Division Specification

### DIV (Signed Division)

**Encoding:** `funct7=0000001`, `funct3=100`

**Behavior:**
```c
int32_t quotient;
if (rs2 == 0) {
    quotient = -1;  // 0xFFFFFFFF
} else if (rs1 == 0x80000000 && rs2 == 0xFFFFFFFF) {
    quotient = 0x80000000;  // Overflow: -2^31 / -1
} else {
    quotient = (int32_t)rs1 / (int32_t)rs2;
}
rd = quotient;
```

### DIVU (Unsigned Division)

**Encoding:** `funct7=0000001`, `funct3=101`

**Behavior:**
```c
uint32_t quotient;
if (rs2 == 0) {
    quotient = 0xFFFFFFFF;
} else {
    quotient = (uint32_t)rs1 / (uint32_t)rs2;
}
rd = quotient;
```

### REM (Signed Remainder)

**Encoding:** `funct7=0000001`, `funct3=110`

**Behavior:**
```c
int32_t remainder;
if (rs2 == 0) {
    remainder = rs1;  // Return dividend unchanged
} else if (rs1 == 0x80000000 && rs2 == 0xFFFFFFFF) {
    remainder = 0;  // Overflow: -2^31 % -1
} else {
    remainder = (int32_t)rs1 % (int32_t)rs2;
}
rd = remainder;
```

### REMU (Unsigned Remainder)

**Encoding:** `funct7=0000001`, `funct3=111`

**Behavior:**
```c
uint32_t remainder;
if (rs2 == 0) {
    remainder = rs1;  // Return dividend unchanged
} else {
    remainder = (uint32_t)rs1 % (uint32_t)rs2;
}
rd = remainder;
```

---

## Appendix B: Alternative Algorithms (Future Work)

### Early Termination Optimization

**Idea:** Detect when quotient is known early (e.g., dividend < divisor) and skip remaining iterations.

**Benefit:** Reduces average division latency from 33 cycles to ~10-20 cycles for typical values.

**Complexity:** Moderate (requires leading zero detection and variable iteration count).

**Recommendation:** Implement in future iteration if performance is critical.

### SRT Division

**Idea:** Use radix-4 or higher SRT division to reduce iterations (16 iterations instead of 32).

**Benefit:** 2× faster division.

**Complexity:** High (requires quotient digit selection logic, lookup tables).

**Recommendation:** Only if targeting high-performance FPGAs.

### Pipelined Division

**Idea:** Allow new division to start before previous one finishes (multi-issue).

**Benefit:** Higher throughput for division-heavy code.

**Complexity:** Very high (requires pipeline registers, hazard detection).

**Recommendation:** Not applicable for single-cycle CPU architecture.

---

## Appendix C: Quick Command Reference

```bash
# Verify RTL (division unit in isolation)
verilator --lint-only rtl/div_unit.sv

# Verify RTL (complete system)
verilator --lint-only rtl/*.sv

# Build
cargo build --verbose

# Run ALU tests only
cargo test --package cpu_verifier -- alu_test

# Run CPU tests only
cargo test --package cpu_verifier -- cpu_test

# Run all tests
cargo test --verbose

# Run specific test with output
cargo test --package cpu_verifier test_alu_div -- --nocapture

# Code quality
cargo fmt -- --check
cargo clippy -- -D warnings

# Generate waveform for debugging
cargo test test_alu_div -- --nocapture  # Check for VCD output path
gtkwave /path/to/dump.vcd  # View waveforms
```

---

## Appendix D: Resources

### RISC-V Specifications

- [RISC-V Unprivileged ISA Specification](https://riscv.org/technical/specifications/)
  - Chapter 7: M Extension (Division and Multiplication)

### Hardware Division Algorithms

- Hennessy & Patterson, "Computer Architecture: A Quantitative Approach"
  - Appendix J: Computer Arithmetic
- Ercegovac & Lang, "Digital Arithmetic" (Morgan Kaufmann, 2004)
  - Chapter 6: Division Algorithms

### Verilog/SystemVerilog Synthesis

- IEEE 1800-2017 SystemVerilog Standard
- Verilator User Guide: https://verilator.org/guide/latest/

### Testing Resources

- RISC-V Compliance Test Suite: https://github.com/riscv-non-isa/riscv-arch-test
- Marlin Documentation: https://github.com/cucapra/marlin

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-08 | GitHub Copilot | Initial draft |

---

**Document Status:** ✅ **Ready for Implementation**

This plan provides a complete roadmap for replacing the non-synthesizable division operations with a hardware-friendly, multi-cycle division unit based on the non-restoring algorithm. All implementation details, timing diagrams, and test strategies are included to enable AI coding agents to execute the plan successfully.

# RTL Peripheral Implementation Plan
## Hybrid/Split Address Space Approach with LED Controller Example

**Author:** GitHub Copilot Hardware-Software Integration Architect  
**Date:** January 27, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU  
**Based on:** `docs/research/bus-peripheral-implementation-approaches.md`  
**Status:** Implementation Plan - Ready for Execution

---

## Executive Summary

This document provides a detailed technical implementation plan for adding RTL-based peripherals to the RISC-V CPU using the **hybrid/split address space approach** recommended in the research document. The plan includes:

1. **Architecture changes** to support RTL peripherals alongside existing Rust peripherals
2. **Memory map partitioning** to separate RTL and Rust peripheral spaces
3. **Concrete implementation** of a LED controller as a reference example
4. **Integration strategy** for future RTL peripherals

**Key Decision:** Use **Split Bus (Mechanism 3)** approach with:
- RTL peripherals in address range **0x50000000 - 0x5FFFFFFF**
- Rust peripherals remain at existing addresses (SimControl: 0x10000000, Video: 0x20000000, Audio: 0x30000000, FIFO: 0x40000000)
- DRAM unchanged at **0x80000000 - 0xFFFFFFFF**

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Memory Map Design](#2-memory-map-design)
3. [RTL Implementation Plan](#3-rtl-implementation-plan)
4. [LED Controller Specification](#4-gpio-peripheral-specification)
5. [Top-Level Integration](#5-top-level-integration)
6. [Rust Integration Layer](#6-rust-integration-layer)
7. [Testing Strategy](#7-testing-strategy)
8. [Implementation Checklist](#8-implementation-checklist)
9. [Future Extensions](#9-future-extensions)
10. [Reference Information](#10-reference-information)

---

## 1. Architecture Overview

### 1.1 Current Architecture

The existing system uses a **Rust-only peripheral approach**:

```
┌─────────────────────────────────────────────┐
│         Rust Simulation (cpu-sim)            │
│  ┌────────────┐  ┌──────────────────────┐   │
│  │ SystemBus  │  │  Rust Peripherals    │   │
│  │  - DRAM    │  │  - FIFO              │   │
│  │  - Routing │  │  - SimControl        │   │
│  └─────┬──────┘  │  - DMA, Video, Audio │   │
│        │         └──────────────────────┘   │
└────────┼──────────────────────────────────────┘
         │ (imem/dmem FFI)
┌────────▼──────────────────────────────────────┐
│      RISC-V CPU RTL (top.sv)                  │
│  - No internal peripherals                    │
│  - External memory interface only             │
└───────────────────────────────────────────────┘
```

**Limitations:**
- ❌ No RTL peripherals - cannot synthesize peripherals to FPGA
- ❌ No hardware validation of peripheral timing
- ❌ Cannot test RTL peripheral integration

### 1.2 Target Hybrid Architecture

**Split Bus (Mechanism 3)** - Recommended approach:

```
┌──────────────────────────────────────────────────────┐
│              Rust Simulation (cpu-sim)                │
│  ┌────────────────────────────────────────────────┐  │
│  │         SystemBus (Modified)                   │  │
│  │  - Routes RTL peripheral range to RTL          │  │
│  │  - Routes Rust peripheral range to Rust        │  │
│  │  - Routes DRAM range to DRAM                   │  │
│  └────────┬──────────────────┬────────────────────┘  │
│           │ (Rust periph)     │ (RTL periph)          │
│  ┌────────▼──────────┐       │                        │
│  │ Rust Peripherals  │       │                        │
│  │ - FIFO            │       │                        │
│  │ - DMA             │       │                        │
│  │ - Video, Audio    │       │                        │
│  └───────────────────┘       │                        │
└─────────────────────────────┼──────────────────────────┘
                               │ (ext_periph FFI)
┌──────────────────────────────▼────────────────────────┐
│      top_with_peripherals.sv (New Wrapper)            │
│  ┌─────────────────────────────────────────────────┐  │
│  │  Address Decoder & Mux                          │  │
│  │  - Decodes 0x30000000-0x3FFFFFFF to RTL periph  │  │
│  │  - Forwards other addresses to external bus     │  │
│  └────────┬──────────────────┬─────────────────────┘  │
│           │                   │                        │
│  ┌────────▼──────────┐  ┌────▼──────────────────┐     │
│  │   RISC-V CPU      │  │  RTL Peripherals      │     │
│  │   (top.sv)        │  │  - led_controller.sv            │     │
│  │   (Unchanged)     │  │  - timer.sv (future)  │     │
│  └───────────────────┘  │  - uart.sv (future)   │     │
│                         └───────────────────────┘     │
└───────────────────────────────────────────────────────┘
```

**Key Characteristics:**
- ✅ **Synthesizable**: RTL peripherals can be deployed to FPGA
- ✅ **Backward Compatible**: Existing Rust peripherals unchanged
- ✅ **Clean Separation**: RTL and Rust peripherals in separate address spaces
- ✅ **Testable**: Both RTL and Rust peripherals can be tested independently

### 1.3 Design Principles

**RTL Peripheral Design:**
1. **Single-cycle response** where possible (ready = 1'b1)
2. **Ready/valid handshaking** for multi-cycle operations
3. **Asynchronous reset** (active-low `rst_n`)
4. **Separate combinational/sequential** logic (`always_comb` / `always_ff`)
5. **No synthesis warnings** - must pass `verilator --lint-only`

**Integration Principles:**
1. **CPU RTL unchanged** - top.sv remains as-is
2. **Address decoding in wrapper** - top_with_peripherals.sv handles routing
3. **Simple interface** - peripherals use same ready/valid protocol as memory
4. **Verilator compatibility** - no features Verilator doesn't support
5. **Minimal Rust changes** - SystemBus modification only

---

## 2. Memory Map Design

### 2.1 Complete Memory Map

```
Address Range          | Device           | Type | Size    | Description
-----------------------|------------------|------|---------|----------------------------
0x00000000-0x0FFFFFFF | Reserved         | -    | 256 MiB | Reserved for future use
0x10000000-0x100000FF | SimControl       | Rust | 256 B   | Simulation control (existing)
0x11000000-0x1FFFFFFF | Reserved         | -    | ~240 MB | Reserved for future use
0x20000000-0x2000000F | Video            | Rust | 16 B    | Video frame buffer (existing)
0x21000000-0x2FFFFFFF | Reserved         | -    | ~240 MB | Reserved for future use
0x30000000-0x3000000F | Audio            | Rust | 16 B    | Audio buffer (existing)
0x31000000-0x3FFFFFFF | Reserved         | -    | ~240 MB | Reserved for future use
0x40000000-0x40000007 | FIFO             | Rust | 8 B     | Host communication FIFO (existing)
0x41000000-0x4FFFFFFF | Reserved (Rust)  | Rust | ~240 MB | Reserved for future Rust
0x50000000-0x5000000F | LED Controller   | RTL  | 16 B    | 8-bit LED output controller
0x51000000-0x510000FF | Timer (future)   | RTL  | 256 B   | Programmable timer
0x52000000-0x520000FF | UART (future)    | RTL  | 256 B   | UART controller
0x53000000-0x530000FF | SPI (future)     | RTL  | 256 B   | SPI master
0x54000000-0x540000FF | I2C (future)     | RTL  | 256 B   | I2C master
0x55000000-0x5FFFFFFF | Reserved (RTL)   | RTL  | ~176 MB | Reserved for future RTL
0x60000000-0x7FFFFFFF | Reserved         | -    | 512 MiB | Reserved for future use
0x80000000-0xFFFFFFFF | DRAM             | Both | 2 GiB   | System memory

Note: SimControl remains a Rust peripheral at its existing address (0x10000000).
The simulator detects program termination via self.bus.sim_control.termination_requested(),
so SimControl must stay in the Rust bus to maintain this functionality.
```

### 2.2 Address Decoding Strategy

**In RTL (top_with_peripherals.sv):**

```systemverilog
// Address range definitions
localparam RTL_PERIPH_BASE  = 32'h50000000;
localparam RTL_PERIPH_LIMIT = 32'h60000000;

localparam LED_BASE = 32'h50000000;
localparam LED_SIZE = 32'h00000010;  // 16 bytes

// Decode address to peripheral select
logic sel_led;
logic sel_external;  // For Rust peripherals + DRAM
logic sel_unmapped_rtl;  // Unmapped RTL peripheral space

assign sel_led          = (cpu_dmem_addr >= LED_BASE) && 
                          (cpu_dmem_addr < LED_BASE + LED_SIZE);
assign sel_unmapped_rtl = (cpu_dmem_addr >= RTL_PERIPH_BASE) && 
                          (cpu_dmem_addr < RTL_PERIPH_LIMIT) && !sel_led;
assign sel_external     = (cpu_dmem_addr < RTL_PERIPH_BASE) || 
                          (cpu_dmem_addr >= RTL_PERIPH_LIMIT);
```

**In Rust (cpu-sim/src/bus.rs):**

```rust
// Constants in riscv_shared/src/bus.rs
pub const RTL_PERIPH_BASE: u32  = 0x50000000;
pub const RTL_PERIPH_LIMIT: u32 = 0x60000000;

pub const LED_BASE: u32 = 0x50000000;
pub const LED_SIZE: u32 = 0x00000010;

// In SystemBus routing logic
fn route_address(&self, addr: u32) -> RoutingTarget {
    if addr >= RTL_PERIPH_BASE && addr < RTL_PERIPH_LIMIT {
        RoutingTarget::RtlPeripheral  // Forward to RTL
    } else if addr >= DRAM_BASE {
        RoutingTarget::Dram
    } else {
        RoutingTarget::RustPeripheral  // SimControl, FIFO, Video, Audio, etc.
    }
}
```

### 2.3 Address Space Reservation

**RTL Peripheral Space (0x30000000 - 0x3FFFFFFF):**
- Total: 256 MiB
- Each peripheral gets a dedicated block
- Leave generous gaps for register expansion
- Align to convenient boundaries (e.g., 16 MiB per major peripheral)

**Design Rationale:**
- **Large blocks**: Easy to decode (only check upper 8-12 bits)
- **Gaps between peripherals**: Allows register map expansion without conflicts
- **Power-of-2 boundaries**: Simplifies address decoder logic

---

## 3. RTL Implementation Plan

### 3.1 File Structure

```
rtl/
├── top.sv                          # Existing CPU (UNCHANGED)
├── alu.sv, decoder.sv, ...         # Existing CPU modules (UNCHANGED)
├── top_with_peripherals.sv         # NEW: Wrapper with peripheral integration
└── peripherals/                    # NEW: Directory for RTL peripherals
    ├── led_controller.sv                     # NEW: LED controller
    ├── timer.sv                    # FUTURE
    └── uart.sv                     # FUTURE
```

### 3.2 Implementation Phases

**Phase 1: LED Controller Peripheral (This Plan)**
- ✅ Create `rtl/peripherals/led_controller.sv`
- ✅ Create `rtl/top_with_peripherals.sv`
- ✅ Update Rust integration
- ✅ Add tests

**Phase 2: Future Peripherals**
- ⏳ Timer (programmable countdown timer)
- ⏳ UART (serial communication)
- ⏳ Additional peripherals as needed

### 3.3 Top-Level Wrapper Structure

The wrapper module `top_with_peripherals.sv` will:

1. **Instantiate CPU core** (top.sv) - no changes to CPU
2. **Instantiate RTL peripherals** (led_controller.sv, etc.)
3. **Decode addresses** to select peripheral or external bus
4. **Multiplex responses** from peripherals back to CPU
5. **Forward non-RTL addresses** to external Rust bus

**Key Interface Changes:**

```systemverilog
// OLD: Direct connection to Rust (current top.sv)
module top (
    // ... existing signals ...
    output logic [31:0] dmem_addr,
    input  logic [31:0] dmem_rdata,
    // ...
);

// NEW: Wrapper exposes external memory interface for Rust
module top_with_peripherals (
    // ... same clock, reset, boot_addr ...
    
    // External memory interface (for Rust peripherals + DRAM)
    output logic [31:0] ext_mem_addr,
    output logic [31:0] ext_mem_wdata,
    input  logic [31:0] ext_mem_rdata,
    output logic        ext_mem_we,
    output logic        ext_mem_re,
    output logic [1:0]  ext_mem_size,
    output logic        ext_mem_req,
    input  logic        ext_mem_ready,
    
    // LED pins (exposed to top level for FPGA synthesis)
    output logic [7:0]  led_out,
    input  logic [7:0]  led_in,
    output logic [7:0]  led_dir,  // 1=output, 0=input
    
    // ... debug signals same as top.sv ...
);
```

---

## 4. LED Controller Specification

### 4.1 Overview

**Purpose:** Control 8 external LED outputs when the RTL is synthesized on an FPGA.

**Features:**
- 8 output-only LED control signals
- Single control register
- Simple write-only interface
- No input capability (simplified design)

**Design Philosophy:**
This is a basic peripheral to demonstrate RTL peripheral integration. It is intentionally simple - just an 8-bit output register that drives LEDs. No input pins, no direction control, no complex features.

### 4.2 Register Map

```
Offset | Name      | Access | Reset  | Description
-------|-----------|--------|--------|---------------------------------------
0x00   | LED_OUT   | RW     | 0x00   | LED output data register
0x04   | Reserved  | -      | -      | Reserved for future use
0x08   | Reserved  | -      | -      | Reserved for future use
0x0C   | Reserved  | -      | -      | Reserved for future use
```

**Address Calculation:**
```
LED_OUT_ADDR = 0x50000000 + 0x00 = 0x50000000
```

### 4.3 Register Description

**LED_OUT (0x50000000):**
- **Bits [7:0]**: Output data for LED signals
- **Bits [31:8]**: Reserved (read as 0, writes ignored)
- **Behavior**: LED outputs directly driven by LED_OUT[7:0]
- **Reset value**: 0x00000000 (all LEDs off)
- **Read behavior**: Reads back the last written value
- **Write behavior**: Updates LED outputs on next clock cycle

**Access Size Support (Required):**
- **Word (32-bit)**: Full 32-bit write/read
- **Halfword (16-bit)**: Writes/reads lower 16 bits, upper bits unchanged on write
- **Byte (8-bit)**: Writes/reads specific byte, other bytes unchanged on write

Byte/halfword accesses must use proper byte lane masking to avoid affecting unintended bits.

### 4.4 Timing Characteristics

**Read Operations:**
- **Latency**: 1 cycle (ready = 1'b1)
- **Behavior**: Combinational read from register

**Write Operations:**
- **Latency**: 1 cycle (ready = 1'b1)
- **Behavior**: Register update on next clock edge

**LED Update Timing:**
- **Output propagation**: LED outputs updated 1 clock cycle after write
- **No input**: This is an output-only peripheral

### 4.5 Interface Signals

**Module Interface:**

```systemverilog
module led_controller (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit)
    input  logic [31:0] wdata,     // Write data
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable
    input  logic        re,        // Read enable
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word)
    output logic        ready,     // Operation complete
    
    // LED outputs
    output logic [7:0]  led_out    // LED outputs (to FPGA pins)
);
```

**Pin Mapping (for FPGA):**

On FPGA, `led_out[7:0]` would be connected directly to 8 LED outputs.

**Example FPGA Top-Level:**

```systemverilog
module fpga_top (
    input  logic       clk_100mhz,
    input  logic       rst_btn_n,
    
    // LED outputs
    output logic [7:0] led
);
    // CPU and peripheral logic
    logic [7:0] led_out;
    
    top_with_peripherals cpu_system (
        .clk(clk_100mhz),
        .rst_n(rst_btn_n),
        .led_out(led_out),
        // ... other connections ...
    );
    
    // Direct connection to LEDs
    assign led = led_out;
endmodule
```

### 4.6 Functional Behavior

**Simple Output Mode:**
- LEDs are always driven by LED_OUT[7:0]
- Writing to LED_OUT updates the LED outputs
- Reading from LED_OUT returns the last written value
- No input capability - this is output-only

**Example Usage:**

```c
// Turn on all LEDs
*(volatile uint32_t *)0x50000000 = 0xFF;

// Turn off all LEDs
*(volatile uint32_t *)0x50000000 = 0x00;

// Set a pattern (alternating LEDs)
*(volatile uint32_t *)0x50000000 = 0xAA;

// Read back current state
uint32_t led_state = *(volatile uint32_t *)0x50000000;
uint8_t leds = led_state & 0xFF;
```

**Byte Access Example:**

```c
// Write only LED[7:0] using byte access
*(volatile uint8_t *)0x50000000 = 0x55;

// Write only LED[7:0] using halfword access (lower 8 bits affected)
*(volatile uint16_t *)0x50000000 = 0x00AA;
```

---

## 5. Top-Level Integration

### 5.1 Wrapper Module Design

**File:** `rtl/top_with_peripherals.sv`

**Purpose:**
- Instantiate CPU core (top.sv)
- Instantiate RTL peripherals
- Decode addresses and route requests
- Multiplex responses back to CPU
- Expose external memory interface for Rust integration

### 5.2 Address Decoder Logic

**Decoder Implementation:**

```systemverilog
// Address range checking
logic sel_led;
logic sel_external;
logic sel_unmapped_rtl;

// LED range: 0x50000000 - 0x5000000F
assign sel_led = (cpu_dmem_addr[31:4] == 28'h5000000);

// Unmapped RTL peripheral space
assign sel_unmapped_rtl = (cpu_dmem_addr >= 32'h50000000) && 
                          (cpu_dmem_addr < 32'h60000000) && !sel_led;

// External: anything not in RTL peripheral space
assign sel_external = (cpu_dmem_addr < 32'h50000000) || 
                      (cpu_dmem_addr >= 32'h60000000);
```

**Alternative (more explicit):**

```systemverilog
always_comb begin
    sel_led          = 1'b0;
    sel_unmapped_rtl = 1'b0;
    sel_external     = 1'b0;
    
    // Check if address is in LED range
    if (cpu_dmem_addr >= 32'h50000000 && cpu_dmem_addr < 32'h50000010) begin
        sel_led = 1'b1;
    end
    // Check if address is in unmapped RTL peripheral space
    else if (cpu_dmem_addr >= 32'h50000000 && cpu_dmem_addr < 32'h60000000) begin
        sel_unmapped_rtl = 1'b1;
    end
    // Otherwise route to external bus
    else begin
        sel_external = 1'b1;
    end
end
```

### 5.3 Response Multiplexer

**Mux Implementation:**

```systemverilog
always_comb begin
    // Default values
    cpu_dmem_rdata = 32'h0;
    cpu_dmem_ready = 1'b0;
    
    // Select response source
    if (sel_led) begin
        cpu_dmem_rdata = led_rdata;
        cpu_dmem_ready = led_ready;
    end else if (sel_unmapped_rtl) begin
        // Unmapped RTL peripheral address - return zero and ready immediately
        // Issue warning via $display for debugging
        $display("WARNING: Access to unmapped RTL peripheral address 0x%08x", cpu_dmem_addr);
        cpu_dmem_rdata = 32'h0;
        cpu_dmem_ready = 1'b1;
    end else if (sel_external) begin
        cpu_dmem_rdata = ext_mem_rdata;
        cpu_dmem_ready = ext_mem_ready;
    end else begin
        // Should never reach here if decoder logic is correct
        cpu_dmem_rdata = 32'h0;
        cpu_dmem_ready = 1'b1;
    end
end
```

### 5.4 External Bus Forwarding

**Forward Signals to Rust:**

```systemverilog
// Forward CPU requests to external bus (Rust)
assign ext_mem_addr  = cpu_dmem_addr;
assign ext_mem_wdata = cpu_dmem_wdata;
assign ext_mem_size  = cpu_dmem_size;

// Only assert request/enable if address is external
assign ext_mem_req = cpu_dmem_req && sel_external;
assign ext_mem_we  = cpu_dmem_we  && sel_external;
assign ext_mem_re  = cpu_dmem_re  && sel_external;
```

### 5.5 Complete Wrapper Skeleton

**Simplified Structure:**

```systemverilog
module top_with_peripherals (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory (unchanged - passed through)
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    output logic        imem_req,
    input  logic        imem_ready,
    
    // External data memory (for Rust peripherals + DRAM)
    output logic [31:0] ext_mem_addr,
    output logic [31:0] ext_mem_wdata,
    input  logic [31:0] ext_mem_rdata,
    output logic        ext_mem_we,
    output logic        ext_mem_re,
    output logic [1:0]  ext_mem_size,
    output logic        ext_mem_req,
    input  logic        ext_mem_ready,
    
    // LED pins
    output logic [7:0]  led_out,
    
    // Debug/control signals (passed through from CPU)
    output logic        halted,
    output logic        instr_complete,
    output logic [31:0] debug_pc,
    output logic [31:0] debug_instruction,
    output logic [31:0] debug_current_pc,
    output logic [31:0] debug_current_instruction,
    output logic [3:0]  debug_fsm_state,
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);

    // Internal CPU memory interface
    logic [31:0] cpu_dmem_addr;
    logic [31:0] cpu_dmem_wdata;
    logic [31:0] cpu_dmem_rdata;
    logic        cpu_dmem_we;
    logic        cpu_dmem_re;
    logic [1:0]  cpu_dmem_size;
    logic        cpu_dmem_req;
    logic        cpu_dmem_ready;
    
    // Peripheral response signals
    logic [31:0] led_rdata;
    logic        led_ready;
    
    // Address decoder
    logic sel_led;
    logic sel_unmapped_rtl;
    logic sel_external;
    
    // Instantiate CPU core
    top cpu (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(boot_addr),
        
        // Instruction memory (pass through)
        .imem_addr(imem_addr),
        .imem_data(imem_data),
        .imem_req(imem_req),
        .imem_ready(imem_ready),
        
        // Data memory (internal)
        .dmem_addr(cpu_dmem_addr),
        .dmem_wdata(cpu_dmem_wdata),
        .dmem_rdata(cpu_dmem_rdata),
        .dmem_we(cpu_dmem_we),
        .dmem_re(cpu_dmem_re),
        .dmem_size(cpu_dmem_size),
        .dmem_req(cpu_dmem_req),
        .dmem_ready(cpu_dmem_ready),
        
        // Debug signals
        .halted(halted),
        .instr_complete(instr_complete),
        .debug_pc(debug_pc),
        .debug_instruction(debug_instruction),
        .debug_current_pc(debug_current_pc),
        .debug_current_instruction(debug_current_instruction),
        .debug_fsm_state(debug_fsm_state),
        .debug_rs1_data(debug_rs1_data),
        .debug_rs2_data(debug_rs2_data),
        .debug_rd_data(debug_rd_data)
    );
    
    // Instantiate LED controller peripheral
    led_controller led_periph (
        .clk(clk),
        .rst_n(rst_n),
        .addr(cpu_dmem_addr),
        .wdata(cpu_dmem_wdata),
        .rdata(led_rdata),
        .we(cpu_dmem_we && sel_led),
        .re(cpu_dmem_re && sel_led),
        .size(cpu_dmem_size),
        .ready(led_ready),
        .led_out(led_out)
    );
    
    // Address decoder
    assign sel_led = (cpu_dmem_addr[31:4] == 28'h5000000);  // 0x50000000 - 0x5000000F
    assign sel_unmapped_rtl = (cpu_dmem_addr >= 32'h50000000) && 
                              (cpu_dmem_addr < 32'h60000000) && !sel_led;
    assign sel_external = (cpu_dmem_addr < 32'h50000000) || 
                          (cpu_dmem_addr >= 32'h60000000);
    
    // Response multiplexer
    always_comb begin
        if (sel_led) begin
            cpu_dmem_rdata = led_rdata;
            cpu_dmem_ready = led_ready;
        end else if (sel_unmapped_rtl) begin
            // Unmapped RTL peripheral - return zero and issue warning
            $display("WARNING: Access to unmapped RTL peripheral address 0x%08x", cpu_dmem_addr);
            cpu_dmem_rdata = 32'h0;
            cpu_dmem_ready = 1'b1;
        end else begin
            cpu_dmem_rdata = ext_mem_rdata;
            cpu_dmem_ready = ext_mem_ready;
        end
    end
    
    // Forward to external bus
    assign ext_mem_addr  = cpu_dmem_addr;
    assign ext_mem_wdata = cpu_dmem_wdata;
    assign ext_mem_size  = cpu_dmem_size;
    assign ext_mem_req   = cpu_dmem_req && sel_external;
    assign ext_mem_we    = cpu_dmem_we  && sel_external;
    assign ext_mem_re    = cpu_dmem_re  && sel_external;

endmodule
```

---

## 6. Rust Integration Layer

### 6.1 Required Changes to cpu-sim

**Files to Modify:**
1. `riscv_shared/src/bus.rs` - Add LED address constants
2. `cpu-sim/src/lib.rs` - Update Verilator bindings to use new wrapper
3. `cpu-sim/src/bus.rs` - Add routing logic for RTL peripheral range
4. Integration tests - Add LED tests

### 6.2 Address Constants

**In `riscv_shared/src/bus.rs`:**

```rust
// RTL Peripheral Address Space
pub const RTL_PERIPH_BASE: u32  = 0x50000000;
pub const RTL_PERIPH_LIMIT: u32 = 0x60000000;

// LED Controller Peripheral
pub const LED_BASE: u32 = 0x50000000;
pub const LED_SIZE: u32 = 0x00000010;  // 16 bytes

// LED Register Offsets
pub const LED_OUT_OFFSET: u32 = 0x00;

// Helper functions
pub fn led_out_addr() -> u32 { LED_BASE + LED_OUT_OFFSET }
```

### 6.3 SystemBus Routing Logic

**In `cpu-sim/src/bus.rs`:**

```rust
impl SystemBus {
    /// Route address to appropriate handler
    fn is_rtl_peripheral(&self, addr: u32) -> bool {
        addr >= RTL_PERIPH_BASE && addr < RTL_PERIPH_LIMIT
    }
    
    /// Read from bus with RTL peripheral awareness
    pub fn read_word(&mut self, addr: u32) -> u32 {
        // RTL peripherals are handled by Verilator, not Rust
        if self.is_rtl_peripheral(addr) {
            // This should never be called for RTL peripherals
            // The simulator should route these directly to Verilator
            panic!("RTL peripheral read should be handled by Verilator: 0x{:08x}", addr);
        }
        
        // Existing logic for Rust peripherals and DRAM
        // ... (unchanged) ...
    }
    
    /// Write to bus with RTL peripheral awareness
    pub fn write_word(&mut self, addr: u32, value: u32) {
        // RTL peripherals are handled by Verilator, not Rust
        if self.is_rtl_peripheral(addr) {
            // This should never be called for RTL peripherals
            panic!("RTL peripheral write should be handled by Verilator: 0x{:08x}", addr);
        }
        
        // Existing logic for Rust peripherals and DRAM
        // ... (unchanged) ...
    }
}
```

### 6.4 Simulator Integration

**In `cpu-sim/src/lib.rs` (or main simulator file):**

The simulator now needs to:
1. Use `top_with_peripherals` instead of `top`
2. Connect external memory interface (ext_mem_*) instead of dmem_*
3. Expose LED pin signals for testing

**Conceptual Change:**

```rust
// OLD: Direct connection to top.sv
// self.core.dmem_addr, dmem_rdata, etc.

// NEW: Connection to top_with_peripherals.sv
// self.core.ext_mem_addr, ext_mem_rdata, etc.
// LED signals: self.core.led_out
```

**Note:** The exact implementation depends on how Marlin generates bindings. The wrapper module should expose the same interface pattern, just with `ext_mem_*` prefix instead of `dmem_*`.

### 6.5 Verilator Build Configuration

**Potential changes to build process:**

1. **Include new files** in Verilator compilation:
   ```
   rtl/top_with_peripherals.sv      (new top-level)
   rtl/peripherals/led_controller.sv (new peripheral)
   ```

2. **Update marlin configuration** (if needed):
   - Change top module from `top` to `top_with_peripherals`
   - May need to update `Cargo.toml` or marlin config

3. **Clean and rebuild**:
   ```bash
   cargo clean  # Clear Verilator cache
   cargo build  # Rebuild with new RTL
   ```

---

## 7. Testing Strategy

### 7.1 Test Pyramid

```
┌────────────────────────────────────┐
│  Integration Tests (Rust)          │  ← Full system test
│  - LED read/write from CPU         │
│  - LED control program             │
└────────────────────────────────────┘
┌────────────────────────────────────┐
│  Unit Tests (RTL Testbench)        │  ← Optional Verilator tests
│  - LED register access             │
│  - Output control behavior         │
└────────────────────────────────────┘
┌────────────────────────────────────┐
│  Linting (Verilator)               │  ← Mandatory before testing
│  - Syntax and style checks         │
│  - Synthesis warnings              │
└────────────────────────────────────┘
```

### 7.2 Linting Tests

**Verify RTL quality:**

```bash
# Lint LED controller module
verilator --lint-only rtl/peripherals/led_controller.sv

# Lint wrapper module
verilator --lint-only rtl/top_with_peripherals.sv \
    -I rtl \
    -I rtl/peripherals

# Expected output: No warnings or errors
```

**Pass criteria:**
- ✅ No syntax errors
- ✅ No linter warnings
- ✅ No unused signals
- ✅ No undriven outputs

### 7.3 Unit Tests (Optional RTL Testbench)

**Note:** For this project, integration tests via Rust are preferred. RTL testbenches are optional.

**Example structure (if implemented):**

```systemverilog
// rtl/peripherals/led_controller_tb.sv
module led_controller_tb;
    // Test signals
    logic clk, rst_n;
    logic [31:0] addr, wdata, rdata;
    logic we, re, ready;
    logic [1:0] size;
    logic [7:0] led_out;
    
    // Instantiate DUT
    led_controller dut (.*);
    
    // Clock generation
    initial clk = 0;
    always #5 clk = ~clk;
    
    // Test cases
    initial begin
        // Reset
        rst_n = 0; #20; rst_n = 1;
        
        // Test 1: Write to LED_OUT (word access)
        @(posedge clk);
        addr = 32'h50000000; wdata = 32'hAA; we = 1; re = 0; size = 2'b10;
        @(posedge clk);
        we = 0;
        assert(led_out == 8'hAA);
        
        // Test 2: Read back LED_OUT
        @(posedge clk);
        addr = 32'h50000000; we = 0; re = 1; size = 2'b10;
        @(posedge clk);
        assert(rdata[7:0] == 8'hAA);
        
        // Test 3: Byte access
        @(posedge clk);
        addr = 32'h50000000; wdata = 32'h55; we = 1; re = 0; size = 2'b00;
        @(posedge clk);
        we = 0;
        assert(led_out == 8'h55);
        
        $display("All tests passed!");
        $finish;
    end
endmodule
```

### 7.4 Integration Tests (Rust)

**Primary testing approach for this project.**

**Test File:** `testbench/tests/led_test.rs`

**Test Cases:**

1. **Basic Register Access**
   - Write to LED_OUT, verify via direct signal inspection
   - Read back LED_OUT value

2. **LED Control Pattern**
   - Write sequence to LED_OUT
   - Verify output pattern matches

3. **Access Size Testing (Required)**
   - Word (32-bit) access
   - Halfword (16-bit) access with proper byte lane masking
   - Byte (8-bit) access with proper byte lane masking

4. **Edge Cases**
   - Write to invalid offsets (should return ready with warning)
   - Read from reserved registers (should return 0)
   - Unmapped RTL peripheral addresses (should warn and return 0)

**Example Test Structure:**

```rust
#[test]
fn test_led_basic_write_read() {
    // Create simulator with new wrapper
    let mut sim = create_simulator();
    
    // Reset
    sim.reset();
    
    // Write 0xAA to LED_OUT (0x50000000)
    sim.write_word(led_out_addr(), 0xAA);
    sim.step();
    
    // Verify led_out signal
    assert_eq!(sim.core.led_out, 0xAA, "LED output mismatch");
    
    // Read back LED_OUT
    let readback = sim.read_word(led_out_addr());
    assert_eq!(readback & 0xFF, 0xAA, "LED readback mismatch");
}

#[test]
fn test_led_pattern() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Test LED patterns
    let patterns = [0x00, 0xFF, 0xAA, 0x55, 0x0F, 0xF0];
    for pattern in patterns {
        sim.write_word(led_out_addr(), pattern);
        sim.step();
        assert_eq!(sim.core.led_out, pattern, 
                   "LED pattern 0x{:02X} failed", pattern);
    }
}

#[test]
fn test_led_byte_access() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Byte write to LED_OUT
    sim.write_byte(led_out_addr(), 0x55);
    sim.step();
    assert_eq!(sim.core.led_out, 0x55);
    
    // Byte read from LED_OUT
    let val = sim.read_byte(led_out_addr());
    assert_eq!(val, 0x55);
}
    let input_val = sim.read_word(led_in_addr());
    assert_eq!(input_val & 0xFF, 0x55, "LED input mismatch");
}

#[test]
fn test_led_led_pattern() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Configure all as outputs
    sim.write_word(led_dir_addr(), 0xFF);
    
    // Test LED patterns
    let patterns = [0x00, 0xFF, 0xAA, 0x55, 0x0F, 0xF0];
    for pattern in patterns {
        sim.write_word(led_out_addr(), pattern);
        sim.step();
        assert_eq!(sim.core.led_out, pattern, 
                   "LED pattern 0x{:02X} failed", pattern);
    }
}

#[test]
fn test_led_direction_control() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Test: Outputs disabled by default (dir = 0)
    sim.write_word(led_out_addr(), 0xFF);
    sim.step();
    assert_eq!(sim.core.led_dir, 0x00, "Should default to inputs");
    
    // Enable outputs
    sim.write_word(led_dir_addr(), 0xFF);
    sim.step();
    assert_eq!(sim.core.led_out, 0xFF);
    assert_eq!(sim.core.led_dir, 0xFF);
}
```

### 7.5 Test Execution Plan

**Step-by-step testing:**

1. **Lint RTL** - Verify no syntax/style issues
   ```bash
   verilator --lint-only rtl/peripherals/led_controller.sv
   verilator --lint-only rtl/top_with_peripherals.sv
   ```

2. **Clean build cache** - Ensure fresh Verilator compilation
   ```bash
   cargo clean
   ```

3. **Build** - Compile with new RTL
   ```bash
   cargo build
   ```

4. **Run tests** - Execute integration tests
   ```bash
   cargo test gpio  # Run LED-specific tests
   cargo test       # Run all tests (verify no regressions)
   ```

5. **Format and lint Rust** - Ensure code quality
   ```bash
   cargo fmt
   cargo clippy --fix --allow-dirty
   cargo clippy -- -D warnings
   ```

---

## 8. Implementation Checklist

### Phase 1: RTL Implementation

- [ ] **Create LED controller module** (`rtl/peripherals/led_controller.sv`)
  - [ ] Define module interface
  - [ ] Implement register file (LED_OUT only)
  - [ ] Implement address decoder
  - [ ] Implement read/write logic with byte/halfword support
  - [ ] Add proper reset behavior
  - [ ] Lint with Verilator

- [ ] **Create wrapper module** (`rtl/top_with_peripherals.sv`)
  - [ ] Define module interface with ext_mem_* signals
  - [ ] Instantiate CPU core (top.sv)
  - [ ] Instantiate LED controller
  - [ ] Implement address decoder
  - [ ] Implement response multiplexer
  - [ ] Connect external bus forwarding
  - [ ] Lint with Verilator

### Phase 2: Rust Integration

- [ ] **Update address constants** (`riscv_shared/src/bus.rs`)
  - [ ] Add RTL_PERIPH_BASE, RTL_PERIPH_LIMIT
  - [ ] Add LED_BASE, LED_SIZE
  - [ ] Add LED register offset constants
  - [ ] Add helper functions

- [ ] **Update SystemBus** (`cpu-sim/src/bus.rs`)
  - [ ] Add is_rtl_peripheral() method
  - [ ] Update read_word() with RTL check
  - [ ] Update write_word() with RTL check
  - [ ] Update documentation

- [ ] **Update simulator** (`cpu-sim/src/lib.rs` or equivalent)
  - [ ] Change Verilator module to top_with_peripherals
  - [ ] Update memory interface signals (dmem → ext_mem)
  - [ ] Expose LED pin signals
  - [ ] Update tests helper functions if needed

- [ ] **Build configuration**
  - [ ] Update Cargo.toml or marlin config (if needed)
  - [ ] Verify include paths for new RTL files
  - [ ] Test clean build: `cargo clean && cargo build`

### Phase 3: Testing

- [ ] **Linting**
  - [ ] `verilator --lint-only rtl/peripherals/led_controller.sv` → Pass
  - [ ] `verilator --lint-only rtl/top_with_peripherals.sv` → Pass

- [ ] **Integration tests** (`testbench/tests/led_test.rs`)
  - [ ] Test: Basic register read/write
  - [ ] Test: LED_OUT updates output pins
  - [ ] Test: Byte/halfword access with proper masking
  - [ ] Test: LED pattern sequence
  - [ ] Test: Invalid address handling (unmapped RTL space)
  - [ ] Test: Read back LED_OUT value

- [ ] **Regression tests**
  - [ ] `cargo test` → All existing tests still pass
  - [ ] No degradation in other functionality

- [ ] **Code quality**
  - [ ] `cargo fmt` → Format Rust code
  - [ ] `cargo clippy --fix --allow-dirty` → Auto-fix warnings
  - [ ] `cargo clippy -- -D warnings` → Zero warnings

### Phase 4: Documentation

- [ ] **Update AGENTS.md**
  - [ ] Add memory map with LED controller
  - [ ] Document RTL peripheral integration process
  - [ ] Add LED usage examples

- [ ] **Create peripheral documentation**
  - [ ] LED register map reference
  - [ ] LED usage examples (C/Rust)
  - [ ] FPGA synthesis notes

- [ ] **Update README** (if applicable)
  - [ ] Mention hybrid RTL/Rust peripheral architecture
  - [ ] Link to peripheral documentation

### Phase 5: Validation

- [ ] **Simulation validation**
  - [ ] Run full test suite: `cargo test --verbose`
  - [ ] Verify LED behavior in longer programs
  - [ ] Check performance impact (should be minimal)

- [ ] **Code review readiness**
  - [ ] All tests pass
  - [ ] No clippy warnings
  - [ ] Code formatted
  - [ ] Documentation complete

---

## 9. Future Extensions

### 9.1 Additional RTL Peripherals

**Priority order for future development:**

1. **Timer/Counter** (0x32000000)
   - Programmable countdown timer
   - Interrupt generation (if interrupt controller added)
   - Use cases: OS tick, timeouts, PWM

2. **UART** (0x33000000)
   - Serial communication
   - Essential for FPGA debug output
   - Standard 8N1 protocol

3. **SPI Master** (0x34000000)
   - SPI bus communication
   - Flash memory, SD card, sensors
   - Configurable clock divider

4. **I2C Master** (0x35000000)
   - I2C bus communication
   - Common for sensors, EEPROMs
   - Multi-master support (future)

5. **Interrupt Controller** (0x36000000)
   - Centralized interrupt management
   - Priority encoding
   - Integration with RISC-V CSRs

### 9.2 Enhanced LED Features (Future)

**Future LED controller improvements:**

- **Interrupt support**: Edge-triggered interrupts on input pins
- **Pin configuration**: Pull-up/pull-down resistors
- **Input synchronization**: 2-FF synchronizer for metastability
- **Atomic operations**: Set/clear/toggle registers
- **Pin multiplexing**: Alternate function select
- **Drive strength**: Configurable output drive

**Example enhanced register map:**

```
0x00  LED_OUT       Output data
0x04  LED_IN        Input data
0x08  LED_DIR       Direction control
0x0C  GPIO_SET       Atomic set (write 1 to set)
0x10  GPIO_CLR       Atomic clear (write 1 to clear)
0x14  GPIO_TOG       Atomic toggle (write 1 to toggle)
0x18  GPIO_IEN       Interrupt enable
0x1C  GPIO_ISR       Interrupt status
0x20  GPIO_CFG       Pin configuration (pull-up/down)
```

### 9.3 FPGA Deployment Strategy

**Steps for FPGA synthesis:**

1. **Choose target board** (e.g., Arty A7, Nexys Video)
2. **Create FPGA top-level** wrapping top_with_peripherals
3. **Add clock management** (PLL for CPU clock)
4. **Add reset logic** (button debouncing)
5. **Constrain LED pins** (XDC constraints file)
6. **Synthesize with Vivado/Quartus**
7. **Test on hardware**

**Example constraints (Xilinx):**

```tcl
# Clock constraint
create_clock -period 10.000 -name clk [get_ports clk_100mhz]

# GPIO/LED pins
set_property PACKAGE_PIN H5 [get_ports {led[0]}]
set_property PACKAGE_PIN J5 [get_ports {led[1]}]
# ... etc ...
set_property IOSTANDARD LVCMOS33 [get_ports led[*]]
```

### 9.4 Bus Fabric Upgrade

**For more complex systems, consider:**

- **AXI4-Lite bus** - Industry standard, better for high-speed peripherals
- **Wishbone B4 bus** - Open-source, simpler than AXI
- **Custom crossbar** - If multiple masters needed (CPU + DMA)

**Trade-offs:**
- ➕ Better performance, standard interfaces
- ➖ More complex, longer development time

**Recommendation:** Current simple bus is sufficient for this project. Only upgrade if adding DMA masters or multi-core support.

---

## 10. Reference Information

### 10.1 Key Files

**RTL Files (New):**
- `rtl/peripherals/led_controller.sv` - LED controller module
- `rtl/top_with_peripherals.sv` - Wrapper with peripheral integration

**RTL Files (Unchanged):**
- `rtl/top.sv` - CPU core (no modifications)
- `rtl/alu.sv`, `rtl/decoder.sv`, etc. - CPU submodules

**Rust Files (Modified):**
- `riscv_shared/src/bus.rs` - Address constants
- `cpu-sim/src/bus.rs` - Bus routing logic
- `cpu-sim/src/lib.rs` - Verilator bindings
- `testbench/tests/led_test.rs` - New test file

### 10.2 Memory Map Quick Reference

```
Address       | Device           | Registers
--------------|------------------|------------------------------------
0x10000000    | SimControl       | Tohost (Rust)
0x20000000    | Video            | Frame buffer (Rust)
0x30000000    | Audio            | Audio buffer (Rust)
0x40000000    | FIFO             | DATA, STATUS (Rust)
0x50000000    | LED Controller   | OUT (RW, RTL)
0x80000000+   | DRAM             | System memory (Rust)
```

### 10.3 Command Reference

```bash
# Lint RTL
verilator --lint-only rtl/peripherals/led_controller.sv
verilator --lint-only rtl/top_with_peripherals.sv

# Build system
cargo clean             # Clear Verilator cache
cargo build             # Build with new RTL
cargo build --release   # Optimized build

# Test
cargo test led          # LED-specific tests
cargo test --verbose    # All tests with output

# Code quality
cargo fmt                        # Format code
cargo clippy --fix --allow-dirty # Auto-fix warnings
cargo clippy -- -D warnings      # Verify zero warnings
```

### 10.4 Common Issues and Solutions

**Issue:** Verilator cache stale after RTL changes  
**Solution:** `cargo clean` before rebuild

**Issue:** Address decode conflict  
**Solution:** Check address ranges don't overlap, verify decoder logic

**Issue:** Ready signal always 0  
**Solution:** Verify peripheral ready assignment, check sel_* signals

**Issue:** GPIO reads return 0  
**Solution:** Check response mux, verify peripheral instantiation

**Issue:** Tests timeout  
**Solution:** Check FSM not stuck, verify memory ready signals

### 10.5 Related Documentation

- **Research:** `docs/research/bus-peripheral-implementation-approaches.md`
- **Main Guide:** `AGENTS.md`
- **Current Bus:** `cpu-sim/src/bus.rs`
- **CPU Interface:** `rtl/top.sv`
- **Verilator Docs:** https://verilator.org/guide/latest/

### 10.6 Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Integration Approach** | Split Bus (Mechanism 3) | Synthesizable, clean separation, testable |
| **Address Space** | RTL: 0x30000000-0x3FFFFFFF | Large, easy to decode, room for expansion |
| **GPIO Base** | 0x31000000 | Simple decode, aligned, memorable |
| **GPIO Size** | 16 bytes (3 registers) | Minimal, room for future registers |
| **CPU Modification** | None (wrapper approach) | Preserve CPU, enable/disable peripherals easily |
| **Default Direction** | Input (0) | Safe default, prevents contention |
| **Ready Latency** | 1 cycle (immediate) | Simple registers, no need for multi-cycle |

---

## Appendix A: Complete LED Controller Register Map

```
Register   | Offset | Access | Reset      | Description
-----------|--------|--------|------------|----------------------------------
LED_OUT    | 0x00   | RW     | 0x00000000 | LED output data register
           |        |        |            |   [7:0]   - LED output data
           |        |        |            |   [31:8]  - Reserved (0)
-----------|--------|--------|------------|----------------------------------
Reserved   | 0x04   | -      | -          | Reserved for future use
-----------|--------|--------|------------|----------------------------------
Reserved   | 0x08   | -      | -          | Reserved for future use
-----------|--------|--------|------------|----------------------------------
Reserved   | 0x0C   | -      | -          | Reserved for future use
```

**Access:**
- **RW** = Read/Write

**Addresses:**
- LED_OUT: 0x50000000

---

## Appendix B: Example Programs

### B.1 C Program - Blink LED

```c
// LED register addresses
#define LED_BASE 0x50000000
#define LED_OUT  (*(volatile uint32_t *)(LED_BASE + 0x00))

void delay(int cycles) {
    for (volatile int i = 0; i < cycles; i++);
}

int main() {
    // Blink pattern
    while (1) {
        LED_OUT = 0xAA;  // Pattern 1
        delay(100000);
        
        LED_OUT = 0x55;  // Pattern 2
        delay(100000);
    }
    
    return 0;
}
```

### B.2 Rust Program - LED Patterns

```rust
// LED register address
const LED_OUT: *mut u32 = 0x50000000 as *mut u32;

fn delay(cycles: usize) {
    for _ in 0..cycles {
        unsafe { core::ptr::read_volatile(&0 as *const i32); }
    }
}

fn main() {
    unsafe {
        loop {
            // Light up all LEDs
            LED_OUT.write_volatile(0xFF);
            delay(100000);
            
            // Turn off all LEDs
            LED_OUT.write_volatile(0x00);
            delay(100000);
            
            // Alternating pattern
            LED_OUT.write_volatile(0xAA);
            delay(100000);
            LED_OUT.write_volatile(0x55);
            delay(100000);
        }
    }
}
```

### B.3 Assembly - Toggle Single LED

```asm
.section .text
.global _start

_start:
    # Load LED base address
    lui  t0, 0x50000      # LED_BASE = 0x50000000
    
loop:
    # Toggle LED[0]
    lw   t2, 0(t0)        # Read LED_OUT
    xori t2, t2, 0x01     # Toggle bit 0
    sw   t2, 0(t0)        # Write LED_OUT
    
    # Delay
    li   t3, 100000
delay_loop:
    addi t3, t3, -1
    bnez t3, delay_loop
    
    # Repeat
    j    loop
```

---

## Appendix C: Verification Checklist

**Pre-Implementation:**
- [x] Research document reviewed
- [x] Architecture decisions documented
- [x] Memory map designed
- [x] Interface specifications complete

**RTL Implementation:**
- [ ] LED module created
- [ ] Wrapper module created
- [ ] Verilator lint passes (led_controller.sv)
- [ ] Verilator lint passes (top_with_peripherals.sv)
- [ ] No synthesis warnings

**Rust Integration:**
- [ ] Address constants added
- [ ] SystemBus routing updated
- [ ] Simulator bindings updated
- [ ] Build succeeds after `cargo clean`

**Testing:**
- [ ] Basic register read/write test passes
- [ ] GPIO output control test passes
- [ ] LED input read test passes
- [ ] Direction control test passes
- [ ] All existing tests still pass
- [ ] No clippy warnings

**Documentation:**
- [ ] Implementation plan complete (this document)
- [ ] AGENTS.md updated
- [ ] Example programs provided
- [ ] Memory map documented

**Code Quality:**
- [ ] Rust code formatted (`cargo fmt`)
- [ ] Clippy warnings auto-fixed (`cargo clippy --fix`)
- [ ] Zero clippy warnings (`cargo clippy -- -D warnings`)
- [ ] RTL style consistent with existing code

**Deliverables:**
- [ ] `rtl/peripherals/led_controller.sv`
- [ ] `rtl/top_with_peripherals.sv`
- [ ] Updated `riscv_shared/src/bus.rs`
- [ ] Updated `cpu-sim/src/bus.rs`
- [ ] Test file `testbench/tests/led_test.rs`
- [ ] This implementation plan document

---

**End of Implementation Plan**

This plan is ready for execution by an AI coding agent or human developer. Follow the checklist sequentially, validate each phase before proceeding to the next.

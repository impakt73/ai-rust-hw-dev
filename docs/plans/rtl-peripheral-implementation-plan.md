# RTL Peripheral Implementation Plan
## Hybrid/Split Address Space Approach with GPIO Example

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
3. **Concrete implementation** of a GPIO peripheral as a reference example
4. **Integration strategy** for future RTL peripherals

**Key Decision:** Use **Split Bus (Mechanism 3)** approach with:
- RTL peripherals in address range **0x30000000 - 0x3FFFFFFF**
- Rust peripherals remain in **0x40000000 - 0x7FFFFFFF**
- DRAM unchanged at **0x80000000 - 0xFFFFFFFF**

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Memory Map Design](#2-memory-map-design)
3. [RTL Implementation Plan](#3-rtl-implementation-plan)
4. [GPIO Peripheral Specification](#4-gpio-peripheral-specification)
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
│  │   (top.sv)        │  │  - gpio.sv            │     │
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
0x00000000-0x2FFFFFFF | Reserved         | -    | 768 MiB | Reserved for future use
0x30000000-0x300000FF | SimControl*      | RTL? | 256 B   | Simulation control (TBD)
0x31000000-0x3100000F | GPIO             | RTL  | 16 B    | 8-bit GPIO controller
0x32000000-0x320000FF | Timer (future)   | RTL  | 256 B   | Programmable timer
0x33000000-0x330000FF | UART (future)    | RTL  | 256 B   | UART controller
0x34000000-0x340000FF | SPI (future)     | RTL  | 256 B   | SPI master
0x35000000-0x350000FF | I2C (future)     | RTL  | 256 B   | I2C master
0x36000000-0x3FFFFFFF | Reserved (RTL)   | RTL  | ~250 MB | Reserved for future RTL
0x40000000-0x40000007 | FIFO             | Rust | 8 B     | Host communication FIFO
0x50000000-0x50000013 | DMA              | Rust | 20 B    | DMA controller
0x60000000-0x6000000F | Video            | Rust | 16 B    | Video frame buffer
0x70000000-0x7000000F | Audio            | Rust | 16 B    | Audio buffer
0x71000000-0x7FFFFFFF | Reserved (Rust)  | Rust | ~250 MB | Reserved for future Rust
0x80000000-0xFFFFFFFF | DRAM             | Both | 2 GiB   | System memory

* SimControl could stay in Rust (0x30000000) or move to RTL - TBD during implementation
```

### 2.2 Address Decoding Strategy

**In RTL (top_with_peripherals.sv):**

```systemverilog
// Address range definitions
localparam RTL_PERIPH_BASE  = 32'h30000000;
localparam RTL_PERIPH_LIMIT = 32'h40000000;

localparam GPIO_BASE = 32'h31000000;
localparam GPIO_SIZE = 32'h00000010;  // 16 bytes

// Decode address to peripheral select
logic sel_gpio;
logic sel_external;  // For Rust peripherals + DRAM

assign sel_gpio     = (cpu_dmem_addr >= GPIO_BASE) && 
                      (cpu_dmem_addr < GPIO_BASE + GPIO_SIZE);
assign sel_external = (cpu_dmem_addr < RTL_PERIPH_BASE) || 
                      (cpu_dmem_addr >= RTL_PERIPH_LIMIT);
```

**In Rust (cpu-sim/src/bus.rs):**

```rust
// Constants in riscv_shared/src/bus.rs
pub const RTL_PERIPH_BASE: u32  = 0x30000000;
pub const RTL_PERIPH_LIMIT: u32 = 0x40000000;

pub const GPIO_BASE: u32 = 0x31000000;
pub const GPIO_SIZE: u32 = 0x00000010;

// In SystemBus routing logic
fn route_address(&self, addr: u32) -> RoutingTarget {
    if addr >= RTL_PERIPH_BASE && addr < RTL_PERIPH_LIMIT {
        RoutingTarget::RtlPeripheral  // Forward to RTL
    } else if addr >= DRAM_BASE {
        RoutingTarget::Dram
    } else {
        RoutingTarget::RustPeripheral  // FIFO, DMA, Video, etc.
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
    ├── gpio.sv                     # NEW: GPIO peripheral
    ├── timer.sv                    # FUTURE
    └── uart.sv                     # FUTURE
```

### 3.2 Implementation Phases

**Phase 1: GPIO Peripheral (This Plan)**
- ✅ Create `rtl/peripherals/gpio.sv`
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
2. **Instantiate RTL peripherals** (gpio.sv, etc.)
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
    
    // GPIO pins (exposed to top level for FPGA synthesis)
    output logic [7:0]  gpio_out,
    input  logic [7:0]  gpio_in,
    output logic [7:0]  gpio_dir,  // 1=output, 0=input
    
    // ... debug signals same as top.sv ...
);
```

---

## 4. GPIO Peripheral Specification

### 4.1 Overview

**Purpose:** Control 8 external GPIO pins for LED control, button input, or general I/O.

**Features:**
- 8 bidirectional GPIO pins
- Individually configurable direction (input/output)
- Output data register
- Input data register (read-only, reflects pin state)
- Direction control register

### 4.2 Register Map

```
Offset | Name      | Access | Reset  | Description
-------|-----------|--------|--------|---------------------------------------
0x00   | GPIO_OUT  | RW     | 0x00   | Output data register (write to pins)
0x04   | GPIO_IN   | RO     | 0x00   | Input data register (read from pins)
0x08   | GPIO_DIR  | RW     | 0x00   | Direction: 1=output, 0=input
0x0C   | Reserved  | -      | -      | Reserved for future use
```

**Address Calculation:**
```
GPIO_OUT_ADDR = 0x31000000 + 0x00 = 0x31000000
GPIO_IN_ADDR  = 0x31000000 + 0x04 = 0x31000004
GPIO_DIR_ADDR = 0x31000000 + 0x08 = 0x31000008
```

### 4.3 Register Descriptions

**GPIO_OUT (0x31000000):**
- **Bits [7:0]**: Output data for GPIO pins
- **Bits [31:8]**: Reserved (read as 0, writes ignored)
- **Behavior**: When GPIO_DIR[n] = 1, GPIO_OUT[n] drives the pin
- **Reset value**: 0x00000000

**GPIO_IN (0x31000004):**
- **Bits [7:0]**: Current state of GPIO pins (sampled)
- **Bits [31:8]**: Reserved (read as 0)
- **Behavior**: Always reflects pin state, regardless of direction
- **Reset value**: 0x00000000
- **Note**: Read-only register

**GPIO_DIR (0x31000008):**
- **Bits [7:0]**: Direction control
  - 1 = Output (pin driven by GPIO_OUT)
  - 0 = Input (pin high-impedance, read via GPIO_IN)
- **Bits [31:8]**: Reserved (read as 0, writes ignored)
- **Reset value**: 0x00000000 (all inputs by default)

### 4.4 Timing Characteristics

**Read Operations:**
- **Latency**: 1 cycle (ready = 1'b1)
- **Behavior**: Combinational read from register

**Write Operations:**
- **Latency**: 1 cycle (ready = 1'b1)
- **Behavior**: Register update on next clock edge

**Pin Update Timing:**
- **Output propagation**: Output pins updated 1 clock cycle after write
- **Input sampling**: Input pins sampled continuously (asynchronous)
- **Metastability**: Input pins synchronized with 2-FF synchronizer (optional, not in initial version)

### 4.5 Interface Signals

**Module Interface:**

```systemverilog
module gpio (
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
    
    // GPIO pins (bidirectional)
    output logic [7:0]  gpio_out,  // Output data to pins
    input  logic [7:0]  gpio_in,   // Input data from pins
    output logic [7:0]  gpio_dir   // Direction: 1=output, 0=input
);
```

**Pin Mapping (for FPGA):**

On FPGA, these signals would be connected to:
- `gpio_out[7:0]` → LED outputs or external pins
- `gpio_in[7:0]` → Button inputs or external pins
- `gpio_dir[7:0]` → Tri-state buffer control

**Example FPGA Top-Level:**

```systemverilog
module fpga_top (
    input  logic       clk_100mhz,
    input  logic       rst_btn_n,
    
    // LED outputs
    output logic [7:0] led,
    
    // External I/O (if bidirectional)
    inout  wire  [7:0] gpio_pins
);
    // CPU and peripheral logic
    logic [7:0] gpio_out, gpio_in, gpio_dir;
    
    top_with_peripherals cpu_system (
        .clk(clk_100mhz),
        .rst_n(rst_btn_n),
        .gpio_out(gpio_out),
        .gpio_in(gpio_in),
        .gpio_dir(gpio_dir),
        // ... other connections ...
    );
    
    // Simple: Direct LED output (ignore bidirectional for now)
    assign led = gpio_out;
    
    // Advanced: Bidirectional with tri-state (for future FPGA deployment)
    // genvar i;
    // generate
    //     for (i = 0; i < 8; i++) begin
    //         assign gpio_pins[i] = gpio_dir[i] ? gpio_out[i] : 1'bz;
    //         assign gpio_in[i]   = gpio_pins[i];
    //     end
    // endgenerate
endmodule
```

### 4.6 Functional Behavior

**Operation Modes:**

1. **Output Mode** (GPIO_DIR[n] = 1):
   - Pin driven by GPIO_OUT[n]
   - Writes to GPIO_OUT[n] update pin after 1 clock cycle
   - GPIO_IN[n] reads back the output value

2. **Input Mode** (GPIO_DIR[n] = 0):
   - Pin is high-impedance (tri-state)
   - GPIO_IN[n] reflects external pin state
   - Writes to GPIO_OUT[n] stored but not driven to pin

**Example Usage:**

```c
// Set GPIO[7:4] as outputs, GPIO[3:0] as inputs
*(volatile uint32_t *)0x31000008 = 0xF0;

// Set outputs high
*(volatile uint32_t *)0x31000000 = 0xF0;

// Read inputs
uint32_t inputs = *(volatile uint32_t *)0x31000004;
uint8_t button_state = inputs & 0x0F;
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
logic sel_gpio;
logic sel_external;

// GPIO range: 0x31000000 - 0x3100000F
assign sel_gpio = (cpu_dmem_addr[31:4] == 28'h3100000);

// External: anything not in RTL peripheral space
// RTL peripheral space: 0x30000000 - 0x3FFFFFFF (top 8 bits = 0x30-0x3F)
assign sel_external = (cpu_dmem_addr[31:28] < 4'h3) || 
                      (cpu_dmem_addr[31:28] > 4'h3);
```

**Alternative (more explicit):**

```systemverilog
always_comb begin
    sel_gpio     = 1'b0;
    sel_external = 1'b0;
    
    // Check if address is in GPIO range
    if (cpu_dmem_addr >= 32'h31000000 && cpu_dmem_addr < 32'h31000010) begin
        sel_gpio = 1'b1;
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
    if (sel_gpio) begin
        cpu_dmem_rdata = gpio_rdata;
        cpu_dmem_ready = gpio_ready;
    end else if (sel_external) begin
        cpu_dmem_rdata = ext_mem_rdata;
        cpu_dmem_ready = ext_mem_ready;
    end else begin
        // Invalid address - return zero and ready immediately
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
    
    // GPIO pins
    output logic [7:0]  gpio_out,
    input  logic [7:0]  gpio_in,
    output logic [7:0]  gpio_dir,
    
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
    logic [31:0] gpio_rdata;
    logic        gpio_ready;
    
    // Address decoder
    logic sel_gpio;
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
    
    // Instantiate GPIO peripheral
    gpio gpio_peripheral (
        .clk(clk),
        .rst_n(rst_n),
        .addr(cpu_dmem_addr),
        .wdata(cpu_dmem_wdata),
        .rdata(gpio_rdata),
        .we(cpu_dmem_we && sel_gpio),
        .re(cpu_dmem_re && sel_gpio),
        .size(cpu_dmem_size),
        .ready(gpio_ready),
        .gpio_out(gpio_out),
        .gpio_in(gpio_in),
        .gpio_dir(gpio_dir)
    );
    
    // Address decoder
    assign sel_gpio = (cpu_dmem_addr[31:4] == 28'h3100000);  // 0x31000000 - 0x3100000F
    assign sel_external = !sel_gpio && (cpu_dmem_addr[31:28] != 4'h3 || 
                                        cpu_dmem_addr[31:24] == 8'h30);
    
    // Response multiplexer
    always_comb begin
        if (sel_gpio) begin
            cpu_dmem_rdata = gpio_rdata;
            cpu_dmem_ready = gpio_ready;
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
1. `riscv_shared/src/bus.rs` - Add GPIO address constants
2. `cpu-sim/src/lib.rs` - Update Verilator bindings to use new wrapper
3. `cpu-sim/src/bus.rs` - Add routing logic for RTL peripheral range
4. Integration tests - Add GPIO tests

### 6.2 Address Constants

**In `riscv_shared/src/bus.rs`:**

```rust
// RTL Peripheral Address Space
pub const RTL_PERIPH_BASE: u32  = 0x30000000;
pub const RTL_PERIPH_LIMIT: u32 = 0x40000000;

// GPIO Peripheral
pub const GPIO_BASE: u32 = 0x31000000;
pub const GPIO_SIZE: u32 = 0x00000010;  // 16 bytes

// GPIO Register Offsets
pub const GPIO_OUT_OFFSET: u32 = 0x00;
pub const GPIO_IN_OFFSET: u32  = 0x04;
pub const GPIO_DIR_OFFSET: u32 = 0x08;

// Helper functions
pub fn gpio_out_addr() -> u32 { GPIO_BASE + GPIO_OUT_OFFSET }
pub fn gpio_in_addr() -> u32  { GPIO_BASE + GPIO_IN_OFFSET }
pub fn gpio_dir_addr() -> u32 { GPIO_BASE + GPIO_DIR_OFFSET }
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
3. Expose GPIO pin signals for testing

**Conceptual Change:**

```rust
// OLD: Direct connection to top.sv
// self.core.dmem_addr, dmem_rdata, etc.

// NEW: Connection to top_with_peripherals.sv
// self.core.ext_mem_addr, ext_mem_rdata, etc.
// GPIO signals: self.core.gpio_out, gpio_in, gpio_dir
```

**Note:** The exact implementation depends on how Marlin generates bindings. The wrapper module should expose the same interface pattern, just with `ext_mem_*` prefix instead of `dmem_*`.

### 6.5 Verilator Build Configuration

**Potential changes to build process:**

1. **Include new files** in Verilator compilation:
   ```
   rtl/top_with_peripherals.sv  (new top-level)
   rtl/peripherals/gpio.sv      (new peripheral)
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
│  - GPIO read/write from CPU        │
│  - LED control program              │
└────────────────────────────────────┘
┌────────────────────────────────────┐
│  Unit Tests (RTL Testbench)        │  ← Optional Verilator tests
│  - GPIO register access            │
│  - Pin control behavior            │
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
# Lint GPIO module
verilator --lint-only rtl/peripherals/gpio.sv

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
// rtl/peripherals/gpio_tb.sv
module gpio_tb;
    // Test signals
    logic clk, rst_n;
    logic [31:0] addr, wdata, rdata;
    logic we, re, ready;
    logic [7:0] gpio_out, gpio_in, gpio_dir;
    
    // Instantiate DUT
    gpio dut (.*);
    
    // Clock generation
    initial clk = 0;
    always #5 clk = ~clk;
    
    // Test cases
    initial begin
        // Reset
        rst_n = 0; #20; rst_n = 1;
        
        // Test 1: Write to GPIO_OUT
        @(posedge clk);
        addr = 32'h31000000; wdata = 32'hAA; we = 1; re = 0;
        @(posedge clk);
        we = 0;
        
        // Test 2: Read GPIO_IN
        @(posedge clk);
        addr = 32'h31000004; we = 0; re = 1;
        @(posedge clk);
        assert(rdata[7:0] == gpio_in);
        
        // ... more tests ...
        
        $display("All tests passed!");
        $finish;
    end
endmodule
```

### 7.4 Integration Tests (Rust)

**Primary testing approach for this project.**

**Test File:** `testbench/tests/gpio_test.rs`

**Test Cases:**

1. **Basic Register Access**
   - Write to GPIO_OUT, verify via direct signal inspection
   - Read from GPIO_IN after setting input
   - Configure GPIO_DIR and verify behavior

2. **LED Control Pattern**
   - Write sequence to GPIO_OUT
   - Verify output pattern matches

3. **Direction Control**
   - Set GPIO_DIR to various patterns
   - Verify output enable behavior

4. **Edge Cases**
   - Write to invalid offsets (should not crash)
   - Read from reserved registers
   - Byte/halfword access (if supported)

**Example Test Structure:**

```rust
#[test]
fn test_gpio_basic_write_read() {
    // Create simulator with new wrapper
    let mut sim = create_simulator();
    
    // Reset
    sim.reset();
    
    // Write 0xAA to GPIO_OUT (0x31000000)
    sim.write_word(gpio_out_addr(), 0xAA);
    sim.step();
    
    // Verify gpio_out signal
    assert_eq!(sim.core.gpio_out, 0xAA, "GPIO output mismatch");
    
    // Set gpio_dir to all outputs
    sim.write_word(gpio_dir_addr(), 0xFF);
    sim.step();
    assert_eq!(sim.core.gpio_dir, 0xFF);
    
    // Simulate external input
    sim.core.gpio_in = 0x55;
    sim.step();
    
    // Read GPIO_IN (0x31000004)
    let input_val = sim.read_word(gpio_in_addr());
    assert_eq!(input_val & 0xFF, 0x55, "GPIO input mismatch");
}

#[test]
fn test_gpio_led_pattern() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Configure all as outputs
    sim.write_word(gpio_dir_addr(), 0xFF);
    
    // Test LED patterns
    let patterns = [0x00, 0xFF, 0xAA, 0x55, 0x0F, 0xF0];
    for pattern in patterns {
        sim.write_word(gpio_out_addr(), pattern);
        sim.step();
        assert_eq!(sim.core.gpio_out, pattern, 
                   "LED pattern 0x{:02X} failed", pattern);
    }
}

#[test]
fn test_gpio_direction_control() {
    let mut sim = create_simulator();
    sim.reset();
    
    // Test: Outputs disabled by default (dir = 0)
    sim.write_word(gpio_out_addr(), 0xFF);
    sim.step();
    assert_eq!(sim.core.gpio_dir, 0x00, "Should default to inputs");
    
    // Enable outputs
    sim.write_word(gpio_dir_addr(), 0xFF);
    sim.step();
    assert_eq!(sim.core.gpio_out, 0xFF);
    assert_eq!(sim.core.gpio_dir, 0xFF);
}
```

### 7.5 Test Execution Plan

**Step-by-step testing:**

1. **Lint RTL** - Verify no syntax/style issues
   ```bash
   verilator --lint-only rtl/peripherals/gpio.sv
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
   cargo test gpio  # Run GPIO-specific tests
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

- [ ] **Create GPIO module** (`rtl/peripherals/gpio.sv`)
  - [ ] Define module interface
  - [ ] Implement register file (GPIO_OUT, GPIO_IN, GPIO_DIR)
  - [ ] Implement address decoder
  - [ ] Implement read/write logic
  - [ ] Add proper reset behavior
  - [ ] Lint with Verilator

- [ ] **Create wrapper module** (`rtl/top_with_peripherals.sv`)
  - [ ] Define module interface with ext_mem_* signals
  - [ ] Instantiate CPU core (top.sv)
  - [ ] Instantiate GPIO peripheral
  - [ ] Implement address decoder
  - [ ] Implement response multiplexer
  - [ ] Connect external bus forwarding
  - [ ] Lint with Verilator

### Phase 2: Rust Integration

- [ ] **Update address constants** (`riscv_shared/src/bus.rs`)
  - [ ] Add RTL_PERIPH_BASE, RTL_PERIPH_LIMIT
  - [ ] Add GPIO_BASE, GPIO_SIZE
  - [ ] Add GPIO register offset constants
  - [ ] Add helper functions

- [ ] **Update SystemBus** (`cpu-sim/src/bus.rs`)
  - [ ] Add is_rtl_peripheral() method
  - [ ] Update read_word() with RTL check
  - [ ] Update write_word() with RTL check
  - [ ] Update documentation

- [ ] **Update simulator** (`cpu-sim/src/lib.rs` or equivalent)
  - [ ] Change Verilator module to top_with_peripherals
  - [ ] Update memory interface signals (dmem → ext_mem)
  - [ ] Expose GPIO pin signals
  - [ ] Update tests helper functions if needed

- [ ] **Build configuration**
  - [ ] Update Cargo.toml or marlin config (if needed)
  - [ ] Verify include paths for new RTL files
  - [ ] Test clean build: `cargo clean && cargo build`

### Phase 3: Testing

- [ ] **Linting**
  - [ ] `verilator --lint-only rtl/peripherals/gpio.sv` → Pass
  - [ ] `verilator --lint-only rtl/top_with_peripherals.sv` → Pass

- [ ] **Integration tests** (`testbench/tests/gpio_test.rs`)
  - [ ] Test: Basic register read/write
  - [ ] Test: GPIO_OUT updates output pins
  - [ ] Test: GPIO_IN reads input pins
  - [ ] Test: GPIO_DIR controls direction
  - [ ] Test: LED pattern sequence
  - [ ] Test: Invalid address handling

- [ ] **Regression tests**
  - [ ] `cargo test` → All existing tests still pass
  - [ ] No degradation in other functionality

- [ ] **Code quality**
  - [ ] `cargo fmt` → Format Rust code
  - [ ] `cargo clippy --fix --allow-dirty` → Auto-fix warnings
  - [ ] `cargo clippy -- -D warnings` → Zero warnings

### Phase 4: Documentation

- [ ] **Update AGENTS.md**
  - [ ] Add memory map with GPIO peripheral
  - [ ] Document RTL peripheral integration process
  - [ ] Add GPIO usage examples

- [ ] **Create peripheral documentation**
  - [ ] GPIO register map reference
  - [ ] GPIO usage examples (C/Rust)
  - [ ] FPGA synthesis notes

- [ ] **Update README** (if applicable)
  - [ ] Mention hybrid RTL/Rust peripheral architecture
  - [ ] Link to peripheral documentation

### Phase 5: Validation

- [ ] **Simulation validation**
  - [ ] Run full test suite: `cargo test --verbose`
  - [ ] Verify GPIO behavior in longer programs
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

### 9.2 Enhanced GPIO Features

**Future GPIO improvements:**

- **Interrupt support**: Edge-triggered interrupts on input pins
- **Pin configuration**: Pull-up/pull-down resistors
- **Input synchronization**: 2-FF synchronizer for metastability
- **Atomic operations**: Set/clear/toggle registers
- **Pin multiplexing**: Alternate function select
- **Drive strength**: Configurable output drive

**Example enhanced register map:**

```
0x00  GPIO_OUT       Output data
0x04  GPIO_IN        Input data
0x08  GPIO_DIR       Direction control
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
5. **Constrain GPIO pins** (XDC constraints file)
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
- `rtl/peripherals/gpio.sv` - GPIO peripheral module
- `rtl/top_with_peripherals.sv` - Wrapper with peripheral integration

**RTL Files (Unchanged):**
- `rtl/top.sv` - CPU core (no modifications)
- `rtl/alu.sv`, `rtl/decoder.sv`, etc. - CPU submodules

**Rust Files (Modified):**
- `riscv_shared/src/bus.rs` - Address constants
- `cpu-sim/src/bus.rs` - Bus routing logic
- `cpu-sim/src/lib.rs` - Verilator bindings
- `testbench/tests/gpio_test.rs` - New test file

### 10.2 Memory Map Quick Reference

```
Address       | Device      | Registers
--------------|-------------|------------------------------------
0x31000000    | GPIO        | OUT (RW)
0x31000004    | GPIO        | IN (RO)
0x31000008    | GPIO        | DIR (RW)
0x40000000    | FIFO        | DATA, STATUS (Rust)
0x80000000+   | DRAM        | System memory (Rust)
```

### 10.3 Command Reference

```bash
# Lint RTL
verilator --lint-only rtl/peripherals/gpio.sv
verilator --lint-only rtl/top_with_peripherals.sv

# Build system
cargo clean             # Clear Verilator cache
cargo build             # Build with new RTL
cargo build --release   # Optimized build

# Test
cargo test gpio         # GPIO-specific tests
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

## Appendix A: Complete GPIO Register Map

```
Register   | Offset | Access | Reset      | Description
-----------|--------|--------|------------|----------------------------------
GPIO_OUT   | 0x00   | RW     | 0x00000000 | Output data register
           |        |        |            |   [7:0]   - Output data
           |        |        |            |   [31:8]  - Reserved (0)
-----------|--------|--------|------------|----------------------------------
GPIO_IN    | 0x04   | RO     | 0x00000000 | Input data register
           |        |        |            |   [7:0]   - Input data (pin state)
           |        |        |            |   [31:8]  - Reserved (0)
-----------|--------|--------|------------|----------------------------------
GPIO_DIR   | 0x08   | RW     | 0x00000000 | Direction control register
           |        |        |            |   [7:0]   - Direction (1=out, 0=in)
           |        |        |            |   [31:8]  - Reserved (0)
-----------|--------|--------|------------|----------------------------------
Reserved   | 0x0C   | -      | -          | Reserved for future use
```

**Access:**
- **RW** = Read/Write
- **RO** = Read-Only

**Addresses:**
- GPIO_OUT: 0x31000000
- GPIO_IN:  0x31000004
- GPIO_DIR: 0x31000008

---

## Appendix B: Example Programs

### B.1 C Program - Blink LED

```c
// GPIO register addresses
#define GPIO_BASE 0x31000000
#define GPIO_OUT  (*(volatile uint32_t *)(GPIO_BASE + 0x00))
#define GPIO_IN   (*(volatile uint32_t *)(GPIO_BASE + 0x04))
#define GPIO_DIR  (*(volatile uint32_t *)(GPIO_BASE + 0x08))

void delay(int cycles) {
    for (volatile int i = 0; i < cycles; i++);
}

int main() {
    // Configure all GPIO as outputs
    GPIO_DIR = 0xFF;
    
    // Blink pattern
    while (1) {
        GPIO_OUT = 0xAA;  // Pattern 1
        delay(100000);
        
        GPIO_OUT = 0x55;  // Pattern 2
        delay(100000);
    }
    
    return 0;
}
```

### B.2 Rust Program - Button + LED

```rust
// GPIO register addresses
const GPIO_OUT: *mut u32 = 0x31000000 as *mut u32;
const GPIO_IN:  *mut u32 = 0x31000004 as *mut u32;
const GPIO_DIR: *mut u32 = 0x31000008 as *mut u32;

fn main() {
    unsafe {
        // Configure GPIO[7:4] as outputs (LEDs)
        // Configure GPIO[3:0] as inputs (buttons)
        GPIO_DIR.write_volatile(0xF0);
        
        loop {
            // Read button state
            let buttons = GPIO_IN.read_volatile() & 0x0F;
            
            // Echo buttons to LEDs (shifted up)
            let leds = buttons << 4;
            GPIO_OUT.write_volatile(leds);
        }
    }
}
```

### B.3 Assembly - Toggle Single LED

```asm
.section .text
.global _start

_start:
    # Load GPIO base address
    lui  t0, 0x31000      # GPIO_BASE = 0x31000000
    
    # Configure GPIO[0] as output
    li   t1, 0x01
    sw   t1, 8(t0)        # GPIO_DIR = 0x01
    
loop:
    # Toggle GPIO[0]
    lw   t2, 0(t0)        # Read GPIO_OUT
    xori t2, t2, 0x01     # Toggle bit 0
    sw   t2, 0(t0)        # Write GPIO_OUT
    
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
- [ ] GPIO module created
- [ ] Wrapper module created
- [ ] Verilator lint passes (gpio.sv)
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
- [ ] GPIO input read test passes
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
- [ ] `rtl/peripherals/gpio.sv`
- [ ] `rtl/top_with_peripherals.sv`
- [ ] Updated `riscv_shared/src/bus.rs`
- [ ] Updated `cpu-sim/src/bus.rs`
- [ ] Test file `testbench/tests/gpio_test.rs`
- [ ] This implementation plan document

---

**End of Implementation Plan**

This plan is ready for execution by an AI coding agent or human developer. Follow the checklist sequentially, validate each phase before proceeding to the next.

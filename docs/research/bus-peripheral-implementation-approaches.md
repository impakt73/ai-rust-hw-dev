# Bus Device and Memory-Mapped Peripheral Implementation Approaches

**Author:** GitHub Copilot Hardware-Software Integration Architect  
**Date:** January 27, 2026  
**Project:** RISC-V RV32IMACF Multi-Cycle CPU with Rust-Based Verification  
**Target:** FPGA prototyping and Verilator simulation

---

## Executive Summary

This document analyzes approaches for implementing bus devices and memory-mapped peripherals in a RISC-V CPU verification environment. It compares RTL (SystemVerilog) implementation versus simulation harness (Rust) implementation, evaluates hybrid approaches, and provides concrete recommendations based on the existing architecture of the `ai-rust-hw-dev` project.

**Key Findings:**
- **Current Architecture:** The project already implements a sophisticated bus infrastructure in Rust (cpu-sim) with multiple working peripherals (DRAM, FIFO, DMA, Video, Audio, SimControl).
- **RTL Approach:** Best for FPGA synthesis and cycle-accurate hardware validation.
- **Rust Simulation Approach:** Best for rapid prototyping, software development, and complex behavioral peripherals.
- **Hybrid Approach:** Optimal for this project - implement simple, synthesizable peripherals in RTL, complex peripherals in Rust.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Current Architecture Analysis](#2-current-architecture-analysis)
3. [RTL-Based Peripheral Implementation](#3-rtl-based-peripheral-implementation)
4. [Rust Simulation-Based Peripheral Implementation](#4-rust-simulation-based-peripheral-implementation)
5. [Hybrid Implementation Approaches](#5-hybrid-implementation-approaches)
6. [Comparison Matrix](#6-comparison-matrix)
7. [Implementation Guidelines](#7-implementation-guidelines)
8. [Recommendations](#8-recommendations)
9. [References](#9-references)

---

## 1. Introduction

### 1.1 Problem Statement

The RISC-V CPU project requires a flexible mechanism for implementing memory-mapped peripherals that can:
- Support FPGA synthesis for hardware validation
- Enable rapid prototyping and testing
- Allow software development before hardware is ready
- Maintain consistency between simulation and hardware
- Scale to complex peripherals (DMA, video controllers, network interfaces)

### 1.2 Design Constraints

**Hardware Constraints:**
- Multi-cycle non-pipelined RISC-V RV32IMACF CPU
- 12-state FSM architecture (IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, HALT, ATOMIC_RMW)
- Variable-latency memory interface with ready/valid handshaking
- Separate instruction and data memory ports
- Memory address range: 0x80000000 - 0xFFFFFFFF (DRAM)

**Software Constraints:**
- Verilator-based simulation framework (marlin crate)
- Rust verification harness with type safety
- 264 tests across RTL, verification, and utilities
- Support for bare-metal RISC-V programs

---

## 2. Current Architecture Analysis

### 2.1 RTL Architecture

The CPU's RTL implementation (`rtl/top.sv`) exposes two primary memory interfaces:

```systemverilog
// Instruction memory interface (exposed to testbench)
output logic [31:0] imem_addr,
input  logic [31:0] imem_data,
output logic        imem_req,     // Request instruction fetch
input  logic        imem_ready,   // Memory has valid data

// Data memory interface (exposed to testbench)
output logic [31:0] dmem_addr,
output logic [31:0] dmem_wdata,
input  logic [31:0] dmem_rdata,
output logic        dmem_we,
output logic        dmem_re,      // Memory read enable
output logic [1:0]  dmem_size,    // 00=byte, 01=halfword, 10=word
output logic        dmem_req,     // Request data memory operation
input  logic        dmem_ready,   // Memory operation complete
```

**Key Characteristics:**
- **Memory-first design:** The CPU doesn't handle address decoding internally
- **External bus management:** All peripheral routing happens outside the RTL
- **Simple interface:** Ready/valid handshaking for variable latency
- **Flexible integration:** Can connect to any memory system or bus fabric

### 2.2 Rust Simulation Bus Architecture

The simulation harness (`cpu-sim/src/bus.rs`) implements a sophisticated bus infrastructure:

```rust
pub struct SystemBus {
    pub memory: Memory,              // Shared memory (0x80000000 - 0xFFFFFFFF)
    pub dram: Dram,                  // Internal DRAM device
    pub fifo: Fifo,                  // Internal FIFO device
    pub sim_control: SimControl,     // Internal simulation control device
    external_devices: Vec<Box<dyn BusDevice>>,  // External devices
    memory_map: Vec<MemoryMapEntry>, // Address routing table
    elapsed_time_us: u64,            // Host time tracking
}
```

**Bus Device Trait:**
```rust
pub trait BusDevice {
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError>;
    fn write_word(&mut self, ctx: &mut SystemContext, offset: u32, value: u32) -> Result<(), BusDeviceError>;
    fn read_halfword(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u16, BusDeviceError>;
    fn write_halfword(&mut self, ctx: &mut SystemContext, offset: u32, value: u16) -> Result<(), BusDeviceError>;
    fn read_byte(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u8, BusDeviceError>;
    fn write_byte(&mut self, ctx: &mut SystemContext, offset: u32, value: u8) -> Result<(), BusDeviceError>;
    fn size(&self) -> u32;
    fn name(&self) -> &str;
    fn reset(&mut self, ctx: &mut SystemContext);
    fn clock_cycle(&mut self, ctx: &mut SystemContext);  // Multi-cycle operations
}
```

**Current Memory Map:**
```
0x30000000 - 0x30000003  SimControl (4 bytes)
0x40000000 - 0x40000007  FIFO (8 bytes)
0x80000000 - 0xFFFFFFFF  DRAM (2 GiB)
```

**Existing Peripherals:**
1. **DRAM:** Memory-backed storage forwarding to shared Memory
2. **FIFO:** Bidirectional queue for CPU ↔ Host communication
3. **SimControl:** Simulation control (exit codes, timing)
4. **Video:** Frame buffer controller with DMA-like multi-cycle reads
5. **Audio:** Audio buffer controller (similar to Video)
6. **DMA:** Memory-to-memory transfer controller (1 byte per cycle)

### 2.3 Strengths of Current Architecture

1. **Clean separation:** CPU RTL is agnostic to peripheral implementation
2. **Flexible registration:** External devices can be registered dynamically
3. **Type-safe Rust:** Trait-based design with compile-time guarantees
4. **Multi-cycle support:** `clock_cycle()` callback enables realistic timing
5. **DMA support:** Devices can access shared memory via `SystemContext`
6. **Proven design:** 264 tests validate the architecture

### 2.4 Limitations of Current Architecture

1. **No RTL peripherals:** All peripherals are in simulation only
2. **Not synthesizable:** Rust bus can't be used in FPGA
3. **No DPI integration:** No mechanism to call Rust from SystemVerilog
4. **Limited hardware validation:** Can't verify RTL peripheral timing

---

## 3. RTL-Based Peripheral Implementation

### 3.1 Approach Overview

RTL peripherals are implemented as SystemVerilog modules that respond to memory-mapped bus transactions. They integrate directly with the CPU's data memory interface.

### 3.2 Implementation Pattern

**Option A: Direct Integration (Simple Peripherals)**

For simple peripherals like GPIO or timers, directly extend the CPU's memory interface:

```systemverilog
module peripheral_controller (
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus interface (subset of CPU dmem interface)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic        re,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,
    
    // Peripheral-specific signals
    output logic [7:0]  gpio_out,
    input  logic [7:0]  gpio_in
);
    // Register map (relative to base address)
    localparam ADDR_GPIO_OUT    = 32'h0000;
    localparam ADDR_GPIO_IN     = 32'h0004;
    localparam ADDR_GPIO_DIR    = 32'h0008;
    
    logic [7:0] gpio_out_reg;
    logic [7:0] gpio_dir_reg;
    
    // Register writes
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            gpio_out_reg <= 8'h00;
            gpio_dir_reg <= 8'h00;
        end else if (we && req && ready) begin
            case (addr)
                ADDR_GPIO_OUT: gpio_out_reg <= wdata[7:0];
                ADDR_GPIO_DIR: gpio_dir_reg <= wdata[7:0];
            endcase
        end
    end
    
    // Register reads
    always_comb begin
        rdata = 32'h0;
        if (re && req) begin
            case (addr)
                ADDR_GPIO_OUT: rdata = {24'h0, gpio_out_reg};
                ADDR_GPIO_IN:  rdata = {24'h0, gpio_in};
                ADDR_GPIO_DIR: rdata = {24'h0, gpio_dir_reg};
            endcase
        end
    end
    
    // GPIO output (only for pins configured as output)
    assign gpio_out = gpio_out_reg & gpio_dir_reg;
    
    // Ready signal (combinational peripherals complete immediately)
    assign ready = 1'b1;
endmodule
```

**Option B: Bus Fabric Integration (Multiple Peripherals)**

For systems with multiple peripherals, implement a bus fabric with address decoding:

```systemverilog
module bus_fabric (
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (master)
    input  logic [31:0] cpu_addr,
    input  logic [31:0] cpu_wdata,
    output logic [31:0] cpu_rdata,
    input  logic        cpu_we,
    input  logic        cpu_re,
    input  logic [1:0]  cpu_size,
    input  logic        cpu_req,
    output logic        cpu_ready,
    
    // Peripheral interfaces (slaves)
    output logic [31:0] periph_addr[0:7],
    output logic [31:0] periph_wdata[0:7],
    input  logic [31:0] periph_rdata[0:7],
    output logic        periph_we[0:7],
    output logic        periph_re[0:7],
    output logic [1:0]  periph_size[0:7],
    output logic        periph_req[0:7],
    input  logic        periph_ready[0:7]
);
    // Address decode (example mapping)
    // 0x30000000 - 0x300000FF: Peripheral 0 (SimControl)
    // 0x40000000 - 0x400000FF: Peripheral 1 (UART/FIFO)
    // 0x50000000 - 0x500000FF: Peripheral 2 (GPIO)
    // 0x60000000 - 0x600000FF: Peripheral 3 (Timer)
    
    logic [2:0] selected_periph;
    
    // Address decoder
    always_comb begin
        selected_periph = 3'b111;  // Default: no peripheral selected
        
        if (cpu_addr[31:28] == 4'h3) selected_periph = 3'd0;
        else if (cpu_addr[31:28] == 4'h4) selected_periph = 3'd1;
        else if (cpu_addr[31:28] == 4'h5) selected_periph = 3'd2;
        else if (cpu_addr[31:28] == 4'h6) selected_periph = 3'd3;
    end
    
    // Route requests to selected peripheral
    genvar i;
    generate
        for (i = 0; i < 8; i++) begin : periph_routing
            assign periph_addr[i]  = cpu_addr;
            assign periph_wdata[i] = cpu_wdata;
            assign periph_we[i]    = (selected_periph == i) ? cpu_we : 1'b0;
            assign periph_re[i]    = (selected_periph == i) ? cpu_re : 1'b0;
            assign periph_size[i]  = cpu_size;
            assign periph_req[i]   = (selected_periph == i) ? cpu_req : 1'b0;
        end
    endgenerate
    
    // Multiplex responses
    assign cpu_rdata = periph_rdata[selected_periph];
    assign cpu_ready = periph_ready[selected_periph];
endmodule
```

### 3.3 Advantages of RTL Peripherals

1. **Cycle-accurate modeling:** Exact timing behavior, critical for hardware validation
2. **Synthesizable:** Can be deployed to FPGA for real hardware testing
3. **Bus protocol compliance:** Ensures compatibility with standard interfaces (AMBA, AXI)
4. **Seamless integration:** What you simulate is what gets synthesized
5. **Performance:** No overhead from software abstraction
6. **Hardware constraints:** Forces realistic implementation (no infinite buffers, etc.)

### 3.4 Disadvantages of RTL Peripherals

1. **Complexity:** Requires HDL expertise and deep understanding of digital design
2. **Slow simulation:** RTL simulators (even Verilator) are slower than pure software
3. **Limited flexibility:** Changes require recompilation of RTL
4. **Debugging difficulty:** Signal-level debugging is more challenging than software
5. **Development time:** Longer iteration cycles due to synthesis/simulation
6. **Limited abstraction:** Hard to implement complex behaviors (networking, file I/O)

### 3.5 When to Use RTL Peripherals

**Use RTL when:**
- Peripheral will be synthesized to FPGA/ASIC
- Cycle-accurate timing is critical
- Hardware resource constraints must be validated
- Interfacing with external hardware (FPGA I/O pins)
- Simple, well-defined peripherals (GPIO, SPI, I2C, timers)
- Bus protocol compliance must be verified

**Examples:**
- GPIO controller
- SPI master/slave
- I2C master/slave
- UART
- Timer/Counter
- Interrupt controller
- Simple DMA controller

---

## 4. Rust Simulation-Based Peripheral Implementation

### 4.1 Approach Overview

Rust peripherals are implemented as structs that implement the `BusDevice` trait. They live in the simulation harness and interact with the CPU through the `SystemBus`.

### 4.2 Implementation Pattern

**Example: Simple Timer Peripheral**

```rust
use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// Simple timer peripheral
///
/// Register Map:
/// - 0x00: COUNTER    - Current counter value (read-only)
/// - 0x04: PERIOD     - Timer period in cycles (read/write)
/// - 0x08: CONTROL    - Control register (read/write)
///   Bit 0: ENABLE (1 = running, 0 = stopped)
///   Bit 1: IRQ_ENABLE (1 = interrupt enabled)
/// - 0x0C: STATUS     - Status register (read/write)
///   Bit 0: IRQ_PENDING (write 1 to clear)
pub struct Timer {
    counter: u32,
    period: u32,
    control: u32,
    status: u32,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            counter: 0,
            period: 0,
            control: 0,
            status: 0,
        }
    }
    
    fn is_enabled(&self) -> bool {
        (self.control & 0x1) != 0
    }
    
    fn is_irq_enabled(&self) -> bool {
        (self.control & 0x2) != 0
    }
    
    fn set_irq_pending(&mut self) {
        self.status |= 0x1;
    }
    
    fn clear_irq_pending(&mut self) {
        self.status &= !0x1;
    }
}

impl BusDevice for Timer {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.counter),
            0x04 => Ok(self.period),
            0x08 => Ok(self.control),
            0x0C => Ok(self.status),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }
    
    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // COUNTER is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            0x04 => {
                self.period = value;
                Ok(())
            }
            0x08 => {
                self.control = value & 0x3;  // Only 2 bits used
                Ok(())
            }
            0x0C => {
                // Writing 1 to IRQ_PENDING clears it
                if (value & 0x1) != 0 {
                    self.clear_irq_pending();
                }
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }
    
    fn size(&self) -> u32 {
        16  // 4 registers × 4 bytes
    }
    
    fn name(&self) -> &str {
        "Timer"
    }
    
    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.counter = 0;
        self.period = 0;
        self.control = 0;
        self.status = 0;
    }
    
    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        // Update timer state on each clock cycle
        if self.is_enabled() {
            self.counter = self.counter.wrapping_add(1);
            
            // Check if period expired
            if self.counter >= self.period && self.period != 0 {
                self.counter = 0;
                
                // Set interrupt pending if enabled
                if self.is_irq_enabled() {
                    self.set_irq_pending();
                }
            }
        }
    }
}
```

**Example: DMA Controller (Existing Implementation)**

The project already has an excellent example in `cpu-sim/src/dma.rs`:

```rust
pub struct Dma {
    src_addr: u32,
    dst_addr: u32,
    size: u32,
    active_transfer: Option<ActiveTransfer>,  // Latched transfer state
}

impl BusDevice for Dma {
    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Transfer one byte per clock cycle if a transfer is in progress
        self.transfer_one_byte(ctx);
    }
}

fn transfer_one_byte(&mut self, ctx: &mut SystemContext) {
    let transfer = match self.active_transfer.as_mut() {
        Some(t) => t,
        None => return,
    };
    
    // Read one byte from source
    let byte = ctx.read_byte(transfer.current_src);
    
    // Write one byte to destination
    ctx.write_byte(transfer.current_dst, byte);
    
    // Update state
    transfer.current_src = transfer.current_src.wrapping_add(1);
    transfer.current_dst = transfer.current_dst.wrapping_add(1);
    transfer.bytes_remaining -= 1;
    
    if transfer.bytes_remaining == 0 {
        self.active_transfer = None;
    }
}
```

### 4.3 Advantages of Rust Simulation Peripherals

1. **Rapid development:** Faster iteration, no RTL recompilation
2. **Type safety:** Rust's ownership system prevents common bugs
3. **Rich ecosystem:** Access to Rust crates for complex behaviors
4. **Easy debugging:** Standard debuggers, logging, unit tests
5. **Complex behavior:** Can implement networking, file I/O, graphics
6. **Performance:** Faster than RTL simulation (especially for complex logic)
7. **Testability:** Easy to write unit tests for individual peripherals
8. **Flexibility:** Runtime configuration and dynamic behavior

### 4.4 Disadvantages of Rust Simulation Peripherals

1. **Not synthesizable:** Cannot be used in FPGA/ASIC
2. **Timing abstraction:** May not accurately model hardware timing
3. **Semantic gaps:** May miss corner cases that appear in real hardware
4. **No bus protocol validation:** Can't verify compliance with standards
5. **Software-centric:** May not reflect realistic hardware constraints
6. **Limited hardware validation:** Can't test RTL integration issues

### 4.5 When to Use Rust Simulation Peripherals

**Use Rust when:**
- Early software development (drivers, OS, applications)
- Complex behavioral peripherals (networking, graphics, storage)
- Rapid prototyping and exploration
- Peripheral will never be synthesized (simulation-only)
- Need access to host system (files, network, GUI)
- Testing software/hardware interaction patterns
- Performance testing with realistic workloads

**Examples:**
- Video frame buffer controller (existing: `video.rs`)
- Audio buffer controller (existing: `audio.rs`)
- DMA controller (existing: `dma.rs`)
- FIFO for host communication (existing: `fifo.rs`)
- Network interface simulator
- Storage controller (SD card, eMMC)
- USB controller simulator
- Display controller with GUI integration

---

## 5. Hybrid Implementation Approaches

### 5.1 Best of Both Worlds

The optimal approach for this project is a **hybrid architecture** that leverages both RTL and Rust peripherals:

```
┌─────────────────────────────────────────────────────┐
│                   Simulation Layer                   │
│                                                       │
│  ┌────────────────────────────────────────────────┐  │
│  │         Rust Simulation Harness (cpu-sim)      │  │
│  │                                                 │  │
│  │  ┌──────────────┐  ┌──────────────────────┐   │  │
│  │  │  SystemBus   │  │  Complex Peripherals  │   │  │
│  │  │              │  │  - Video              │   │  │
│  │  │  - Address   │  │  - Audio              │   │  │
│  │  │    Decoding  │  │  - Network            │   │  │
│  │  │  - Routing   │  │  - Storage            │   │  │
│  │  └──────┬───────┘  └──────────────────────┘   │  │
│  │         │                                       │  │
│  └─────────┼───────────────────────────────────────┘  │
│            │                                           │
└────────────┼───────────────────────────────────────────┘
             │
    ┌────────▼─────────┐
    │  Verilator FFI   │  ← Memory transactions
    └────────┬─────────┘
             │
┌────────────▼───────────────────────────────────────────┐
│                    Hardware Layer                       │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │         RISC-V CPU RTL (top.sv)                    │ │
│  │                                                     │ │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────────────┐  │ │
│  │  │ ALU  │  │ Regs │  │ CSR  │  │  Simple RTL  │  │ │
│  │  └──────┘  └──────┘  └──────┘  │  Peripherals │  │ │
│  │                                 │  - GPIO      │  │ │
│  │  ┌──────────────┐               │  - Timer     │  │ │
│  │  │ Mem Interface│               │  - UART      │  │ │
│  │  │ - imem       │               └──────────────┘  │ │
│  │  │ - dmem       │                                 │ │
│  │  └──────┬───────┘                                 │ │
│  └─────────┼─────────────────────────────────────────┘ │
│            │                                            │
└────────────┼────────────────────────────────────────────┘
             │
    External Memory/Peripherals
```

### 5.2 Integration Mechanisms

**Mechanism 1: External Bus Fabric (Current)**

The current approach routes all memory transactions through the Rust `SystemBus`:

```rust
// In cpu-sim simulator loop
fn step(&mut self) -> SimulationStepResult {
    // CPU requests memory access
    if self.core.dmem_req != 0 {
        let addr = self.core.dmem_addr;
        
        if self.core.dmem_we != 0 {
            // Write operation
            self.bus.write_word(addr, self.core.dmem_wdata);
            self.core.dmem_ready = 1;
        } else if self.core.dmem_re != 0 {
            // Read operation
            let data = self.bus.read_word(addr);
            self.core.dmem_rdata = data;
            self.core.dmem_ready = 1;
        }
    }
    
    // Tick CPU and all devices
    self.core.eval();
    self.bus.clock_cycle_all_devices();
}
```

**Advantages:**
- ✅ Simple: CPU RTL is unmodified
- ✅ Flexible: Easy to add/remove devices
- ✅ Testable: Each device can be unit tested

**Disadvantages:**
- ❌ No RTL peripherals: Can't test RTL peripheral integration
- ❌ No synthesis: Rust bus can't be synthesized

**Mechanism 2: SystemVerilog DPI (Direct Programming Interface)**

For integrating complex Rust peripherals with RTL, use Verilator's DPI support:

```systemverilog
// In top.sv or peripheral module
import "DPI-C" function int dpi_peripheral_read(input int addr);
import "DPI-C" function void dpi_peripheral_write(input int addr, input int data);

always_comb begin
    if (periph_re) begin
        periph_rdata = dpi_peripheral_read(periph_addr);
    end
end

always_ff @(posedge clk) begin
    if (periph_we) begin
        dpi_peripheral_write(periph_addr, periph_wdata);
    end
end
```

```rust
// In Rust harness
#[no_mangle]
pub extern "C" fn dpi_peripheral_read(addr: u32) -> u32 {
    // Access Rust peripheral via global or callback
    unsafe { PERIPHERAL.read_word(addr) }
}

#[no_mangle]
pub extern "C" fn dpi_peripheral_write(addr: u32, data: u32) {
    unsafe { PERIPHERAL.write_word(addr, data); }
}
```

**Advantages:**
- ✅ RTL integration: Can test RTL + Rust peripherals together
- ✅ Flexible: Rust provides complex behavior, RTL provides interface

**Disadvantages:**
- ❌ Complex: Requires careful management of global state
- ❌ Not synthesizable: DPI calls don't work in FPGA
- ❌ Verilator-specific: May not work with other simulators

**Best Practices for DPI (from research):**
1. Use DPI functions (not tasks) - Verilator has limited task support
2. Mark side-effect functions as `impure` to prevent optimization
3. Use 2-state types (bit/int/logic) - avoid 4-state (x/z)
4. Pass data through arguments only, not global variable access
5. Use input arguments only (no output/reference parameters)

**Mechanism 3: Split Bus (Recommended)**

The recommended approach for this project:

```systemverilog
module top_with_peripherals (
    input  logic        clk,
    input  logic        rst_n,
    
    // External memory (handled by Rust)
    output logic [31:0] ext_mem_addr,
    output logic [31:0] ext_mem_wdata,
    input  logic [31:0] ext_mem_rdata,
    output logic        ext_mem_we,
    output logic        ext_mem_re,
    input  logic        ext_mem_ready,
    
    // Other signals...
);
    // Internal signals
    logic [31:0] cpu_dmem_addr;
    logic [31:0] cpu_dmem_wdata;
    logic [31:0] cpu_dmem_rdata;
    logic        cpu_dmem_we;
    logic        cpu_dmem_re;
    logic        cpu_dmem_ready;
    
    // Peripheral signals
    logic [31:0] periph_rdata[0:3];
    logic        periph_ready[0:3];
    
    // CPU core
    top cpu (
        .clk(clk),
        .rst_n(rst_n),
        .dmem_addr(cpu_dmem_addr),
        .dmem_wdata(cpu_dmem_wdata),
        .dmem_rdata(cpu_dmem_rdata),
        .dmem_we(cpu_dmem_we),
        .dmem_re(cpu_dmem_re),
        .dmem_ready(cpu_dmem_ready),
        // ...
    );
    
    // RTL peripherals
    gpio_controller periph_gpio (
        .clk(clk),
        .rst_n(rst_n),
        .addr(cpu_dmem_addr),
        .wdata(cpu_dmem_wdata),
        .rdata(periph_rdata[0]),
        .we(cpu_dmem_we && (cpu_dmem_addr[31:16] == 16'h5000)),
        .re(cpu_dmem_re && (cpu_dmem_addr[31:16] == 16'h5000)),
        .ready(periph_ready[0])
    );
    
    timer_controller periph_timer (
        .clk(clk),
        .rst_n(rst_n),
        .addr(cpu_dmem_addr),
        .wdata(cpu_dmem_wdata),
        .rdata(periph_rdata[1]),
        .we(cpu_dmem_we && (cpu_dmem_addr[31:16] == 16'h6000)),
        .re(cpu_dmem_re && (cpu_dmem_addr[31:16] == 16'h6000)),
        .ready(periph_ready[1])
    );
    
    // External bus for Rust peripherals
    assign ext_mem_addr  = cpu_dmem_addr;
    assign ext_mem_wdata = cpu_dmem_wdata;
    assign ext_mem_we    = cpu_dmem_we && (cpu_dmem_addr[31:28] >= 4'h8);
    assign ext_mem_re    = cpu_dmem_re && (cpu_dmem_addr[31:28] >= 4'h8);
    
    // Multiplex responses
    always_comb begin
        if (cpu_dmem_addr[31:16] == 16'h5000) begin
            cpu_dmem_rdata = periph_rdata[0];
            cpu_dmem_ready = periph_ready[0];
        end else if (cpu_dmem_addr[31:16] == 16'h6000) begin
            cpu_dmem_rdata = periph_rdata[1];
            cpu_dmem_ready = periph_ready[1];
        end else begin
            cpu_dmem_rdata = ext_mem_rdata;
            cpu_dmem_ready = ext_mem_ready;
        end
    end
endmodule
```

**Advantages:**
- ✅ Synthesizable: RTL peripherals can go to FPGA
- ✅ Flexible: Rust handles complex peripherals
- ✅ Clean separation: RTL peripherals don't depend on Rust
- ✅ Testable: Both RTL and Rust parts can be tested independently

**Disadvantages:**
- ⚠️ More complex: Requires careful address space partitioning
- ⚠️ Duplication: Address decoding in both RTL and Rust

### 5.3 Memory Map Strategy for Hybrid Architecture

**Proposed Memory Map:**
```
0x00000000 - 0x2FFFFFFF  Reserved for future use
0x30000000 - 0x3FFFFFFF  RTL peripherals (synthesizable)
  0x30000000 - 0x300000FF    SimControl (or move to Rust)
  0x31000000 - 0x310000FF    GPIO
  0x32000000 - 0x320000FF    Timer
  0x33000000 - 0x330000FF    UART
  0x34000000 - 0x340000FF    SPI
  0x35000000 - 0x350000FF    I2C
0x40000000 - 0x7FFFFFFF  Rust peripherals (simulation-only)
  0x40000000 - 0x40000007    FIFO
  0x50000000 - 0x5000000F    DMA
  0x60000000 - 0x6000000F    Video
  0x70000000 - 0x7000000F    Audio
0x80000000 - 0xFFFFFFFF  DRAM (2 GiB)
```

---

## 6. Comparison Matrix

| Aspect | RTL (SystemVerilog) | Rust Simulation | Hybrid |
|--------|---------------------|-----------------|--------|
| **Synthesizable** | ✅ Yes | ❌ No | ⚠️ Partial (RTL parts only) |
| **Cycle-accurate** | ✅ Yes | ⚠️ Optional (via `clock_cycle`) | ⚠️ RTL parts only |
| **Development Speed** | ❌ Slow | ✅ Fast | ⚠️ Medium |
| **Simulation Speed** | ❌ Slow | ✅ Fast | ⚠️ Medium |
| **Debugging** | ❌ Complex (waveforms) | ✅ Easy (debugger, logs) | ⚠️ Mixed |
| **Complex Behavior** | ❌ Limited | ✅ Excellent | ✅ Excellent (Rust) |
| **Hardware Validation** | ✅ Excellent | ❌ None | ⚠️ RTL parts only |
| **Type Safety** | ❌ Limited | ✅ Strong | ⚠️ Rust parts only |
| **Bus Protocol Compliance** | ✅ Verifiable | ❌ N/A | ⚠️ RTL parts only |
| **Host Integration** | ❌ Difficult | ✅ Easy | ✅ Easy (Rust) |
| **Unit Testing** | ⚠️ Testbenches | ✅ Cargo test | ✅ Both |
| **FPGA Deployment** | ✅ Direct | ❌ Impossible | ⚠️ RTL parts only |
| **Iteration Time** | ❌ Long (recompile RTL) | ✅ Short (cargo build) | ⚠️ Depends |
| **Learning Curve** | ❌ Steep (HDL) | ⚠️ Medium (Rust) | ❌ Both |
| **Resource Constraints** | ✅ Enforced | ❌ Unlimited | ⚠️ Mixed |

**Legend:**
- ✅ Excellent
- ⚠️ Partial / Medium
- ❌ Poor / Not supported

---

## 7. Implementation Guidelines

### 7.1 Peripheral Classification Decision Tree

```
                    ┌─────────────────────────┐
                    │  New Peripheral Needed  │
                    └───────────┬─────────────┘
                                │
                    ┌───────────▼────────────┐
                    │ Will it be synthesized │
                    │   to FPGA/ASIC?        │
                    └───┬───────────────┬────┘
                       YES               NO
                        │                 │
            ┌───────────▼─────────────┐   │
            │ Is it cycle-accurately   │   │
            │  timing critical?        │   │
            └───┬──────────────────┬──┘   │
               YES                NO      │
                │                  │      │
        ┌───────▼────────┐  ┌──────▼─────▼────────┐
        │ Implement in   │  │ Does it need complex │
        │ SystemVerilog  │  │ behavior (networking,│
        │      RTL       │  │  file I/O, GUI)?     │
        └────────────────┘  └───┬──────────────┬───┘
                               YES             NO
                                │               │
                    ┌───────────▼─────┐  ┌──────▼──────┐
                    │ Implement in    │  │ Either RTL  │
                    │  Rust harness   │  │  or Rust    │
                    │  (simulation)   │  │ (your choice)│
                    └─────────────────┘  └─────────────┘
```

### 7.2 RTL Peripheral Guidelines

**When implementing in RTL:**

1. **Follow ready/valid handshaking:**
```systemverilog
// Always use ready signal
output logic ready;

// Single-cycle operations
assign ready = 1'b1;

// Multi-cycle operations
logic [3:0] delay_counter;
assign ready = (delay_counter == 0);
```

2. **Use proper reset:**
```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        // Reset all registers to known state
        register <= 32'h0;
    end else begin
        // Normal operation
    end
end
```

3. **Separate combinational and sequential logic:**
```systemverilog
// Combinational logic
always_comb begin
    rdata = 32'h0;
    case (addr)
        // ...
    endcase
end

// Sequential logic
always_ff @(posedge clk or negedge rst_n) begin
    // ...
end
```

4. **Lint your code:**
```bash
verilator --lint-only rtl/peripheral.sv
```

5. **Add comprehensive testbenches:**
```systemverilog
module peripheral_tb;
    // Testbench logic
endmodule
```

### 7.3 Rust Peripheral Guidelines

**When implementing in Rust:**

1. **Implement `BusDevice` trait:**
```rust
impl BusDevice for MyPeripheral {
    // Required methods
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError>;
    fn write_word(&mut self, ctx: &mut SystemContext, offset: u32, value: u32) -> Result<(), BusDeviceError>;
    fn size(&self) -> u32;
    fn name(&self) -> &str;
    
    // Optional but recommended
    fn reset(&mut self, ctx: &mut SystemContext);
    fn clock_cycle(&mut self, ctx: &mut SystemContext);
}
```

2. **Use proper error handling:**
```rust
match offset {
    0x00 => Ok(self.register),
    _ => Err(BusDeviceError::InvalidAddress { offset }),
}
```

3. **Leverage `SystemContext` for memory access:**
```rust
fn clock_cycle(&mut self, ctx: &mut SystemContext) {
    if self.dma_active {
        let data = ctx.read_word(self.src_addr);
        ctx.write_word(self.dst_addr, data);
    }
}
```

4. **Write comprehensive unit tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_register_access() {
        let mut periph = MyPeripheral::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);
        
        periph.write_word(&mut ctx, 0x00, 0x1234).unwrap();
        assert_eq!(periph.read_word(&mut ctx, 0x00).unwrap(), 0x1234);
    }
}
```

5. **Follow Rust coding standards:**
```bash
cargo fmt
cargo clippy --fix --allow-dirty
cargo clippy -- -D warnings
```

### 7.4 Integration Workflow

**Adding a new RTL peripheral:**

1. Create SystemVerilog module in `rtl/peripheral_name.sv`
2. Lint with `verilator --lint-only rtl/peripheral_name.sv`
3. Add to top-level wrapper with address decoding
4. Update memory map documentation
5. Add integration test in `testbench/tests/`
6. Run `cargo clean` to rebuild Verilator cache
7. Run `cargo test`

**Adding a new Rust peripheral:**

1. Create Rust module in `cpu-sim/src/peripheral_name.rs`
2. Implement `BusDevice` trait
3. Add to `cpu-sim/src/lib.rs`
4. Register in `SystemBus` (either internal or external)
5. Add unit tests in module
6. Add integration test if needed
7. Run `cargo test`

---

## 8. Recommendations

### 8.1 Short-Term (Current Project State)

**Keep current Rust-only peripheral approach for:**
- DRAM ✅ (already exists)
- FIFO ✅ (already exists)
- DMA ✅ (already exists)
- Video ✅ (already exists)
- Audio ✅ (already exists)
- SimControl ✅ (already exists)

**Rationale:** These work well in simulation and don't need FPGA synthesis.

### 8.2 Medium-Term (FPGA Deployment)

**Add RTL peripherals when deploying to FPGA:**
1. **GPIO controller** - Simple, common, synthesizable
2. **Timer/Counter** - Needed for OS/driver testing
3. **UART** - Essential for debug output on FPGA
4. **Interrupt controller** - If adding interrupt support

**Implementation strategy:**
1. Create `rtl_peripherals/` directory
2. Implement each as standalone SystemVerilog module
3. Create `top_with_peripherals.sv` wrapper
4. Add Verilator testbenches for each
5. Partition memory map (0x30000000-0x3FFFFFFF for RTL)

### 8.3 Long-Term (Production System)

**Hybrid architecture:**
- **RTL (0x30000000-0x3FFFFFFF):** GPIO, Timer, UART, SPI, I2C, Interrupt Controller
- **Rust (0x40000000-0x7FFFFFFF):** FIFO, DMA, Video, Audio, Network, Storage
- **DRAM (0x80000000-0xFFFFFFFF):** Shared memory

**Benefits:**
- ✅ RTL peripherals can be synthesized to FPGA
- ✅ Rust peripherals provide rich simulation environment
- ✅ Clean separation of concerns
- ✅ Best of both worlds

### 8.4 Documentation Needs

1. **Memory Map Document** - Document all peripheral addresses
2. **Peripheral Register Maps** - Document each peripheral's registers
3. **Integration Guide** - How to add new peripherals
4. **FPGA Deployment Guide** - How to synthesize RTL peripherals
5. **Driver Examples** - Bare-metal C/Rust examples for each peripheral

---

## 9. References

### 9.1 Industry Standards

- **AMBA APB Specification** - ARM's Advanced Peripheral Bus protocol
- **AMBA AXI Specification** - ARM's Advanced eXtensible Interface
- **Wishbone B4 Specification** - Open-source SoC interconnect
- **IEEE 1800-2017** - SystemVerilog Language Reference Manual
- **IEEE 1364-2005** - Verilog Hardware Description Language

### 9.2 Research Papers

- **"Virtual-Peripheral-in-the-Loop: A Hardware-in-the-Loop Strategy"** - ArXiv, 2023
- **"Accelerating RTL Simulation with Hardware-Software Co-Design"** - MICRO 2023
- **"Component-Based Hardware/Software Co-Simulation"** - Portland State University
- **"Methodology for Hardware/Software Co-verification in C/C++"** - University of South Florida

### 9.3 Tools and Frameworks

- **Verilator** - Open-source SystemVerilog simulator
  - User Guide: https://verilator.org/guide/latest/
  - DPI Documentation: https://www.cl.cam.ac.uk/~jrrk2/docs/other/dpi/
- **Marlin** - Rust framework for Verilator integration (used in this project)
- **QEMU** - Open-source machine emulator and virtualizer
- **SystemC TLM** - Transaction-Level Modeling library

### 9.4 Existing Implementations

- **Chipmunk Logic** - "Designing Memory-mapped Peripheral IPs in RTL"
- **Microchip SmartFusion** - Bus Functional Model documentation
- **OpenCores** - Open-source peripheral IP cores
- **LowRISC** - Open-source SoC platform with peripheral examples

### 9.5 Project-Specific References

**Current codebase:**
- `cpu-sim/src/bus.rs` - System bus implementation
- `cpu-sim/src/bus_device.rs` - BusDevice trait and SystemContext
- `cpu-sim/src/dram.rs` - DRAM peripheral example
- `cpu-sim/src/dma.rs` - DMA controller example
- `cpu-sim/src/video.rs` - Video controller example
- `rtl/top.sv` - CPU top module with memory interface
- `rtl/mem_interface.sv` - Memory interface module

**Documentation:**
- `AGENTS.md` - Developer guide with FSM details
- `README.md` - Project overview
- `cpu-sim/README.md` - Simulator documentation

---

## Appendix A: Example Memory Maps

### A.1 Proposed Hybrid Memory Map

```
Address Range          | Device              | Type | Description
-----------------------|---------------------|------|----------------------------
0x00000000-0x2FFFFFFF | Reserved            | -    | Reserved for future use
0x30000000-0x300000FF | SimControl*         | RTL  | Simulation control
0x31000000-0x310000FF | GPIO                | RTL  | 8-bit GPIO controller
0x32000000-0x320000FF | Timer               | RTL  | Programmable timer
0x33000000-0x330000FF | UART                | RTL  | UART controller
0x34000000-0x340000FF | SPI                 | RTL  | SPI master
0x35000000-0x350000FF | I2C                 | RTL  | I2C master
0x36000000-0x3FFFFFFF | Reserved (RTL)      | RTL  | Reserved for future RTL
0x40000000-0x40000007 | FIFO                | Rust | Host communication FIFO
0x50000000-0x50000013 | DMA                 | Rust | DMA controller
0x60000000-0x6000000F | Video               | Rust | Video frame buffer
0x70000000-0x7000000F | Audio               | Rust | Audio buffer
0x71000000-0x7FFFFFFF | Reserved (Rust)     | Rust | Reserved for future Rust
0x80000000-0xFFFFFFFF | DRAM                | Both | 2 GiB system memory

* SimControl could remain in Rust for simulation-only use
```

### A.2 Current Simulation Memory Map

```
Address Range          | Device              | Size
-----------------------|---------------------|--------
0x30000000-0x30000003 | SimControl          | 4 bytes
0x40000000-0x40000007 | FIFO                | 8 bytes
0x80000000-0xFFFFFFFF | DRAM                | 2 GiB
```

---

## Appendix B: Code Templates

### B.1 Minimal RTL Peripheral Template

```systemverilog
module minimal_peripheral (
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus interface
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic        re,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready
);
    // Registers
    logic [31:0] control_reg;
    logic [31:0] status_reg;
    
    // Register writes
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            control_reg <= 32'h0;
        end else if (we && req && ready) begin
            case (addr[3:0])
                4'h0: control_reg <= wdata;
            endcase
        end
    end
    
    // Register reads
    always_comb begin
        rdata = 32'h0;
        if (re && req) begin
            case (addr[3:0])
                4'h0: rdata = control_reg;
                4'h4: rdata = status_reg;
            endcase
        end
    end
    
    // Ready (single-cycle for simple peripherals)
    assign ready = 1'b1;
    
    // Peripheral logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            status_reg <= 32'h0;
        end else begin
            // Update status based on control
        end
    end
endmodule
```

### B.2 Minimal Rust Peripheral Template

```rust
use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

pub struct MinimalPeripheral {
    control: u32,
    status: u32,
}

impl MinimalPeripheral {
    pub fn new() -> Self {
        MinimalPeripheral {
            control: 0,
            status: 0,
        }
    }
}

impl Default for MinimalPeripheral {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for MinimalPeripheral {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.control),
            0x04 => Ok(self.status),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }
    
    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                self.control = value;
                Ok(())
            }
            0x04 => Err(BusDeviceError::WriteToReadOnly { offset }),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }
    
    fn size(&self) -> u32 {
        8  // 2 registers × 4 bytes
    }
    
    fn name(&self) -> &str {
        "MinimalPeripheral"
    }
    
    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.control = 0;
        self.status = 0;
    }
    
    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        // Update state each clock cycle
        if self.control & 0x1 != 0 {
            self.status += 1;
        }
    }
}
```

---

## Conclusion

This research document provides a comprehensive analysis of bus device and memory-mapped peripheral implementation approaches for the RISC-V CPU project. The key takeaways are:

1. **Current architecture is excellent** for simulation-only peripherals
2. **Hybrid approach is recommended** for future FPGA deployment
3. **RTL peripherals** should be used for simple, synthesizable devices
4. **Rust peripherals** should be used for complex, simulation-only devices
5. **Clean memory map partitioning** enables both approaches to coexist

The project is well-positioned to support both RTL and Rust peripherals with minimal architectural changes. The existing `BusDevice` trait and `SystemBus` infrastructure provide a solid foundation for expansion.

**Next Steps:**
1. Document current memory map
2. Plan FPGA peripheral requirements
3. Implement first RTL peripheral (e.g., GPIO)
4. Create integration guide for future developers

---

**Document Version:** 1.0  
**Last Updated:** January 27, 2026  
**Maintainer:** GitHub Copilot Hardware-Software Integration Architect

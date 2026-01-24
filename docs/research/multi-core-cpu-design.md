# Multi-Core RISC-V CPU Design for cpu-sim

**Research Document**  
**Context:** Extending the single-core RV32IMACF CPU to a multi-hart/multi-core architecture  
**Date:** 2026-01-24

## Executive Summary

This document presents research findings and concrete approaches for implementing multi-hart/multi-core RISC-V CPU support into the `cpu-sim` crate. The current architecture features a single-core, multi-cycle non-pipelined RV32IMACF CPU with 118 instructions. This research explores three viable paths to extend the design to support 2-4 cores while maintaining the existing verification framework and simulation performance.

**Key Recommendations:**
1. **Start with dual-core SMP** (Symmetric Multiprocessing) using shared memory
2. **Implement basic CLINT** for inter-processor interrupts (IPIs)
3. **Leverage existing atomic instruction support** (A extension already implemented)
4. **Use Verilator multi-threading** for parallel core simulation
5. **Extend SystemBus architecture** to support multi-core memory arbitration

## Context: Current cpu-sim Architecture

### Current Single-Core Design

The existing system consists of:

**RTL Layer (SystemVerilog):**
- **Top Module (`top.sv`)**: Multi-cycle FSM with 12 states (IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, HALT, ATOMIC_RMW)
- **Instruction Set**: RV32IMACF (118 instructions: Base + M/A/C/F extensions + Zicsr)
- **Memory Interface**: Separate instruction and data ports with ready/valid handshaking for variable-latency memory
- **Debug Infrastructure**: FIFO-based packet protocol at 0x40000000

**Rust Simulation Layer:**
- **Verilator Integration**: Marlin-based RTL simulation with automatic compilation/caching
- **SystemBus Architecture**: Memory-mapped device routing with pluggable `BusDevice` trait
  - DRAM: 0x80000000 - 0xFFFFFFFF
  - FIFO: 0x40000000
  - SimControl: 0x30000000
  - Audio: 0x50000000
  - Video: 0x60000000
- **Memory Model**: Sparse HashMap-based byte-addressable memory with configurable latency
- **Verification**: 264 comprehensive tests across ALU, registers, decompression, and CPU execution

**Architecture Strengths for Multi-Core Extension:**
- ✅ Atomic instructions (A extension) already implemented for synchronization
- ✅ Clean memory interface with ready/valid handshaking
- ✅ Modular SystemBus with device registration pattern
- ✅ CSR infrastructure for per-hart state management
- ✅ Variable-latency memory support for contention modeling

**Architecture Challenges:**
- ❌ Single global PC and register file
- ❌ No inter-processor interrupt mechanism
- ❌ No memory arbitration for concurrent access
- ❌ Single-threaded Verilator simulation model

---

## Research Findings: RISC-V Multi-Hart Specifications

### 1. Hart Identification and Isolation

**RISC-V Privileged Architecture** defines the concept of "hart" (hardware thread):
- Each hart has a unique **`mhartid`** CSR (read-only, machine mode)
- Harts operate independently with separate:
  - Register files (x0-x31 for integer, f0-f31 for floating-point)
  - CSR state (status, interrupt enable, exception handlers, counters)
  - PC (program counter)
  - FSM state (if multi-cycle)

**Implication for cpu-sim:**
- Need to instantiate multiple `top.sv` modules (one per hart)
- Each hart gets unique `mhartid` parameter
- Shared memory bus requires arbitration logic

**Reference:** RISC-V Privileged Architecture Specification, Volume II, Chapters 1-3

### 2. Memory Ordering Model (RVWMO)

**RISC-V Weak Memory Ordering (RVWMO):**
- Less strict than sequential consistency
- Allows instruction/memory reordering for performance
- Synchronization via:
  - **FENCE** instruction (explicit ordering barrier)
  - **Atomic operations** (LR/SC, AMO with acquire/release semantics)
  
**Memory Ordering Guarantees:**
- Operations on same address appear in program order
- Atomic instructions enforce ordering based on `aq` (acquire) and `rl` (release) bits
- Multi-core correctness requires proper FENCE/atomic usage in software

**Implication for cpu-sim:**
- Current A extension implementation provides atomicity primitives
- RTL must ensure atomic operations are truly atomic across harts
- Verification tests must validate memory ordering semantics

**Reference:** RVWMO specification in unprivileged ISA manual

### 3. Cache Coherence Protocols

**Common Protocols for RISC-V Multi-Core:**
- **MSI (Modified, Shared, Invalid):** Simplest 3-state protocol
  - Modified: Exclusively owned, dirty
  - Shared: Read-only, may be in other caches
  - Invalid: Not present or stale
  
- **MESI (Modified, Exclusive, Shared, Invalid):** 4-state protocol
  - Adds "Exclusive" state for clean exclusive ownership
  - Reduces bus traffic for read-modify-write sequences
  
- **Directory-Based:** For scalable many-core (8+ cores)
  - Maintains directory of which caches hold each line
  - Used in OpenPiton, DECADES SoC (60+ RISC-V cores)

**For Small Multi-Core (2-4 harts):**
- **Snoop-based MSI/MESI** is sufficient
- All caches monitor (snoop) bus transactions
- Invalidate/update on write conflicts

**Implication for cpu-sim:**
- **No cache in current design** - simplifies initial multi-core implementation!
- Future cache addition would require coherence protocol
- Current shared memory model acts as "always coherent" (but slow)

**References:**
- MIT 6.823 Cache Coherence Lectures
- "Cache Coherent Framework for RISC-V Many-core Systems" (CARRV 2023)
- "Culsans: Efficient Snoop-based Coherency for CVA6" (arXiv 2407.19895)

### 4. Inter-Processor Interrupts (IPIs)

**CLINT (Core Local Interruptor):**
- Memory-mapped registers per hart:
  - **MSIP (Machine Software Interrupt Pending):** IPI trigger
  - **MTIMECMP:** Timer compare for local timer interrupts
  - **MTIME:** Global time counter
  
**IPI Mechanism:**
```
Hart 0 writes to Hart 1's MSIP register → Hart 1 receives machine software interrupt
```

**Memory Map Example (2-hart):**
```
0x02000000: Hart 0 MSIP
0x02000004: Hart 1 MSIP
0x02004000: Hart 0 MTIMECMP
0x02004008: Hart 1 MTIMECMP
0x0200BFF8: MTIME (shared)
```

**PLIC (Platform-Level Interrupt Controller):**
- Routes **external interrupts** (from devices) to harts
- Each hart has target context with priority/enable
- Not used for IPIs (CLINT handles core-to-core signaling)

**Implication for cpu-sim:**
- Implement minimal CLINT as a `BusDevice` at 0x02000000
- Each hart polls its MSIP register for software interrupts
- Extend CSR file to support `mip` (interrupt pending) and `mie` (interrupt enable)

**References:**
- RISC-V Privileged Architecture: CLINT/PLIC sections
- "RISC-V Advanced Core Local Interruptor (ACLINT) Specification"
- SiFive CLINT documentation

### 5. Synchronization Primitives

**Already Implemented (A Extension):**
- **LR/SC (Load-Reserved/Store-Conditional):**
  - LR.W: Load word and set reservation
  - SC.W: Store conditionally if reservation valid
  - Current RTL has `reservation_valid` and `reservation_addr` logic
  
- **AMO (Atomic Memory Operations):**
  - AMOSWAP, AMOADD, AMOAND, AMOOR, AMOXOR
  - AMOMIN, AMOMAX (signed), AMOMINU, AMOMAXU (unsigned)
  - Current RTL has `S_ATOMIC_RMW` state for atomic operations

**Multi-Core Considerations:**
- **Reservation must be per-hart** (not global)
- **SC must fail if another hart writes to reserved address**
- **AMO operations must be indivisible** across all harts

**Implication for cpu-sim:**
- Add hart ID to reservation tracking: `reservation_valid[hart_id]`, `reservation_addr[hart_id]`
- Implement reservation invalidation logic when any hart writes to reserved address
- SystemBus must serialize atomic operations (only one AMO active at a time)

**Reference:** RISC-V A Extension Specification (Unprivileged ISA Manual, Chapter 8)

---

## Implementation Approaches

### Approach 1: Dual-Core SMP with Shared Memory (Recommended)

#### Overview

This approach creates a symmetric dual-core system where both harts share the same memory space and can execute identical code. This is the simplest path to multi-core support and leverages the existing architecture maximally.

#### Architecture Diagram (Text-Based)

```
┌─────────────────────────────────────────────────────────────┐
│                      Rust Simulator                         │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  SystemBus                          │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │   │
│  │  │   DRAM   │  │   FIFO   │  │  CLINT   │ ...      │   │
│  │  └──────────┘  └──────────┘  └──────────┘          │   │
│  └───────────┬─────────────────────────┬───────────────┘   │
│              │                         │                    │
│      ┌───────▼─────────┐       ┌───────▼─────────┐        │
│      │  MemArbiter     │       │  MemArbiter     │        │
│      │  (Hart 0)       │       │  (Hart 1)       │        │
│      └───────┬─────────┘       └───────┬─────────┘        │
│              │                         │                    │
├──────────────┼─────────────────────────┼────────────────────┤
│ Verilator    │                         │                    │
│  ┌───────────▼─────────┐       ┌───────▼─────────┐        │
│  │   Top (Hart 0)      │       │   Top (Hart 1)  │        │
│  │   mhartid = 0       │       │   mhartid = 1   │        │
│  │  ┌───┬───┬───┬───┐ │       │  ┌───┬───┬───┐  │        │
│  │  │ALU│REG│CSR│..│  │       │  │ALU│REG│CSR│  │        │
│  │  └───┴───┴───┴───┘ │       │  └───┴───┴───┘  │        │
│  └─────────────────────┘       └─────────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

#### RTL Changes

**1. Parameterize `top.sv` for Hart ID:**

```systemverilog
module top #(
    parameter HART_ID = 0  // Unique hart identifier
) (
    input  logic        clk,
    input  logic        rst_n,
    // ... existing ports ...
);
    
    // Hart ID CSR (read-only)
    assign csr_rdata = (csr_addr == 12'hF14) ? HART_ID : /* other CSRs */;
    
    // Per-hart reservation tracking (moved to memory controller)
    // Remove reservation_valid/reservation_addr from top.sv
```

**2. Create Memory Arbiter Module:**

```systemverilog
module mem_arbiter #(
    parameter NUM_HARTS = 2
) (
    input  logic        clk,
    input  logic        rst_n,
    
    // Hart interfaces (per-hart arrays)
    input  logic [NUM_HARTS-1:0][31:0] hart_imem_addr,
    output logic [NUM_HARTS-1:0][31:0] hart_imem_data,
    input  logic [NUM_HARTS-1:0]       hart_imem_req,
    output logic [NUM_HARTS-1:0]       hart_imem_ready,
    
    input  logic [NUM_HARTS-1:0][31:0] hart_dmem_addr,
    input  logic [NUM_HARTS-1:0][31:0] hart_dmem_wdata,
    output logic [NUM_HARTS-1:0][31:0] hart_dmem_rdata,
    input  logic [NUM_HARTS-1:0]       hart_dmem_we,
    input  logic [NUM_HARTS-1:0]       hart_dmem_re,
    input  logic [NUM_HARTS-1:0][1:0]  hart_dmem_size,
    input  logic [NUM_HARTS-1:0]       hart_dmem_req,
    output logic [NUM_HARTS-1:0]       hart_dmem_ready,
    
    // Shared memory interface (to SystemBus)
    output logic [31:0] mem_addr,
    output logic [31:0] mem_wdata,
    input  logic [31:0] mem_rdata,
    output logic        mem_we,
    output logic        mem_re,
    output logic [1:0]  mem_size,
    output logic        mem_req,
    input  logic        mem_ready
);
    // Round-robin arbitration logic
    // Priority: atomic operations > data access > instruction fetch
    // Tracks reservation_valid/reservation_addr per hart
endmodule
```

**3. Top-Level Multi-Core Wrapper:**

```systemverilog
module multi_core_top #(
    parameter NUM_HARTS = 2
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Shared memory interface
    output logic [31:0] mem_addr,
    output logic [31:0] mem_wdata,
    input  logic [31:0] mem_rdata,
    output logic        mem_we,
    output logic        mem_re,
    output logic [1:0]  mem_size,
    output logic        mem_req,
    input  logic        mem_ready,
    
    // Debug outputs per hart
    output logic [NUM_HARTS-1:0]       halted,
    output logic [NUM_HARTS-1:0][31:0] debug_pc
);
    
    // Hart-specific signals
    logic [NUM_HARTS-1:0][31:0] hart_imem_addr;
    logic [NUM_HARTS-1:0][31:0] hart_imem_data;
    logic [NUM_HARTS-1:0]       hart_imem_req;
    logic [NUM_HARTS-1:0]       hart_imem_ready;
    
    logic [NUM_HARTS-1:0][31:0] hart_dmem_addr;
    logic [NUM_HARTS-1:0][31:0] hart_dmem_wdata;
    logic [NUM_HARTS-1:0][31:0] hart_dmem_rdata;
    logic [NUM_HARTS-1:0]       hart_dmem_we;
    logic [NUM_HARTS-1:0]       hart_dmem_re;
    logic [NUM_HARTS-1:0][1:0]  hart_dmem_size;
    logic [NUM_HARTS-1:0]       hart_dmem_req;
    logic [NUM_HARTS-1:0]       hart_dmem_ready;
    
    // Instantiate harts
    generate
        for (genvar i = 0; i < NUM_HARTS; i++) begin : hart_gen
            top #(.HART_ID(i)) hart_inst (
                .clk(clk),
                .rst_n(rst_n),
                .boot_addr(boot_addr),
                .imem_addr(hart_imem_addr[i]),
                .imem_data(hart_imem_data[i]),
                .imem_req(hart_imem_req[i]),
                .imem_ready(hart_imem_ready[i]),
                .dmem_addr(hart_dmem_addr[i]),
                .dmem_wdata(hart_dmem_wdata[i]),
                .dmem_rdata(hart_dmem_rdata[i]),
                .dmem_we(hart_dmem_we[i]),
                .dmem_re(hart_dmem_re[i]),
                .dmem_size(hart_dmem_size[i]),
                .dmem_req(hart_dmem_req[i]),
                .dmem_ready(hart_dmem_ready[i]),
                .halted(halted[i]),
                .debug_pc(debug_pc[i])
            );
        end
    endgenerate
    
    // Memory arbiter
    mem_arbiter #(.NUM_HARTS(NUM_HARTS)) arbiter_inst (
        .clk(clk),
        .rst_n(rst_n),
        .hart_imem_addr(hart_imem_addr),
        .hart_imem_data(hart_imem_data),
        .hart_imem_req(hart_imem_req),
        .hart_imem_ready(hart_imem_ready),
        .hart_dmem_addr(hart_dmem_addr),
        .hart_dmem_wdata(hart_dmem_wdata),
        .hart_dmem_rdata(hart_dmem_rdata),
        .hart_dmem_we(hart_dmem_we),
        .hart_dmem_re(hart_dmem_re),
        .hart_dmem_size(hart_dmem_size),
        .hart_dmem_req(hart_dmem_req),
        .hart_dmem_ready(hart_dmem_ready),
        .mem_addr(mem_addr),
        .mem_wdata(mem_wdata),
        .mem_rdata(mem_rdata),
        .mem_we(mem_we),
        .mem_re(mem_re),
        .mem_size(mem_size),
        .mem_req(mem_req),
        .mem_ready(mem_ready)
    );
    
endmodule
```

#### Rust Changes

**1. Create CLINT Device:**

```rust
// cpu-sim/src/clint.rs
use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

pub struct Clint {
    num_harts: usize,
    msip: Vec<bool>,        // Machine software interrupt pending (per hart)
    mtimecmp: Vec<u64>,     // Timer compare (per hart)
    mtime: u64,             // Global time counter
}

impl Clint {
    pub fn new(num_harts: usize) -> Self {
        Self {
            num_harts,
            msip: vec![false; num_harts],
            mtimecmp: vec![0; num_harts],
            mtime: 0,
        }
    }
    
    pub fn tick(&mut self) {
        self.mtime += 1;
    }
    
    pub fn get_msip(&self, hart_id: usize) -> bool {
        self.msip.get(hart_id).copied().unwrap_or(false)
    }
}

impl BusDevice for Clint {
    fn name(&self) -> &str { "CLINT" }
    fn size(&self) -> u32 { 0x10000 } // 64KB
    
    fn read(&self, offset: u32, _ctx: &SystemContext) -> Result<u32, BusDeviceError> {
        match offset {
            // MSIP registers (0x0000 - 0x0003 for hart 0, etc.)
            0x0000..=0x0FFF if offset < (self.num_harts as u32 * 4) => {
                let hart_id = (offset / 4) as usize;
                Ok(if self.msip[hart_id] { 1 } else { 0 })
            }
            // MTIMECMP registers (0x4000+)
            0x4000..=0x7FFF => {
                let hart_id = ((offset - 0x4000) / 8) as usize;
                if hart_id < self.num_harts {
                    let low = (self.mtimecmp[hart_id] & 0xFFFFFFFF) as u32;
                    let high = (self.mtimecmp[hart_id] >> 32) as u32;
                    Ok(if offset % 8 == 0 { low } else { high })
                } else {
                    Err(BusDeviceError::InvalidOffset)
                }
            }
            // MTIME register (0xBFF8)
            0xBFF8 => Ok((self.mtime & 0xFFFFFFFF) as u32),
            0xBFFC => Ok((self.mtime >> 32) as u32),
            _ => Err(BusDeviceError::InvalidOffset),
        }
    }
    
    fn write(&mut self, offset: u32, value: u32, _ctx: &SystemContext) -> Result<(), BusDeviceError> {
        match offset {
            // MSIP registers
            0x0000..=0x0FFF if offset < (self.num_harts as u32 * 4) => {
                let hart_id = (offset / 4) as usize;
                self.msip[hart_id] = value & 1 != 0;
                Ok(())
            }
            // MTIMECMP registers
            0x4000..=0x7FFF => {
                let hart_id = ((offset - 0x4000) / 8) as usize;
                if hart_id < self.num_harts {
                    if offset % 8 == 0 {
                        self.mtimecmp[hart_id] = (self.mtimecmp[hart_id] & 0xFFFFFFFF00000000) | (value as u64);
                    } else {
                        self.mtimecmp[hart_id] = (self.mtimecmp[hart_id] & 0xFFFFFFFF) | ((value as u64) << 32);
                    }
                    Ok(())
                } else {
                    Err(BusDeviceError::InvalidOffset)
                }
            }
            _ => Err(BusDeviceError::InvalidOffset),
        }
    }
}
```

**2. Extend SystemBus for Multi-Core:**

```rust
// cpu-sim/src/bus.rs modifications
use crate::clint::Clint;

pub const CLINT_BASE: u32 = 0x0200_0000;

pub struct SystemBus {
    // ... existing fields ...
    pub clint: Clint,  // New field
    
    // Memory contention tracking
    active_atomic_hart: Option<usize>,  // Which hart is doing atomic op
}

impl SystemBus {
    pub fn new_multicore(num_harts: usize) -> Self {
        let mut bus = Self::new();
        bus.clint = Clint::new(num_harts);
        
        // Register CLINT in memory map
        let memory_map_entry = MemoryMapEntry {
            base: CLINT_BASE,
            end: CLINT_BASE + bus.clint.size(),
            id: DeviceId::Clint,
        };
        bus.memory_map.push(memory_map_entry);
        
        bus
    }
    
    // Check if atomic operation can proceed (no other hart has active atomic)
    pub fn try_acquire_atomic(&mut self, hart_id: usize) -> bool {
        match self.active_atomic_hart {
            None => {
                self.active_atomic_hart = Some(hart_id);
                true
            }
            Some(id) if id == hart_id => true,
            Some(_) => false,  // Another hart has atomic lock
        }
    }
    
    pub fn release_atomic(&mut self, hart_id: usize) {
        if self.active_atomic_hart == Some(hart_id) {
            self.active_atomic_hart = None;
        }
    }
}
```

**3. Multi-Core Simulator:**

```rust
// cpu-sim/src/multi_core_sim.rs
use riscv_core::{MultiCoreTop, VerilatorRuntime};
use crate::bus::SystemBus;

pub struct MultiCoreSimulator {
    runtime: VerilatorRuntime,
    core: MultiCoreTop,
    bus: SystemBus,
    num_harts: usize,
}

impl MultiCoreSimulator {
    pub fn new(num_harts: usize) -> Result<Self, String> {
        let runtime = create_multicore_runtime(num_harts)?;
        let core = MultiCoreTop::new(&runtime);
        let bus = SystemBus::new_multicore(num_harts);
        
        Ok(Self {
            runtime,
            core,
            bus,
            num_harts,
        })
    }
    
    pub fn step(&mut self) -> Result<(), String> {
        // Clock all harts simultaneously
        self.core.clk = true;
        self.core.eval();
        
        // Handle memory requests from all harts
        for hart_id in 0..self.num_harts {
            if self.core.hart_imem_req[hart_id] != 0 {
                let addr = self.core.hart_imem_addr[hart_id];
                let data = self.bus.read_word(addr)?;
                self.core.hart_imem_data[hart_id] = data;
                self.core.hart_imem_ready[hart_id] = 1;
            }
            
            if self.core.hart_dmem_req[hart_id] != 0 {
                let addr = self.core.hart_dmem_addr[hart_id];
                if self.core.hart_dmem_we[hart_id] != 0 {
                    let data = self.core.hart_dmem_wdata[hart_id];
                    self.bus.write_word(addr, data)?;
                } else {
                    let data = self.bus.read_word(addr)?;
                    self.core.hart_dmem_rdata[hart_id] = data;
                }
                self.core.hart_dmem_ready[hart_id] = 1;
            }
        }
        
        self.core.clk = false;
        self.core.eval();
        
        // Tick CLINT
        self.bus.clint.tick();
        
        Ok(())
    }
}
```

#### Testing Strategy

**Phase 1: Single-Hart Baseline**
- Verify existing tests pass with parameterized `top.sv` (HART_ID=0)
- Ensure no regressions in single-core functionality

**Phase 2: Dual-Core Basic Tests**
- Boot both harts to same entry point
- Each hart increments separate memory location
- Verify both locations updated correctly

**Phase 3: Inter-Hart Communication**
- Hart 0 writes data to shared memory
- Hart 0 sends IPI to Hart 1 via CLINT
- Hart 1 reads IPI, reads shared data
- Validate data consistency

**Phase 4: Synchronization Primitives**
- Test LR/SC across harts (spinlock implementation)
- Test AMO operations for atomic counters
- Verify reservation invalidation when other hart writes

**Phase 5: Memory Contention**
- Both harts access same memory regions concurrently
- Verify arbiter correctly serializes requests
- Measure performance degradation under contention

#### Performance Considerations

**Verilator Simulation Performance:**
- Single-threaded Verilator: Both harts simulated sequentially (2x slower)
- Multi-threaded Verilator (`--threads N`): Parallel hart simulation (potential speedup)
  - Requires careful handling of shared memory state
  - May need separate Verilator contexts

**Memory Bottleneck:**
- Shared memory arbiter serializes all accesses
- Expected performance: ~50% per-hart throughput (vs. single-core)
- Mitigation: Implement instruction cache (future work)

**Scalability:**
- 2 harts: Good (research baseline)
- 4 harts: Acceptable (memory contention becomes significant)
- 8+ harts: Poor without cache hierarchy

#### Migration Path from Single to Multi-Core

**Step 1: Parameterize Existing Top Module**
- Add `HART_ID` parameter
- Implement `mhartid` CSR
- Test with single hart (HART_ID=0)

**Step 2: Create Memory Arbiter**
- Implement round-robin arbitration
- Add reservation tracking per hart
- Test with dual instantiation (offline, no Verilator yet)

**Step 3: Implement CLINT**
- Add CLINT device to SystemBus
- Implement MSIP/MTIME registers
- Test IPI mechanism in isolation

**Step 4: Create Multi-Core Wrapper**
- Instantiate 2 copies of `top.sv`
- Connect to memory arbiter
- Integrate with Verilator

**Step 5: Rust Integration**
- Update `riscv_core` to support multi-core top
- Extend SystemBus for multi-hart
- Create multi-core simulator API

**Step 6: Verification**
- Port existing tests to multi-core (single-hart execution)
- Add multi-hart specific tests
- Performance benchmarking

---

### Approach 2: Asymmetric Multi-Processing (AMP)

#### Overview

AMP assigns different tasks to different harts with minimal synchronization. Hart 0 runs main application, Hart 1 handles background tasks (e.g., audio processing, DMA transfers). This approach simplifies synchronization but limits parallelism.

#### Architecture

```
Hart 0 (Main)        Hart 1 (Background)
     │                      │
     ├─ Application         ├─ Audio DMA
     ├─ Video Rendering     ├─ Packet Processing
     └─ User Input          └─ Timers
```

**Memory Partitioning:**
- DRAM 0x80000000-0x8FFFFFFF: Hart 0 (256 MB)
- DRAM 0x90000000-0x9FFFFFFF: Hart 1 (256 MB)
- Shared 0xA0000000-0xAFFFFFFF: Communication (256 MB)

**Communication:**
- Lock-free ring buffers in shared region
- CLINT IPIs for task signaling
- Minimal cache coherence requirements

#### Advantages
- ✅ Simpler than SMP (less contention)
- ✅ Deterministic performance per hart
- ✅ Easier to debug (isolated execution)

#### Disadvantages
- ❌ Less flexible than SMP
- ❌ Manual task partitioning required
- ❌ Underutilization if workload imbalanced

#### Use Cases
- Real-time audio/video processing on dedicated hart
- Background monitoring while main code runs
- Offloading I/O to secondary hart

---

### Approach 3: Message-Passing Multi-Core

#### Overview

Harts communicate via explicit message queues (FIFOs) rather than shared memory. Each hart has private memory, with messages sent through hardware FIFOs. Inspired by embedded multi-core systems (e.g., ARM Cortex-R dual-core).

#### Architecture

```
┌────────────┐         ┌────────────┐
│  Hart 0    │         │  Hart 1    │
│  ┌──────┐  │         │  ┌──────┐  │
│  │ DRAM │  │         │  │ DRAM │  │
│  └──────┘  │         │  └──────┘  │
│  ┌──────┐  │         │  ┌──────┐  │
│  │ Code │  │         │  │ Code │  │
│  └──────┘  │         │  └──────┘  │
└──────┬─────┘         └──────┬─────┘
       │                      │
       └──────┬──────┬────────┘
              │      │
       ┌──────▼──┐ ┌─▼────────┐
       │ FIFO    │ │  FIFO    │
       │ 0 → 1   │ │  1 → 0   │
       └─────────┘ └──────────┘
```

**Message Protocol:**
```rust
struct Message {
    msg_type: u32,    // Command ID
    payload: [u32; 7], // Up to 7 words of data
}
```

**Memory Map:**
- Hart 0 DRAM: 0x80000000-0x8FFFFFFF
- Hart 1 DRAM: 0x90000000-0x9FFFFFFF
- FIFO 0→1: 0xA0000000 (write by 0, read by 1)
- FIFO 1→0: 0xA0001000 (write by 1, read by 0)

#### Advantages
- ✅ No cache coherence required
- ✅ Predictable latency (FIFO depth limits)
- ✅ Natural fit for pipelined processing

#### Disadvantages
- ❌ More complex software (explicit messaging)
- ❌ FIFO overflow handling required
- ❌ Not RISC-V standard (custom hardware)

#### Use Cases
- Video encoding pipeline (fetch → encode → compress)
- Network packet processing (receive → parse → transmit)
- Producer-consumer patterns

---

## Comparative Analysis

| Aspect                   | Approach 1: SMP       | Approach 2: AMP        | Approach 3: Message-Passing |
|--------------------------|-----------------------|------------------------|-----------------------------|
| **Complexity (RTL)**     | High                  | Medium                 | Medium                      |
| **Complexity (Software)**| Low (POSIX-like)      | Medium (manual split)  | High (messaging protocol)   |
| **Synchronization**      | FENCE/Atomic          | Minimal (partitioned)  | None (message queues)       |
| **Memory Sharing**       | Full (coherent)       | Partial (shared region)| None (private per hart)     |
| **Scalability**          | Poor (>4 harts)       | Good (independent)     | Excellent (FIFO scales)     |
| **RISC-V Compliance**    | Full                  | Full                   | Partial (custom FIFOs)      |
| **Debugging**            | Hard (race conditions)| Medium                 | Medium (message traces)     |
| **Best For**             | General parallelism   | Real-time workloads    | Pipeline processing         |

**Recommendation:** Start with **Approach 1 (SMP)** for maximum RISC-V compliance and software compatibility. Transition to AMP or message-passing if performance profiling reveals bottlenecks.

---

## Implementation Roadmap

### Phase 1: Foundation (2-3 weeks)
- [ ] Parameterize `top.sv` with `HART_ID`
- [ ] Implement `mhartid` CSR
- [ ] Create memory arbiter module
- [ ] Verify single-hart functionality with parameterized design

### Phase 2: Dual-Core RTL (3-4 weeks)
- [ ] Create `multi_core_top.sv` wrapper
- [ ] Implement round-robin memory arbiter
- [ ] Add per-hart reservation tracking
- [ ] Verilator integration with dual instantiation
- [ ] RTL testbench for arbiter logic

### Phase 3: CLINT Implementation (1-2 weeks)
- [ ] Create `clint.sv` module or Rust `BusDevice`
- [ ] Implement MSIP, MTIMECMP, MTIME registers
- [ ] Integrate with SystemBus
- [ ] Test IPI mechanism

### Phase 4: Rust Simulator Integration (2-3 weeks)
- [ ] Extend `riscv_core` for multi-core top
- [ ] Create `MultiCoreSimulator` API
- [ ] Update SystemBus for multi-hart support
- [ ] Implement atomic operation serialization

### Phase 5: Verification (3-4 weeks)
- [ ] Port existing single-core tests
- [ ] Create multi-hart synchronization tests
- [ ] LR/SC correctness across harts
- [ ] AMO atomicity validation
- [ ] Memory contention stress tests
- [ ] IPI communication tests

### Phase 6: Optimization & Documentation (2 weeks)
- [ ] Performance profiling
- [ ] Verilator multi-threading exploration
- [ ] User guide for multi-core programming
- [ ] Example multi-hart programs

**Total Estimated Time:** 13-18 weeks for full dual-core SMP implementation

---

## Open Questions & Future Research

### 1. Verilator Multi-Threading
- Can we use Verilator's `--threads` to parallelize hart simulation?
- What is the overhead of inter-thread synchronization for shared memory?
- Benchmark: Single-threaded vs multi-threaded Verilator performance

### 2. Cache Addition
- When to add L1 caches (per-hart)?
- MSI vs MESI protocol selection
- Cache coherence verification strategy

### 3. Memory Bandwidth
- Shared memory becomes bottleneck at scale
- Should we implement:
  - Separate instruction/data buses?
  - Bank-interleaved memory?
  - Non-blocking memory controller?

### 4. Debugging Infrastructure
- Extend FIFO protocol for per-hart debug packets?
- Trace hart ID with each instruction?
- Multi-core VCD waveform analysis tools

### 5. Software Ecosystem
- Bare-metal multi-core SDK?
- Spinlock/mutex library in Rust?
- Port FreeRTOS or Zephyr for multi-core validation?

---

## Recommended Reading & References

### RISC-V Specifications
1. **RISC-V Privileged Architecture** - Volume II (Interrupts, CSRs, hart management)
2. **RISC-V Unprivileged ISA** - Volume I (A Extension, RVWMO memory model)
3. **RISC-V CLINT Specification** - Core-local interrupts and timers
4. **RISC-V PLIC Specification** - Platform-level external interrupt controller
5. **RISC-V ACLINT Specification** - Advanced core-local interrupts (modern CLINT)

### Academic Papers
1. **"OpenPiton+Ariane: First Open-Source SMP Linux-Booting RISC-V System"** (CARRV 2019)
   - Practical dual/quad-core RISC-V SMP implementation
   - Cache coherence with P-Mesh NoC
   
2. **"Cache Coherent Framework for RISC-V Many-core Systems"** (CARRV 2023)
   - MESI/MSI protocol implementations for RISC-V
   - Scalability analysis for 60+ cores
   
3. **"Culsans: Efficient Snoop-based Coherency for CVA6"** (arXiv 2407.19895)
   - Modern snoop protocol for small multi-core (2-4 cores)
   - Performance comparison with OpenPiton
   
4. **"DECADES SoC: 67mm², 1.46 TOPS, 55 Giga Cache-Coherent 64-bit RISC-V System"** (CICC 2023)
   - Commercial-scale many-core RISC-V with directory-based coherence

### Implementation Guides
1. **MIT 6.823: Cache Coherence Lectures** - Foundational coherence protocol theory
2. **Vayavya Labs: Cache Coherence in RISC-V** - Practical implementation guide
3. **SiFive Documentation** - Real-world multi-core RISC-V examples (U74, U84)

### Open-Source Projects
1. **OpenPiton** - Many-core RISC-V research platform (GitHub: PrincetonUniversity/openpiton)
2. **SiYuan** - Dual/quad-core RISC-V SMP with TileLink (GitHub: xjtuiair-cag/SiYuan)
3. **CVA6 (Ariane)** - Application-class RISC-V core used in multi-core systems

---

## Conclusion

Multi-core RISC-V CPU design is achievable for the `cpu-sim` project with careful architectural planning. The **recommended approach** is:

1. **Start with dual-core SMP** (Approach 1) for RISC-V compliance
2. **Implement minimal CLINT** for IPI support
3. **Leverage existing A extension** for synchronization
4. **Use Verilator multi-threading** for simulation performance
5. **Iterate incrementally** with comprehensive testing at each phase

The current architecture's strengths—modular SystemBus, variable-latency memory, atomic instruction support—provide a solid foundation for multi-core extension. The primary challenges are memory arbitration, reservation tracking, and verification complexity.

**Expected Outcomes:**
- Dual-core RV32IMACF CPU with full SMP support
- CLINT-based IPI mechanism
- Validated synchronization primitives (LR/SC, AMO)
- Foundation for future cache hierarchy and scale-out to 4+ cores

**Next Steps:**
1. Synthesize this research into an implementation plan
2. Create detailed RTL design for memory arbiter
3. Prototype CLINT as Rust `BusDevice`
4. Develop verification test plan for multi-hart synchronization

---

**Document Status:** Research complete, ready for implementation planning  
**Lifecycle:** Retain until implementation plan created in `docs/plans/`

use crate::memory::Memory;
use riscv_core::Top;

/// Result of a simulation run
#[derive(Debug)]
pub struct SimulationResult {
    pub cycles: u64,
    pub tohost_value: Option<u32>,
}

/// RISC-V CPU Simulator
pub struct Simulator<'a, F>
where
    F: FnMut(u8),
{
    cpu: Top<'a>,
    memory: Memory,
    cycle_count: u64,
    entry_point: u32,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
}

impl<'a, F> Simulator<'a, F>
where
    F: FnMut(u8),
{
    /// Create a new simulator with the given memory, runtime, and entry point
    pub fn new(
        runtime: &'a riscv_core::VerilatorRuntime,
        memory: Memory,
        entry_point: u32,
        print_inst_trace: bool,
    ) -> Result<Self, String> {
        // Create CPU model from the runtime
        let cpu = runtime
            .create_model_simple::<Top>()
            .map_err(|e| format!("Failed to create CPU model: {}", e))?;

        Ok(Simulator {
            cpu,
            memory,
            cycle_count: 0,
            entry_point,
            print_inst_trace,
            fifo_callback: None,
        })
    }

    /// Set a callback to be invoked when data is written to the FIFO
    pub fn set_fifo_callback(&mut self, callback: F) {
        self.fifo_callback = Some(callback);
    }

    /// Reset the CPU
    /// The boot address is set to the entry point while reset is asserted so that
    /// the PC samples this value through the asynchronous reset and then holds it
    /// when reset is released.
    pub fn reset(&mut self) {
        // Set the boot address BEFORE asserting and during reset
        // This is critical because the PC register uses an asynchronous reset that
        // loads boot_addr whenever rst_n is low; boot_addr must be stable while
        // reset is asserted so the PC will hold this value after reset is released.
        self.cpu.boot_addr = self.entry_point;

        // Initialize FIFO signals
        self.cpu.fifo_rd_en = 0;

        // Drive reset low
        self.cpu.rst_n = 0;
        self.cpu.clk = 0;
        self.cpu.eval();
        self.cpu.clk = 1;
        self.cpu.eval();

        // Release reset
        self.cpu.rst_n = 1;
        self.cpu.clk = 0;
        self.cpu.eval();

        log::info!(
            "CPU reset complete with entry point: 0x{:08x}",
            self.entry_point
        );
    }

    /// Drain all data from the FIFO and invoke the callback for each byte
    pub fn drain_fifo(&mut self) {
        while self.cpu.fifo_empty == 0 {
            let data = self.cpu.fifo_rd_data;
            log::debug!("FIFO drain: 0x{:02x} ('{}')", data, data as char);
            
            // Invoke callback if set
            if let Some(ref mut callback) = self.fifo_callback {
                callback(data);
            }
            
            // Assert rd_en and clock to pop the FIFO
            self.cpu.fifo_rd_en = 1;
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();
            
            // Deassert rd_en
            self.cpu.fifo_rd_en = 0;
            self.cpu.eval();
        }
    }

    /// Run the simulation for up to max_cycles
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    pub fn run(&mut self, max_cycles: u64) -> Result<SimulationResult, String> {
        self.reset();

        // Magic address for halt signal (tohost mechanism)
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        log::info!("Starting simulation (max {} cycles)", max_cycles);

        while self.cycle_count < max_cycles {
            // Instruction Fetch
            let pc = self.cpu.imem_addr;
            let instruction = self.memory.read_word(pc);
            self.cpu.imem_data = instruction;

            // First evaluation: Decode instruction and compute addresses
            // This eval() propagates the new instruction through the combinational
            // logic, computing outputs like dmem_addr (for load/store operations),
            // dmem_we, dmem_wdata, etc.
            self.cpu.eval();

            // Data Memory Read (use address from THIS cycle's computation)
            // After the first eval, dmem_addr contains the data memory address
            // computed by the current instruction (for load/store operations)
            let dmem_addr = self.cpu.dmem_addr;
            let rdata = self.memory.read_word(dmem_addr);
            self.cpu.dmem_rdata = rdata;

            // Second evaluation: Propagate loaded data to rd_data
            // For load instructions, this eval() propagates dmem_rdata through the
            // combinational path to rd_data so it can be written to the register file
            // on the next clock edge. This is necessary because Verilator requires
            // explicit eval() calls to propagate combinational logic changes.
            self.cpu.eval();

            // Data Memory Write
            // dmem_we and dmem_wdata are stable after eval
            if self.cpu.dmem_we != 0 {
                let wdata = self.cpu.dmem_wdata;
                
                // MMIO region check (0xF0000000 - 0xF000000F)
                const MMIO_BASE: u32 = 0xF0000000;
                const MMIO_SIZE: u32 = 0x10;
                let is_mmio = dmem_addr >= MMIO_BASE && dmem_addr < (MMIO_BASE + MMIO_SIZE);
                
                // Only write to memory if not MMIO
                if !is_mmio {
                    self.memory.write_word(dmem_addr, wdata);
                    log::debug!(
                        "Memory Write: addr=0x{:08x}, data=0x{:08x}",
                        dmem_addr,
                        wdata
                    );
                } else {
                    log::debug!(
                        "MMIO Write: addr=0x{:08x}, data=0x{:08x}",
                        dmem_addr,
                        wdata
                    );
                }

                // Check for halt signal
                if dmem_addr == TOHOST_ADDR {
                    log::info!(
                        "Halt signal detected at tohost (0x{:08x}), value=0x{:08x}",
                        TOHOST_ADDR,
                        wdata
                    );
                    // Drain FIFO before returning
                    self.drain_fifo();
                    return Ok(SimulationResult {
                        cycles: self.cycle_count,
                        tohost_value: Some(wdata),
                    });
                }
            }

            // Clock tick
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();

            // Debug logging: print after evaluation to capture rd_data
            if self.print_inst_trace {
                let rs1_value = self.cpu.debug_rs1_data;
                let rs2_value = self.cpu.debug_rs2_data;
                let rd_value = self.cpu.debug_rd_data;
                let disassembled = riscv_core::disasm::disassemble_with_all_values(
                    instruction,
                    rs1_value,
                    rs2_value,
                    rd_value,
                );
                println!(
                    "Cycle {:6} | PC: 0x{:08x} | Addr: 0x{:08x} | Instr: 0x{:08x} | {}",
                    self.cycle_count, pc, pc, instruction, disassembled
                );
            }

            // Log execution (original verbose logging)
            if !self.print_inst_trace
                && (self.cycle_count.is_multiple_of(1000) || log::log_enabled!(log::Level::Debug))
            {
                log::debug!(
                    "Cycle {}: PC=0x{:08x}, Instr=0x{:08x}",
                    self.cycle_count,
                    pc,
                    instruction
                );
            }

            self.cycle_count += 1;
        }

        log::warn!("Simulation reached max cycles ({})", max_cycles);
        // Drain FIFO before returning
        self.drain_fifo();
        Ok(SimulationResult {
            cycles: self.cycle_count,
            tohost_value: None,
        })
    }
}

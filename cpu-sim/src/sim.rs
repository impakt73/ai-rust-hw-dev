use crate::memory::Memory;
use riscv_core::Top;

/// RISC-V CPU Simulator
pub struct Simulator<'a> {
    cpu: Top<'a>,
    memory: Memory,
    cycle_count: u64,
}

impl<'a> Simulator<'a> {
    /// Create a new simulator with the given memory and runtime
    pub fn new(runtime: &'a riscv_core::VerilatorRuntime, memory: Memory) -> Self {
        // Create CPU model from the runtime
        let cpu = runtime.create_model_simple::<Top>().unwrap();

        Simulator {
            cpu,
            memory,
            cycle_count: 0,
        }
    }

    /// Reset the CPU
    pub fn reset(&mut self) {
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
        self.cpu.clk = 1;
        self.cpu.eval();

        log::info!("CPU reset complete");
    }

    /// Run the simulation for up to max_cycles
    /// Returns Ok(cycles) on normal completion or Err on error
    pub fn run(&mut self, max_cycles: u64) -> Result<u64, String> {
        self.reset();

        // Magic address for halt signal (tohost mechanism)
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        log::info!("Starting simulation (max {} cycles)", max_cycles);

        while self.cycle_count < max_cycles {
            // Instruction Fetch
            let pc = self.cpu.imem_addr;
            let instruction = self.memory.read_word(pc);
            self.cpu.imem_data = instruction;

            // Log execution
            if self.cycle_count % 1000 == 0 || log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "Cycle {}: PC=0x{:08x}, Instr=0x{:08x}",
                    self.cycle_count,
                    pc,
                    instruction
                );
            }

            // Data Memory Access
            let dmem_addr = self.cpu.dmem_addr;

            // Handle write
            if self.cpu.dmem_we != 0 {
                let wdata = self.cpu.dmem_wdata;
                self.memory.write_word(dmem_addr, wdata);
                log::debug!(
                    "Memory Write: addr=0x{:08x}, data=0x{:08x}",
                    dmem_addr,
                    wdata
                );

                // Check for halt signal
                if dmem_addr == TOHOST_ADDR {
                    log::info!(
                        "Halt signal detected at tohost (0x{:08x}), value=0x{:08x}",
                        TOHOST_ADDR,
                        wdata
                    );
                    return Ok(self.cycle_count);
                }
            }

            // Always read from memory (for loads)
            let rdata = self.memory.read_word(dmem_addr);
            self.cpu.dmem_rdata = rdata;

            // Clock tick
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();

            self.cycle_count += 1;
        }

        log::warn!("Simulation reached max cycles ({})", max_cycles);
        Ok(self.cycle_count)
    }

    /// Get the current cycle count
    pub fn get_cycle_count(&self) -> u64 {
        self.cycle_count
    }
}

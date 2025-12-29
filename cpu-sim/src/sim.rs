use crate::memory::Memory;
use riscv_core::Top;

/// RISC-V CPU Simulator
pub struct Simulator<'a> {
    cpu: Top<'a>,
    memory: Memory,
    cycle_count: u64,
    entry_point: u32,
    print_inst_trace: bool,
}

impl<'a> Simulator<'a> {
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
        })
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

            // First evaluation: Decode instruction and compute addresses
            self.cpu.eval();

            // Data Memory Read (use address from THIS cycle's computation)
            // After eval, dmem_addr reflects the current instruction's address
            let dmem_addr = self.cpu.dmem_addr;
            let rdata = self.memory.read_word(dmem_addr);
            self.cpu.dmem_rdata = rdata;

            // Second evaluation: Propagate loaded data to rd_data
            self.cpu.eval();

            // Data Memory Write
            // dmem_we and dmem_wdata are stable after eval
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
        Ok(self.cycle_count)
    }
}

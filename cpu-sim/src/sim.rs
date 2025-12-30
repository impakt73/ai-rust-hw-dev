use crate::bus::SystemBus;
use riscv_core::trace::InstructionTrace;
use riscv_core::Top;

/// Result of a simulation run
#[derive(Debug)]
pub struct SimulationResult {
    pub cycles: u64,
    pub tohost_value: Option<u32>,
}

/// RISC-V CPU Simulator
pub struct Simulator<'a, F, T>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
{
    cpu: Top<'a>,
    pub bus: SystemBus,
    cycle_count: u64,
    entry_point: u32,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    trace_callback: Option<T>,
}

impl<'a, F, T> Simulator<'a, F, T>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
{
    /// Create a new simulator with the given bus, runtime, entry point, and optional callbacks
    pub fn new(
        runtime: &'a riscv_core::VerilatorRuntime,
        bus: SystemBus,
        entry_point: u32,
        print_inst_trace: bool,
        fifo_callback: Option<F>,
        trace_callback: Option<T>,
    ) -> Result<Self, String> {
        // Create CPU model from the runtime
        let cpu = runtime
            .create_model_simple::<Top>()
            .map_err(|e| format!("Failed to create CPU model: {}", e))?;

        Ok(Simulator {
            cpu,
            bus,
            cycle_count: 0,
            entry_point,
            print_inst_trace,
            fifo_callback,
            trace_callback,
        })
    }

    /// Write a u32 word to the FIFO RX queue (host-to-CPU direction)
    /// This allows the host to send data to the simulated program
    pub fn fifo_write_rx(&mut self, word: u32) {
        self.bus.fifo.rx.push_back(word);
    }

    /// Write a string to the FIFO RX queue
    /// Chunks the string into u32 words with zero-padding and adds a null terminator
    pub fn fifo_write_rx_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let mut i = 0;

        // Write all complete words
        while i < bytes.len() {
            let mut word: u32 = 0;

            // Pack up to 4 bytes into a u32 word (little-endian)
            for j in 0..4 {
                if i + j < bytes.len() {
                    word |= (bytes[i + j] as u32) << (j * 8);
                }
                // Remaining bytes are implicitly 0 (zero-padding)
            }

            self.fifo_write_rx(word);
            i += 4;
        }

        // Add a null terminator word if the string ends on a word boundary
        // This ensures the reading side can detect the end of the string
        if bytes.len().is_multiple_of(4) {
            self.fifo_write_rx(0);
        }
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
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    pub fn run(&mut self, max_cycles: u64) -> Result<SimulationResult, String> {
        self.reset();

        // Magic address for halt signal (tohost mechanism)
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        log::info!("Starting simulation (max {} cycles)", max_cycles);

        while self.cycle_count < max_cycles {
            // Instruction Fetch
            let pc = self.cpu.imem_addr;
            let instruction = self.bus.read_word(pc);
            self.cpu.imem_data = instruction;

            // First evaluation: Decode instruction and compute addresses
            // This eval() propagates the new instruction through the combinational
            // logic, computing outputs like dmem_addr (for load/store operations),
            // dmem_we, dmem_wdata, etc.
            self.cpu.eval();

            // Data Memory Read (use address from THIS cycle's computation)
            // After the first eval, dmem_addr contains the data memory address
            // computed by the instruction just decoded (for load/store operations).
            // Only read if this is NOT a write instruction (dmem_we == 0) to avoid
            // spurious reads with side effects (e.g., draining FIFO queues).
            let dmem_addr = self.cpu.dmem_addr;
            let rdata = if self.cpu.dmem_we == 0 {
                self.bus.read_word(dmem_addr)
            } else {
                0 // For stores, rdata is not used by the CPU and should not trigger side effects
            };
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
                self.bus.write_word(dmem_addr, wdata);
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
                    return Ok(SimulationResult {
                        cycles: self.cycle_count,
                        tohost_value: Some(wdata),
                    });
                }
            }

            // Sample debug signals BEFORE clock tick to capture the values that were
            // actually used during instruction execution, not the values after the write.
            // This is critical for correctness when rd == rs1 or rd == rs2.
            let rs1_value = self.cpu.debug_rs1_data;
            let rs2_value = self.cpu.debug_rs2_data;
            let rd_value = self.cpu.debug_rd_data;

            // Create instruction trace structure
            let trace = InstructionTrace::from_instruction(
                pc,
                instruction,
                rs1_value,
                rs2_value,
                rd_value,
            );

            // Clock tick
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();

            // Process FIFO TX data
            while let Some(word) = self.bus.fifo.tx.pop_front() {
                if let Some(ref mut callback) = self.fifo_callback {
                    callback(word);
                }
                // If no callback, just clear the buffer (don't print)
            }

            // Call trace callback if provided
            if let Some(ref mut callback) = self.trace_callback {
                callback(&trace);
            }

            // Debug logging: print using the trace structure for backward compatibility
            if self.print_inst_trace {
                println!(
                    "Cycle {:6} | PC: 0x{:08x} | Addr: 0x{:08x} | Instr: 0x{:08x} | {}",
                    self.cycle_count, pc, pc, instruction, trace
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
        Ok(SimulationResult {
            cycles: self.cycle_count,
            tohost_value: None,
        })
    }
}

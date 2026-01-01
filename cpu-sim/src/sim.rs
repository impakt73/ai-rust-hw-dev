use crate::bus::SystemBus;
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, Vcd, VerilatedModelConfig};
use riscv_protocol::*;

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
    print_debug_packets: bool,
    fifo_callback: Option<F>,
    trace_callback: Option<T>,
    vcd: Option<Vcd<'a>>,
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
        // Create CPU model from the runtime (without tracing by default)
        let cpu = runtime
            .create_model_simple::<Top>()
            .map_err(|e| format!("Failed to create CPU model: {}", e))?;

        Ok(Simulator {
            cpu,
            bus,
            cycle_count: 0,
            entry_point,
            print_inst_trace,
            print_debug_packets: true, // Enable by default
            fifo_callback,
            trace_callback,
            vcd: None,
        })
    }

    /// Create a new simulator with VCD tracing enabled
    pub fn new_with_vcd(
        runtime: &'a riscv_core::VerilatorRuntime,
        bus: SystemBus,
        entry_point: u32,
        print_inst_trace: bool,
        fifo_callback: Option<F>,
        trace_callback: Option<T>,
        vcd_path: &str,
    ) -> Result<Self, String> {
        // Create CPU model with tracing enabled
        let config = VerilatedModelConfig {
            enable_tracing: true,
            ..Default::default()
        };

        let mut cpu = runtime
            .create_model::<Top>(&config)
            .map_err(|e| format!("Failed to create CPU model with tracing: {}", e))?;

        // Open VCD file
        let vcd = cpu.open_vcd(vcd_path);
        log::info!("VCD tracing enabled, writing to: {}", vcd_path);

        Ok(Simulator {
            cpu,
            bus,
            cycle_count: 0,
            entry_point,
            print_inst_trace,
            print_debug_packets: true,
            fifo_callback,
            trace_callback,
            vcd: Some(vcd),
        })
    }

    /// Enable or disable automatic printing of DebugPacket messages
    pub fn set_print_debug_packets(&mut self, enable: bool) {
        self.print_debug_packets = enable;
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
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(0); // Capture initial state with reset asserted, clk=0
        }

        // First clock edge during reset
        self.cpu.clk = 1;
        self.cpu.eval();
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(1); // Capture state after rising edge during reset
        }

        // Second clock cycle during reset (falling edge)
        self.cpu.clk = 0;
        self.cpu.eval();
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(2); // Capture state after falling edge during reset
        }

        // Release reset (still at clk=0)
        self.cpu.rst_n = 1;
        self.cpu.eval();
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(3); // Capture state with reset released
        }

        log::info!(
            "CPU reset complete with entry point: 0x{:08x}",
            self.entry_point
        );
    }

    /// Execute a single simulation step (one instruction, multi-cycle)
    /// Returns Some(tohost_value) if halt detected, None otherwise
    pub fn step(&mut self) -> Option<u32> {
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
        const MAX_CYCLES_PER_INSTR: u64 = 100;

        let mut cycles_this_instr = 0;
        let mut halt_value = None;

        loop {
            cycles_this_instr += 1;
            if cycles_this_instr > MAX_CYCLES_PER_INSTR {
                log::error!(
                    "Instruction taking too long (>{} cycles); breaking",
                    MAX_CYCLES_PER_INSTR
                );
                break;
            }

            // Instruction Fetch
            let pc = self.cpu.imem_addr;
            let instruction = self.bus.read_word(pc);
            self.cpu.imem_data = instruction;

            // First evaluation: combinational logic
            self.cpu.eval();

            // Data Memory Read (every cycle if asserted)
            if self.cpu.dmem_re != 0 {
                let dmem_addr = self.cpu.dmem_addr;
                let dmem_size = self.cpu.dmem_size;
                let rdata = match dmem_size {
                    0b00 => self.bus.read_byte(dmem_addr) as u32,
                    0b01 => self.bus.read_halfword(dmem_addr) as u32,
                    _ => self.bus.read_word(dmem_addr),
                };
                self.cpu.dmem_rdata = rdata;
                self.cpu.eval();
            }

            // Data Memory Write (every cycle if asserted)
            if self.cpu.dmem_we != 0 {
                let dmem_addr = self.cpu.dmem_addr;
                let dmem_size = self.cpu.dmem_size;
                let wdata = self.cpu.dmem_wdata;

                match dmem_size {
                    0b00 => self.bus.write_byte(dmem_addr, wdata as u8),
                    0b01 => self.bus.write_halfword(dmem_addr, wdata as u16),
                    _ => self.bus.write_word(dmem_addr, wdata),
                }

                if dmem_addr == TOHOST_ADDR {
                    halt_value = Some(wdata);
                }
            }

            // Sample debug signals BEFORE clock tick
            let trace = if self.print_inst_trace || self.trace_callback.is_some() {
                let rs1_value = self.cpu.debug_rs1_data;
                let rs2_value = self.cpu.debug_rs2_data;
                let rd_value = self.cpu.debug_rd_data;
                Some(InstructionTrace::from_instruction(
                    pc,
                    instruction,
                    rs1_value,
                    rs2_value,
                    rd_value,
                ))
            } else {
                None
            };

            let instr_done = self.cpu.instr_complete != 0 || self.cpu.halted != 0;

            // Clock tick
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();

            // Increment cycle count before dumping to VCD
            self.cycle_count += 1;

            // Dump VCD if enabled (after clock edge, with proper timestamp)
            // Reset sequence uses timestamps 0-3, so execution cycles start at 4
            if let Some(ref mut vcd) = self.vcd {
                vcd.dump(self.cycle_count + 3);
            }

            // Process FIFO TX data
            if let Some(ref mut callback) = self.fifo_callback {
                while let Some(word) = self.bus.fifo.tx.pop_front() {
                    callback(word);
                }
            } else if self.print_debug_packets {
                while let Ok(Some(debug_pkt)) = self.try_receive_debug_packet() {
                    let level_str = match debug_pkt.level {
                        DebugLevel::Trace => "[TRACE]",
                        DebugLevel::Debug => "[DEBUG]",
                        DebugLevel::Info => "[INFO]",
                        DebugLevel::Warning => "[WARN]",
                        DebugLevel::Error => "[ERROR]",
                    };
                    println!("{} {}", level_str, debug_pkt.message);
                }
            } else {
                while self.bus.fifo.tx.pop_front().is_some() {}
            }

            // Call trace callback if provided
            if let Some(ref mut callback) = self.trace_callback {
                if let Some(ref trace_data) = trace {
                    callback(trace_data);
                }
            }

            // Debug logging
            if self.print_inst_trace {
                if let Some(ref trace_data) = trace {
                    println!(
                        "Cycle {:6} | PC: 0x{:08x} | Addr: 0x{:08x} | Instr: 0x{:08x} | {}",
                        self.cycle_count, pc, pc, instruction, trace_data
                    );
                }
            }

            // Exit when instruction completes or CPU halts
            if instr_done || self.cpu.instr_complete != 0 || self.cpu.halted != 0 {
                break;
            }
        }

        halt_value
    }

    /// Run the simulation for up to max_cycles
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    pub fn run(&mut self, max_cycles: u64) -> Result<SimulationResult, String> {
        self.reset();

        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        log::info!("Starting simulation (max {} cycles)", max_cycles);

        while self.cycle_count < max_cycles {
            // Execute one step and check for halt
            if let Some(tohost_value) = self.step() {
                log::info!(
                    "Halt signal detected at tohost (0x{:08x}), value=0x{:08x}",
                    TOHOST_ADDR,
                    tohost_value
                );
                return Ok(SimulationResult {
                    cycles: self.cycle_count,
                    tohost_value: Some(tohost_value),
                });
            }

            // Log execution periodically for debugging
            if !self.print_inst_trace
                && (self.cycle_count.is_multiple_of(1000) || log::log_enabled!(log::Level::Debug))
            {
                log::debug!(
                    "Cycle {}: PC=0x{:08x}",
                    self.cycle_count,
                    self.cpu.imem_addr
                );
            }
        }

        log::warn!("Simulation reached max cycles ({})", max_cycles);
        Ok(SimulationResult {
            cycles: self.cycle_count,
            tohost_value: None,
        })
    }

    /// Send an Echo packet to the simulated CPU
    pub fn send_echo_packet(&mut self, packet: &EchoPacket) -> Result<(), String> {
        crate::packet_transport::send_echo_packet(packet, &mut self.bus.fifo.rx)
    }

    /// Send a DataU32 packet to the simulated CPU
    pub fn send_data_u32_packet(&mut self, packet: &DataU32Packet) -> Result<(), String> {
        crate::packet_transport::send_data_u32_packet(packet, &mut self.bus.fifo.rx)
    }

    /// Try to receive an Echo packet from the simulated CPU
    pub fn try_receive_echo_packet(&mut self) -> Result<Option<EchoPacket>, String> {
        crate::packet_transport::receive_echo_packet(&mut self.bus.fifo.tx)
    }

    /// Try to receive a DataU32 packet from the simulated CPU
    pub fn try_receive_data_u32_packet(&mut self) -> Result<Option<DataU32Packet>, String> {
        crate::packet_transport::receive_data_u32_packet(&mut self.bus.fifo.tx)
    }

    /// Try to receive a Debug packet from the simulated CPU
    pub fn try_receive_debug_packet(&mut self) -> Result<Option<DebugPacket>, String> {
        crate::packet_transport::receive_debug_packet(&mut self.bus.fifo.tx)
    }

    /// Try to receive an Assert packet from the simulated CPU
    pub fn try_receive_assert_packet(&mut self) -> Result<Option<AssertPacket>, String> {
        crate::packet_transport::receive_assert_packet(&mut self.bus.fifo.tx)
    }
}

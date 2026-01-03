use crate::bus::SystemBus;
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, Vcd, VerilatedModelConfig};
use riscv_protocol::*;
use std::path::Path;

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
    /// Create a new simulator with the given bus, runtime, and optional callbacks
    pub fn new(
        runtime: &'a riscv_core::VerilatorRuntime,
        bus: SystemBus,
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
    /// The boot address is set to the boot_pc while reset is asserted so that
    /// the PC samples this value through the asynchronous reset and then holds it
    /// when reset is released.
    ///
    /// # Arguments
    /// * `boot_pc` - The program counter value to start execution from
    pub fn reset(&mut self, boot_pc: u32) {
        // Set the boot address BEFORE asserting and during reset
        // This is critical because the PC register uses an asynchronous reset that
        // loads boot_addr whenever rst_n is low; boot_addr must be stable while
        // reset is asserted so the PC will hold this value after reset is released.
        self.cpu.boot_addr = boot_pc;

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

        log::info!("CPU reset complete with boot PC: 0x{:08x}", boot_pc);
    }

    /// Execute a single simulation step (one instruction - may take multiple cycles)
    /// Returns Some(tohost_value) if halt detected, None otherwise
    pub fn step(&mut self) -> Option<u32> {
        // Magic address for halt signal (tohost mechanism)
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
        const MAX_CYCLES_PER_INSTR: u32 = 100;  // Safety limit for variable latency
        
        let mut cycles = 0;
        let mut halt_value = None;
        
        // Multi-cycle execution loop - continue until instruction completes
        loop {
            // Evaluate combinational logic
            self.cpu.eval();
            
            // Handle instruction memory with zero-latency (for now)
            // NOTE: This is a simplified, zero-latency placeholder for bring-up/testing.
            // Variable latency can be added by implementing a counter-based delay.
            if self.cpu.imem_req != 0 {
                let addr = self.cpu.imem_addr;
                let data = self.bus.read_word(addr);
                self.cpu.imem_data = data;
                self.cpu.imem_ready = 1;  // Always ready (zero latency)
            } else {
                self.cpu.imem_ready = 0;
            }
            
            // Handle data memory with zero-latency (for now)
            // NOTE: This is a simplified, zero-latency placeholder for bring-up/testing.
            // Variable latency can be added by implementing a counter-based delay.
            if self.cpu.dmem_req != 0 {
                let addr = self.cpu.dmem_addr;
                let size = self.cpu.dmem_size;
                
                if self.cpu.dmem_we != 0 {
                    // Data Memory Write
                    let wdata = self.cpu.dmem_wdata;
                    match size {
                        0b00 => self.bus.write_byte(addr, wdata as u8),
                        0b01 => self.bus.write_halfword(addr, wdata as u16),
                        _ => self.bus.write_word(addr, wdata),
                    }
                    
                    // Check for halt signal
                    if addr == TOHOST_ADDR {
                        halt_value = Some(wdata);
                    }
                    
                    self.cpu.dmem_ready = 1;  // Always ready (zero latency)
                } else if self.cpu.dmem_re != 0 {
                    // Data Memory Read
                    let rdata = match size {
                        0b00 => self.bus.read_byte(addr) as u32,
                        0b01 => self.bus.read_halfword(addr) as u32,
                        _ => self.bus.read_word(addr),
                    };
                    self.cpu.dmem_rdata = rdata;
                    self.cpu.dmem_ready = 1;  // Always ready (zero latency)
                } else {
                    self.cpu.dmem_ready = 0;
                }
            } else {
                self.cpu.dmem_ready = 0;
            }
            
            // Re-evaluate after setting memory signals
            self.cpu.eval();
            
            // Check if instruction complete
            if self.cpu.instr_complete != 0 {
                break;
            }
            
            // Safety check
            cycles += 1;
            if cycles >= MAX_CYCLES_PER_INSTR {
                panic!("Instruction exceeded maximum cycles ({})", MAX_CYCLES_PER_INSTR);
            }
            
            // Clock edge
            self.cpu.clk = 0;
            self.cpu.eval();
            self.cpu.clk = 1;
            self.cpu.eval();
            
            // Increment cycle count
            self.cycle_count += 1;
            
            // Dump VCD if enabled (after clock edge, with proper timestamp)
            // Reset sequence uses timestamps 0-3, so execution cycles start at 4
            if let Some(ref mut vcd) = self.vcd {
                vcd.dump(self.cycle_count + 3);
            }
        }
        
        // Final clock edge to complete instruction
        self.cpu.clk = 0;
        self.cpu.eval();
        self.cpu.clk = 1;
        self.cpu.eval();
        
        // Increment cycle count
        self.cycle_count += 1;
        
        // Dump VCD if enabled
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(self.cycle_count + 3);
        }
        
        // Process FIFO TX data
        // Strategy: drain FIFO via callback, or parse packets for printing, or just drain
        if let Some(ref mut callback) = self.fifo_callback {
            // Callback provided - drain FIFO and invoke callback for each word
            while let Some(word) = self.bus.fifo.tx.pop_front() {
                callback(word);
            }
        } else if self.print_debug_packets {
            // No callback but auto-printing enabled - parse and print DebugPackets
            while let Ok(Some(debug_pkt)) = self.try_receive_debug_packet() {
                // Format the message with level prefix
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
            // No callback and no auto-printing - drain FIFO to prevent accumulation
            while self.bus.fifo.tx.pop_front().is_some() {}
        }
        
        // Trace printing (simplified - only at instruction completion)
        if self.print_inst_trace {
            let pc = self.cpu.imem_addr;
            let instruction = self.cpu.imem_data;
            println!(
                "Cycle {:6} | PC: 0x{:08x} | Instr: 0x{:08x} | Cycles: {}",
                self.cycle_count, pc, instruction, cycles + 1
            );
        }
        
        // Call trace callback if provided (at instruction completion)
        if let Some(ref mut callback) = self.trace_callback {
            let pc = self.cpu.imem_addr;
            let instruction = self.cpu.imem_data;
            let rs1_value = self.cpu.debug_rs1_data;
            let rs2_value = self.cpu.debug_rs2_data;
            let rd_value = self.cpu.debug_rd_data;
            let trace = InstructionTrace::from_instruction(
                pc,
                instruction,
                rs1_value,
                rs2_value,
                rd_value,
            );
            callback(&trace);
        }

        halt_value
    }

    /// Run the simulation for up to max_cycles
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    ///
    /// # Arguments
    /// * `boot_pc` - The program counter value to start execution from
    /// * `max_cycles` - Maximum number of cycles to run
    pub fn run(&mut self, boot_pc: u32, max_cycles: u64) -> Result<SimulationResult, String> {
        self.reset(boot_pc);

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

    /// Write a region of memory from a byte slice
    ///
    /// Writes bytes from the provided slice into the memory region starting at `start_addr`.
    /// This allows external code to populate the simulator's memory with arbitrary data,
    /// such as programmatically generated instructions or test data.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address of the memory region to write
    /// * `data` - Byte slice containing the data to write
    ///
    /// # Examples
    /// ```
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// # let runtime = riscv_core::create_cpu_runtime().map_err(|e| e.to_string())?;
    /// # let bus = bus::SystemBus::new();
    /// let mut sim = Simulator::new(
    ///     &runtime,
    ///     bus,
    ///     false,
    ///     None::<fn(u32)>,
    ///     None::<fn(&riscv_core::trace::InstructionTrace)>,
    /// )?;
    /// let instructions = vec![0x13, 0x01, 0x00, 0x00]; // addi x2, x0, 0
    /// sim.write_memory_region(0x8000_0000, &instructions);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_memory_region(&mut self, start_addr: u32, data: &[u8]) {
        for (offset, &byte) in data.iter().enumerate() {
            let addr = start_addr.wrapping_add(offset as u32);
            self.bus.dram.write_byte(addr, byte);
        }
    }

    /// Dump a region of memory as a byte iterator
    ///
    /// Returns an iterator over bytes in the specified memory region.
    /// This allows efficient access without allocating a new buffer.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address of the memory region
    /// * `size` - Number of bytes to dump
    ///
    /// # Returns
    /// An iterator yielding bytes from the memory region
    ///
    /// # Examples
    /// ```
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// # let runtime = riscv_core::create_cpu_runtime().map_err(|e| e.to_string())?;
    /// # let bus = bus::SystemBus::new();
    /// let sim = Simulator::new(
    ///     &runtime,
    ///     bus,
    ///     false,
    ///     None::<fn(u32)>,
    ///     None::<fn(&riscv_core::trace::InstructionTrace)>,
    /// )?;
    /// let bytes: Vec<u8> = sim.dump_memory_region(0x8000_0000, 1024).collect();
    /// # Ok(())
    /// # }
    /// ```
    pub fn dump_memory_region(&self, start_addr: u32, size: u32) -> impl Iterator<Item = u8> + '_ {
        (0..size).map(move |offset| {
            let addr = start_addr.wrapping_add(offset);
            self.bus.dram.read_byte(addr)
        })
    }

    /// Dump a region of memory as an RGBA8 image
    ///
    /// Interprets the memory region as RGBA8 pixel data (4 bytes per pixel)
    /// and saves it as an image file. The format is determined by the file extension.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address of the memory region containing image data
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `output_path` - Path to the output image file (format determined by extension)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` on error
    ///
    /// # Requirements
    /// The memory region must contain at least `width * height * 4` bytes of valid data.
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// # let runtime = riscv_core::create_cpu_runtime().map_err(|e| e.to_string())?;
    /// # let bus = bus::SystemBus::new();
    /// let sim = Simulator::new(
    ///     &runtime,
    ///     bus,
    ///     false,
    ///     None::<fn(u32)>,
    ///     None::<fn(&riscv_core::trace::InstructionTrace)>,
    /// )?;
    /// sim.dump_memory_region_as_image(
    ///     0x8000_0000,
    ///     640,
    ///     480,
    ///     "output.png"
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn dump_memory_region_as_image(
        &self,
        start_addr: u32,
        width: u32,
        height: u32,
        output_path: &str,
    ) -> Result<(), String> {
        use image::{ImageBuffer, Rgba};

        // Calculate total bytes needed
        let pixel_count = width
            .checked_mul(height)
            .ok_or_else(|| "Image dimensions overflow".to_string())?;
        let total_bytes = pixel_count
            .checked_mul(4)
            .ok_or_else(|| "Image size overflow".to_string())?;

        // Collect pixel data from memory
        let pixel_data: Vec<u8> = self.dump_memory_region(start_addr, total_bytes).collect();

        // Create image buffer from raw RGBA8 data
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, pixel_data)
            .ok_or_else(|| {
                "Failed to create image buffer from pixel data (size mismatch)".to_string()
            })?;

        // Save the image
        img_buffer
            .save(Path::new(output_path))
            .map_err(|e| format!("Failed to save image: {}", e))?;

        log::info!("Image saved: {} ({}x{} RGBA8)", output_path, width, height);
        Ok(())
    }
}

use crate::bus::SystemBus;
use crate::hung_detector::{HungDetector, HungDetectorConfig, HungStateError};
use ouroboros::self_referencing;
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, VerilatedModelConfig, VerilatorRuntime};
use std::path::Path;
use std::time::Instant;

/// DRAM memory range: DRAM_BASE to DRAM_END (inclusive)
use crate::bus::{is_valid_dram_range, DRAM_BASE, DRAM_END};

/// Result of a single simulation step
#[derive(Debug)]
pub struct SimulationStepResult {
    pub tohost_value: Option<u32>,
    pub elapsed_cpu_time_us: u64,
}

/// Result of a simulation run
#[derive(Debug)]
pub struct SimulationResult {
    pub cycles: u64,
    pub tohost_value: Option<u32>,
    pub elapsed_cpu_time_us: u64,
}

/// Restricted view of the Simulator for use in callbacks
///
/// Provides controlled access to FIFO and memory operations without exposing
/// the full Simulator internals. This allows callbacks to interact with memory,
/// FIFO, and other simulator components while maintaining encapsulation.
pub struct SimulatorView<'a> {
    bus: &'a mut crate::bus::SystemBus,
    hung_detector: &'a mut Option<HungDetector>,
}

impl<'a> SimulatorView<'a> {
    /// Create a new SimulatorView with access to the given components
    pub(crate) fn new(
        bus: &'a mut crate::bus::SystemBus,
        hung_detector: &'a mut Option<HungDetector>,
    ) -> Self {
        SimulatorView { bus, hung_detector }
    }

    /// Read a word from the FIFO TX queue (CPU → Host)
    ///
    /// Returns `Some(word)` if data is available, `None` if the queue is empty.
    pub fn fifo_read_tx(&mut self) -> Option<u32> {
        self.bus.fifo.tx.pop_front()
    }

    /// Write a word to the FIFO RX queue (Host → CPU)
    ///
    /// This allows the host to send data to the simulated CPU.
    pub fn fifo_write_rx(&mut self, word: u32) {
        self.bus.fifo.rx.push_back(word);
    }

    /// Check if the FIFO TX queue (CPU → Host) is empty
    pub fn fifo_tx_is_empty(&self) -> bool {
        self.bus.fifo.tx.is_empty()
    }

    /// Check if the FIFO RX queue (Host → CPU) is empty
    pub fn fifo_rx_is_empty(&self) -> bool {
        self.bus.fifo.rx.is_empty()
    }

    /// Get the number of words in the FIFO TX queue (CPU → Host)
    pub fn fifo_tx_len(&self) -> usize {
        self.bus.fifo.tx.len()
    }

    /// Get the number of words in the FIFO RX queue (Host → CPU)
    pub fn fifo_rx_len(&self) -> usize {
        self.bus.fifo.rx.len()
    }

    /// Send a packet to the FIFO RX queue using the packet_transport module
    ///
    /// This is a convenience wrapper around packet_transport send functions.
    /// It serializes the packet and writes it to the RX queue.
    pub fn send_packet_to_rx<T: serde::Serialize>(&mut self, packet: &T) -> Result<(), String> {
        use postcard::to_allocvec;

        let bytes: Vec<u8> =
            to_allocvec(packet).map_err(|e| format!("Serialization failed: {:?}", e))?;

        let mut i = 0;
        while i < bytes.len() {
            let mut word: u32 = 0;
            for j in 0..4 {
                if i + j < bytes.len() {
                    word |= (bytes[i + j] as u32) << (j * 8);
                }
            }
            self.bus.fifo.rx.push_back(word);
            i += 4;
        }

        Ok(())
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

    /// Write a region of memory from a byte slice
    ///
    /// Writes bytes from the provided slice into the memory region starting at `start_addr`.
    /// This allows external code to populate the simulator's memory with arbitrary data,
    /// such as programmatically generated instructions or test data.
    ///
    /// If `is_instructions` is true, the memory range will be marked as valid for the PC
    /// (program counter) for hung state detection purposes.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address of the memory region to write (absolute address)
    /// * `data` - Byte slice containing the data to write
    /// * `is_instructions` - If true, marks this region as valid for PC execution
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// // write_memory_region is typically used within run_program's setup_callback
    /// let instructions = vec![0x13, 0x01, 0x00, 0x00]; // addi x2, x0, 0
    /// let result = run_program(
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     None::<fn(&mut SimulatorView)>,
    ///     None::<fn(&InstructionTrace)>,
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     |sim| {
    ///         sim.write_memory_region(0x8000_0000, &instructions, true);
    ///         Ok(0x8000_0000)
    ///     },
    ///     None::<fn(&SimulatorView, &SimulationResult)>,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_memory_region(&mut self, start_addr: u32, data: &[u8], is_instructions: bool) {
        // Validate the entire range before writing
        if !data.is_empty() {
            let size = data.len() as u32;
            if !is_valid_dram_range(start_addr, size) {
                log::warn!(
                    "write_memory_region: Address range 0x{:08x} - 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), operation rejected",
                    start_addr,
                    start_addr.wrapping_add(size).wrapping_sub(1),
                    DRAM_BASE,
                    DRAM_END
                );
                return;
            }
        }

        // Write to memory using absolute addresses
        for (offset, &byte) in data.iter().enumerate() {
            let addr = start_addr.wrapping_add(offset as u32);
            self.bus.memory.write_byte(addr, byte);
        }

        // Update valid PC ranges for hung detection based on whether this is instruction or data memory
        if !data.is_empty() {
            if let Some(ref mut detector) = self.hung_detector {
                let new_start = start_addr;
                let new_end = start_addr.wrapping_add(data.len() as u32);
                detector.update_pc_range(new_start, new_end, is_instructions);
            }
        }
    }

    /// Dump a region of memory as a byte iterator
    ///
    /// Returns an iterator over bytes in the specified memory region.
    /// This allows efficient access without allocating a new buffer.
    ///
    /// **Validation:** This method validates that all reads fall within the valid
    /// DRAM range. Out-of-bounds reads return 0 and log warnings.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address of the memory region (absolute address, must be in DRAM range)
    /// * `size` - Number of bytes to dump
    ///
    /// # Returns
    /// An iterator yielding bytes from the memory region
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # use std::path::Path;
    /// # fn main() -> Result<(), String> {
    /// // dump_memory_region is typically used in run_elf's termination_callback
    /// run_elf(
    ///     Path::new("test.elf"),
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     None::<fn(&mut SimulatorView)>, // inst_complete_callback
    ///     None::<fn(&InstructionTrace)>, // trace_callback
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     None::<fn(&mut SimulatorView)>, // setup_callback
    ///     Some(|sim: &SimulatorView, _result: &SimulationResult| {
    ///         let bytes: Vec<u8> = sim.dump_memory_region(0x8000_0000, 1024).collect();
    ///         // Process bytes...
    ///     }),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn dump_memory_region(&self, start_addr: u32, size: u32) -> impl Iterator<Item = u8> + '_ {
        // Validate the entire range upfront
        let is_valid = if size > 0 {
            is_valid_dram_range(start_addr, size)
        } else {
            true // Empty range is valid
        };

        if !is_valid && size > 0 {
            log::warn!(
                "dump_memory_region: Address range 0x{:08x} - 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x})",
                start_addr,
                start_addr.wrapping_add(size).wrapping_sub(1),
                DRAM_BASE,
                DRAM_END
            );
        }

        // Dump from memory using absolute addresses.
        // The address range is validated once above; if it is invalid, this iterator
        // returns 0 without performing any memory reads.
        (0..size).map(move |offset| {
            let addr = start_addr.wrapping_add(offset);
            if is_valid {
                self.bus.memory.read_byte(addr)
            } else {
                0 // Return 0 for out-of-bounds reads
            }
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
    /// # use std::path::Path;
    /// # fn main() -> Result<(), String> {
    /// // dump_memory_region_as_image is typically used in run_elf's termination_callback
    /// run_elf(
    ///     Path::new("graphics.elf"),
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     None::<fn(&mut SimulatorView)>, // inst_complete_callback
    ///     None::<fn(&InstructionTrace)>, // trace_callback
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     None::<fn(&mut SimulatorView)>, // setup_callback
    ///     Some(|sim: &SimulatorView, _result: &SimulationResult| {
    ///         sim.dump_memory_region_as_image(0x8000_0000, 640, 480, "output.png")
    ///             .expect("Failed to dump image");
    ///     }),
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

    /// Read a single byte from memory
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_byte(&self, addr: u32) -> u8 {
        if !is_valid_dram_range(addr, 1) {
            log::warn!(
                "read_byte: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.bus.memory.read_byte(addr)
    }

    /// Read a 16-bit halfword from memory (little-endian)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_halfword(&self, addr: u32) -> u16 {
        if !is_valid_dram_range(addr, 2) {
            log::warn!(
                "read_halfword: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.bus.memory.read_halfword(addr)
    }

    /// Read a 32-bit word from memory (little-endian)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_word(&self, addr: u32) -> u32 {
        if !is_valid_dram_range(addr, 4) {
            log::warn!(
                "read_word: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.bus.memory.read_word(addr)
    }

    /// Register a custom device on the system bus
    ///
    /// This allows user code to register custom peripherals that will be
    /// accessible via the CPU's memory-mapped I/O.
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device in the system memory map
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(String)` - Address range conflicts with existing device
    ///
    /// # Example
    /// ```no_run
    /// use cpu_sim::*;
    ///
    /// # struct MyVideoDevice;
    /// # impl MyVideoDevice {
    /// #     fn new() -> Self { MyVideoDevice }
    /// # }
    /// # impl BusDevice for MyVideoDevice {
    /// #     fn read_word(&mut self, _ctx: &mut SystemContext, _offset: u32) -> Result<u32, BusDeviceError> { Ok(0) }
    /// #     fn write_word(&mut self, _ctx: &mut SystemContext, _offset: u32, _value: u32) -> Result<(), BusDeviceError> { Ok(()) }
    /// #     fn size(&self) -> u32 { 4 }
    /// # }
    /// run_program(
    ///     1000,
    ///     false,
    ///     false,
    ///     None::<fn(&mut SimulatorView)>,
    ///     None::<fn(&InstructionTrace)>,
    ///     None,
    ///     0,
    ///     |sim| {
    ///         // Register custom video device
    ///         let video_device = Box::new(MyVideoDevice::new());
    ///         sim.register_device(0x5000_0000, video_device)
    ///             .map_err(|e| format!("Failed to register device: {}", e))?;
    ///
    ///         // Load program
    ///         Ok(0x8000_0000)
    ///     },
    ///     None::<fn(&SimulatorView, &SimulationResult)>,
    /// )?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn crate::BusDevice>,
    ) -> Result<(), String> {
        self.bus
            .register_device(base_addr, device)
            .map_err(|e| format!("{}", e))
    }
}

/// Inner self-referencing structure that owns the runtime and CPU
#[self_referencing]
struct SimulatorInner {
    runtime: VerilatorRuntime,
    #[borrows(runtime)]
    #[covariant]
    cpu: Top<'this>,
}

/// RISC-V CPU Simulator
///
/// This structure now owns its runtime internally using the ouroboros crate,
/// eliminating the need for external lifetime management.
pub struct Simulator<F, T>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    inner: SimulatorInner,
    pub bus: SystemBus,
    cycle_count: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_time: u64, // VCD timestamp counter (incremented independently from cycle_count)
    // Memory latency simulation
    mem_latency_cycles: u32, // Number of cycles to delay memory operations
    imem_delay_counter: u32, // Current delay counter for instruction memory
    dmem_delay_counter: u32, // Current delay counter for data memory
    // Hung state detection
    pub(crate) hung_detector: Option<HungDetector>,
}

impl<F, T> Simulator<F, T>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    /// Create a new simulator with the given bus and optional callbacks
    ///
    /// The runtime is now created and owned internally by the Simulator,
    /// eliminating the need for external lifetime management.
    ///
    /// # Arguments
    /// * `bus` - System bus with memory and peripherals
    /// * `print_inst_trace` - Enable instruction trace printing
    /// * `print_fsm_state` - Enable FSM state printing
    /// * `inst_complete_callback` - Optional callback invoked after each instruction completes, receives a mutable `SimulatorView` providing controlled FIFO access
    /// * `trace_callback` - Optional callback for instruction traces
    /// * `vcd_path` - Optional path to VCD file for waveform tracing
    /// * `mem_latency_cycles` - Number of cycles to delay memory operations
    /// * `hung_detector_config` - Optional hung state detector configuration
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bus: SystemBus,
        print_inst_trace: bool,
        print_fsm_state: bool,
        inst_complete_callback: Option<F>,
        trace_callback: Option<T>,
        vcd_path: Option<&str>,
        mem_latency_cycles: u32,
        hung_detector_config: Option<HungDetectorConfig>,
    ) -> Result<Self, String> {
        // Create runtime
        let runtime = riscv_core::create_cpu_runtime()
            .map_err(|e| format!("Failed to create CPU runtime: {}", e))?;

        log::info!("Memory latency configured to {} cycles", mem_latency_cycles);

        let hung_detector = hung_detector_config.map(HungDetector::new);

        // Build the self-referencing structure (VCD initialized to None initially)
        let vcd_path_owned = vcd_path.map(|s| s.to_string());
        let inner = SimulatorInnerTryBuilder {
            runtime,
            cpu_builder: |runtime: &VerilatorRuntime| {
                // Create CPU model - enable tracing if VCD path is provided
                if vcd_path_owned.is_some() {
                    let config = VerilatedModelConfig {
                        enable_tracing: true,
                        ..Default::default()
                    };
                    runtime
                        .create_model::<Top>(&config)
                        .map_err(|e| format!("Failed to create CPU model with tracing: {}", e))
                } else {
                    runtime
                        .create_model_simple::<Top>()
                        .map_err(|e| format!("Failed to create CPU model: {}", e))
                }
            },
        }
        .try_build()?;

        // Now open VCD if requested using with_cpu_mut
        // Unfortunately, ouroboros doesn't allow us to easily store VCD which borrows from CPU
        // We'll need to manage VCD differently - for now, skip VCD support with ouroboros
        // or we need to use a more complex solution
        if vcd_path_owned.is_some() {
            log::warn!("VCD tracing is not yet supported with the new ouroboros-based simulator");
        }

        Ok(Simulator {
            inner,
            bus,
            cycle_count: 0,
            print_inst_trace,
            print_fsm_state,
            inst_complete_callback,
            trace_callback,
            vcd_time: 0,
            mem_latency_cycles,
            imem_delay_counter: 0,
            dmem_delay_counter: 0,
            hung_detector,
        })
    }

    /// Helper function to decode FSM state value to human-readable string
    fn fsm_state_name(state: u8) -> &'static str {
        match state {
            0 => "IDLE",
            1 => "FETCH",
            2 => "DECODE",
            3 => "EXECUTE",
            4 => "MEM_ADDR",
            5 => "MEM_READ",
            6 => "MEM_WRITE",
            7 => "WRITEBACK",
            8 => "BRANCH",
            9 => "CSR",
            10 => "HALT",
            _ => "UNKNOWN",
        }
    }

    /// Dump VCD waveform at current timestamp and increment the timestamp counter
    ///
    /// This is a helper function that handles VCD dumping if VCD tracing is enabled.
    /// It automatically increments the VCD timestamp after dumping.
    ///
    /// Note: VCD support is currently disabled with the ouroboros-based implementation.
    fn dump_vcd(&mut self) {
        // VCD tracing not yet supported with ouroboros
        self.vcd_time += 1;
    }

    /// Reset the CPU
    /// The boot address is set to the boot_pc while reset is asserted so that
    /// the PC samples this value through the asynchronous reset and then holds it
    /// when reset is released.
    ///
    /// # Arguments
    /// * `boot_pc` - The program counter value to start execution from
    ///
    /// # Returns
    /// * `Ok(())` if reset succeeds
    /// * `Err(HungStateError)` if the boot_pc is outside valid PC ranges
    pub fn reset(&mut self, boot_pc: u32) -> Result<(), HungStateError> {
        // Validate boot address before reset if hung detector is configured
        if let Some(ref detector) = self.hung_detector {
            detector.validate_boot_addr(boot_pc)?;
        }

        self.inner.with_cpu_mut(|cpu| {
            // Set the boot address BEFORE asserting and during reset
            // This is critical because the PC register uses an asynchronous reset that
            // loads boot_addr whenever rst_n is low; boot_addr must be stable while
            // reset is asserted so the PC will hold this value after reset is released.
            cpu.boot_addr = boot_pc;

            // Drive reset low
            cpu.rst_n = 0;
            cpu.clk = 0;
            cpu.eval();
        });
        self.dump_vcd(); // Capture initial state with reset asserted, clk=0

        // First clock edge during reset
        self.inner.with_cpu_mut(|cpu| {
            cpu.clk = 1;
            cpu.eval();
        });
        self.dump_vcd(); // Capture state after rising edge during reset

        // Second clock cycle during reset (falling edge)
        self.inner.with_cpu_mut(|cpu| {
            cpu.clk = 0;
            cpu.eval();
        });
        self.dump_vcd(); // Capture state after falling edge during reset

        // Release reset (still at clk=0)
        self.inner.with_cpu_mut(|cpu| {
            cpu.rst_n = 1;
            cpu.eval();
        });
        self.dump_vcd(); // Capture state with reset released

        // Reset the hung detector state
        if let Some(ref mut detector) = self.hung_detector {
            detector.reset();
        }

        // Reset all bus devices
        self.bus.reset_all_devices();

        log::info!("CPU reset complete with boot PC: 0x{:08x}", boot_pc);
        Ok(())
    }

    /// Execute a single simulation step (one instruction - may take multiple cycles)
    /// Returns SimulationStepResult containing:
    /// - tohost_value: Some(value) if halt detected, None otherwise
    /// - elapsed_cpu_time_us: CPU time elapsed during this step in microseconds
    ///
    /// # Errors
    /// Returns `HungStateError` if the CPU is detected to be in a hung state
    pub fn step(&mut self) -> Result<SimulationStepResult, HungStateError> {
        let start_time = Instant::now();

        // Multi-cycle execution loop - continue until instruction completes
        loop {
            let (instruction_complete, pc, instruction, fsm_state) = self.inner.with_cpu_mut(|cpu| {
                // Evaluate combinational logic
                cpu.eval();

                // Handle instruction memory with variable latency
                if cpu.imem_req != 0 {
                    // Implement delay counter for variable latency
                    if self.imem_delay_counter <= self.mem_latency_cycles {
                        if self.imem_delay_counter == self.mem_latency_cycles {
                            // Perform read on the cycle when we reach the threshold
                            let addr = cpu.imem_addr;
                            let data = self.bus.read_word(addr);
                            cpu.imem_data = data;
                            cpu.imem_ready = 1; // Ready after delay
                        } else {
                            self.imem_delay_counter += 1;
                            cpu.imem_ready = 0; // Not ready yet
                        }
                    } else {
                        // delay_counter > mem_latency_cycles: already completed, keep ready high
                        cpu.imem_ready = 1;
                    }
                } else {
                    cpu.imem_ready = 0;
                    self.imem_delay_counter = 0; // Reset counter when no request
                }

                // Handle data memory with variable latency
                if cpu.dmem_req != 0 {
                    if cpu.dmem_we != 0 {
                        // Data Memory Write
                        // Implement delay counter for variable latency
                        if self.dmem_delay_counter <= self.mem_latency_cycles {
                            if self.dmem_delay_counter == self.mem_latency_cycles {
                                // Perform write on the cycle when we reach the threshold
                                let addr = cpu.dmem_addr;
                                let size = cpu.dmem_size;
                                let wdata = cpu.dmem_wdata;

                                match size {
                                    0b00 => self.bus.write_byte(addr, wdata as u8),
                                    0b01 => self.bus.write_halfword(addr, wdata as u16),
                                    _ => self.bus.write_word(addr, wdata),
                                }

                                cpu.dmem_ready = 1; // Ready after delay
                            } else {
                                self.dmem_delay_counter += 1;
                                cpu.dmem_ready = 0; // Not ready yet
                            }
                        } else {
                            // delay_counter > mem_latency_cycles: already completed, keep ready high
                            cpu.dmem_ready = 1;
                        }
                    } else if cpu.dmem_re != 0 {
                        // Data Memory Read
                        // Implement delay counter for variable latency
                        if self.dmem_delay_counter <= self.mem_latency_cycles {
                            if self.dmem_delay_counter == self.mem_latency_cycles {
                                // Perform read on the cycle when we reach the threshold
                                let addr = cpu.dmem_addr;
                                let size = cpu.dmem_size;
                                let rdata = match size {
                                    0b00 => self.bus.read_byte(addr) as u32,
                                    0b01 => self.bus.read_halfword(addr) as u32,
                                    _ => self.bus.read_word(addr),
                                };

                                cpu.dmem_rdata = rdata;
                                cpu.dmem_ready = 1; // Ready after delay
                            } else {
                                self.dmem_delay_counter += 1;
                                cpu.dmem_ready = 0; // Not ready yet
                            }
                        } else {
                            // delay_counter > mem_latency_cycles: already completed, keep ready high
                            cpu.dmem_ready = 1;
                        }
                    } else {
                        cpu.dmem_ready = 0;
                    }
                } else {
                    cpu.dmem_ready = 0;
                    self.dmem_delay_counter = 0; // Reset counter when no request
                }

                // Re-evaluate after setting memory signals
                cpu.eval();

                // Print FSM state if enabled (before clock edge)
                if self.print_fsm_state {
                    let fsm_state = cpu.debug_fsm_state;
                    let state_name = Self::fsm_state_name(fsm_state);
                    println!(
                        "Cycle {:6} | State: {:10} | PC: 0x{:08x} | imem_req={} imem_ready={} | dmem_req={} dmem_ready={} | instr_complete={}",
                        self.cycle_count,
                        state_name,
                        cpu.imem_addr,
                        cpu.imem_req,
                        cpu.imem_ready,
                        cpu.dmem_req,
                        cpu.dmem_ready,
                        cpu.instr_complete
                    );
                }

                // Clock edge
                cpu.clk = 0;
                cpu.eval();
                cpu.clk = 1;
                cpu.eval();

                // Extract values needed for hung detection and trace
                let instruction_complete = cpu.instr_complete != 0;
                let pc = cpu.debug_current_pc;
                let instruction = cpu.debug_current_instruction;
                let fsm_state = cpu.debug_fsm_state;

                (instruction_complete, pc, instruction, fsm_state)
            });

            // Increment cycle count
            self.cycle_count += 1;

            // Dump VCD if enabled (after clock edge)
            self.dump_vcd();

            // Call clock_cycle on all bus devices (after clock edge completes)
            self.bus.clock_cycle_all_devices();

            // Check for hung state on every cycle
            // This detects stuck FSM, invalid PC, and PC loops (when instruction completes)
            if let Some(ref mut detector) = self.hung_detector {
                // Use current PC and instruction for hung detection (not completed ones)
                // debug_current_pc: PC that was used to fetch the current instruction
                // debug_current_instruction: The instruction currently being executed
                detector.check_cycle(
                    self.cycle_count,
                    pc,
                    instruction,
                    fsm_state,
                    instruction_complete,
                )?;
            }

            if instruction_complete {
                break;
            }
        }

        // Call inst_complete callback if provided (after instruction completion)
        // This callback receives restricted access to the Simulator via SimulatorView
        if let Some(ref mut callback) = self.inst_complete_callback {
            let mut view = SimulatorView::new(&mut self.bus, &mut self.hung_detector);
            callback(&mut view);
        }

        // Unified instruction trace handling
        // Check if trace callback is valid or instruction trace printing is enabled
        if self.trace_callback.is_some() || self.print_inst_trace {
            // Assemble InstructionTrace structure using debug signals from CPU
            let (pc, instruction, rs1_value, rs2_value, rd_value) = self.inner.with_cpu(|cpu| {
                (
                    cpu.debug_pc,
                    cpu.debug_instruction,
                    cpu.debug_rs1_data,
                    cpu.debug_rs2_data,
                    cpu.debug_rd_data,
                )
            });

            let trace =
                InstructionTrace::from_instruction(pc, instruction, rs1_value, rs2_value, rd_value);

            // Print the display version of the structure if printing is enabled
            if self.print_inst_trace {
                println!(
                    "Cycle {:6} | PC: 0x{:08x} | {}",
                    self.cycle_count, pc, trace
                );
            }

            // Call the trace callback with the structure if the callback is valid
            if let Some(ref mut callback) = self.trace_callback {
                callback(&trace);
            }
        }

        // Check for termination via SimControl device
        let halt_value = self.bus.sim_control.termination_requested();

        let elapsed_us = start_time.elapsed().as_micros() as u64;
        Ok(SimulationStepResult {
            tohost_value: halt_value,
            elapsed_cpu_time_us: elapsed_us,
        })
    }

    /// Run the simulation for up to max_cycles
    ///
    /// **Note:** This method performs a CPU reset internally before starting execution,
    /// so callers do not need to call `reset()` before calling `run()`.
    ///
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    ///
    /// # Arguments
    /// * `boot_pc` - The program counter value to start execution from
    /// * `max_cycles` - Maximum number of cycles to run
    ///
    /// # Errors
    /// Returns error if hung state is detected or other simulation errors occur
    pub fn run(&mut self, boot_pc: u32, max_cycles: u64) -> Result<SimulationResult, String> {
        self.reset(boot_pc)
            .map_err(|e| format!("Reset failed: {}", e))?;

        log::info!("Starting simulation (max {} cycles)", max_cycles);

        let mut total_elapsed_us: u64 = 0;

        while self.cycle_count < max_cycles {
            // Execute one step and check for halt
            let step_result = self
                .step()
                .map_err(|e| format!("Hung state detected: {}", e))?;
            total_elapsed_us = total_elapsed_us.saturating_add(step_result.elapsed_cpu_time_us);

            if let Some(tohost_value) = step_result.tohost_value {
                log::info!(
                    "Halt signal detected via SimControl, value=0x{:08x}",
                    tohost_value
                );
                return Ok(SimulationResult {
                    cycles: self.cycle_count,
                    tohost_value: Some(tohost_value),
                    elapsed_cpu_time_us: total_elapsed_us,
                });
            }

            // Log execution periodically for debugging
            if !self.print_inst_trace
                && (self.cycle_count.is_multiple_of(1000) || log::log_enabled!(log::Level::Debug))
            {
                let imem_addr = self.inner.with_cpu(|cpu| cpu.imem_addr);
                log::debug!("Cycle {}: PC=0x{:08x}", self.cycle_count, imem_addr);
            }
        }

        log::warn!("Simulation reached max cycles ({})", max_cycles);
        Ok(SimulationResult {
            cycles: self.cycle_count,
            tohost_value: None,
            elapsed_cpu_time_us: total_elapsed_us,
        })
    }
}

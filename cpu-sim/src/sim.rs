use crate::bus::SystemBus;
use crate::hung_detector::{HungDetector, HungDetectorConfig, HungStateError};
use host_bus_handler::{AccessSize, BusRequest, BusResponse, HostBusHandler};
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, Vcd, VerilatedModelConfig, VerilatorRuntime};
use std::path::Path;
use std::time::Instant;

/// DRAM memory range: DRAM_BASE to DRAM_END (inclusive)
use crate::bus::{is_valid_dram_range, DRAM_BASE, DRAM_END};

/// Pending response for a host-initiated request (awaiting completion after latency)
#[derive(Debug, Clone)]
struct PendingResponse {
    /// The bus response to send
    response: BusResponse,
    /// Cycle at which the request was accepted
    accepted_cycle: u64,
}

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
    cpu: &'a Top<'static>,
    host_bus_handler: &'a mut HostBusHandler,
}

impl<'a> SimulatorView<'a> {
    /// Create a new SimulatorView with access to the given components
    pub(crate) fn new(
        bus: &'a mut crate::bus::SystemBus,
        hung_detector: &'a mut Option<HungDetector>,
        cpu: &'a Top<'static>,
        host_bus_handler: &'a mut HostBusHandler,
    ) -> Self {
        SimulatorView {
            bus,
            hung_detector,
            cpu,
            host_bus_handler,
        }
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

    /// Get the current LED output value from the LED controller peripheral
    ///
    /// Returns the 8-bit LED output value from the LED controller peripheral
    /// at address 0x50000000.
    ///
    /// # Returns
    /// The current 8-bit LED output value
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// run_program(
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     None::<fn(&mut SimulatorView)>,
    ///     None::<fn(&InstructionTrace)>,
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     |_sim| Ok(0x8000_0000),
    ///     Some(|sim: &SimulatorView, _result: &SimulationResult| {
    ///         let led_value = sim.led_out();
    ///         println!("LED output: 0x{:02x}", led_value);
    ///     }),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn led_out(&self) -> u8 {
        self.cpu.led_out
    }

    /// Send a bus request from the host to the RTL target
    ///
    /// The request will be processed by the RTL host_bus_interface module
    /// and routed through the bus arbiter to the appropriate peripheral.
    ///
    /// # Arguments
    /// * `request` - Bus request (read or write) to send to the RTL target
    ///
    /// # Returns
    /// * `Ok(())` - Request queued successfully
    /// * `Err(String)` - Request rejected (already pending, or invalid address)
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// run_program(
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     Some(|sim: &mut SimulatorView| {
    ///         // Write to LED peripheral at 0x50000000
    ///         let request = BusRequest::write(0x50000000, 0xAB, AccessSize::Byte);
    ///         sim.send_bus_request(request)
    ///             .expect("Should queue host request");
    ///     }),
    ///     None::<fn(&InstructionTrace)>,
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     |_sim| Ok(0x8000_0000),
    ///     None::<fn(&SimulatorView, &SimulationResult)>,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_bus_request(&mut self, request: BusRequest) -> Result<(), String> {
        // Validate address is in RTL peripheral space (0x50000000-0x5FFFFFFF)
        // This prevents deadlock per Rule 1 (no self-routing)
        if !(0x50000000..0x60000000).contains(&request.addr) {
            return Err(format!(
                "Invalid address 0x{:08x}: must be in RTL peripheral space (0x50000000-0x5FFFFFFF)",
                request.addr
            ));
        }

        // Send through the handler
        self.host_bus_handler
            .send_request(request)
            .map_err(|e| format!("Host request rejected: {:?}", e))
    }

    /// Receive a bus response from the RTL target
    ///
    /// Returns the response for the most recently completed host-initiated request.
    /// This should be called in a loop until it returns `Some` to wait for the
    /// response to be ready.
    ///
    /// # Returns
    /// * `Some(response)` - Response received (contains rdata for reads)
    /// * `None` - No response available yet
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::*;
    /// # fn main() -> Result<(), String> {
    /// run_program(
    ///     100,
    ///     false, // print_inst_trace
    ///     false, // print_fsm_state
    ///     Some(|sim: &mut SimulatorView| {
    ///         // Send host-initiated read request
    ///         let request = BusRequest::read(0x50000000, AccessSize::Byte);
    ///         sim.send_bus_request(request)
    ///             .expect("Should queue host request");
    ///
    ///         // Poll for response (will be available after a few cycles)
    ///         if let Some(response) = sim.receive_bus_response() {
    ///             println!("LED value: 0x{:02x}", response.rdata);
    ///         }
    ///     }),
    ///     None::<fn(&InstructionTrace)>,
    ///     None, // vcd_path
    ///     0, // mem_latency_cycles
    ///     |_sim| Ok(0x8000_0000),
    ///     None::<fn(&SimulatorView, &SimulationResult)>,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn receive_bus_response(&mut self) -> Option<BusResponse> {
        self.host_bus_handler.receive_response()
    }
}

/// Maximum number of clock cycles allowed during the boot process before timing out
const BOOT_TIMEOUT_CYCLES: u32 = 10_000;

/// RISC-V CPU Simulator
///
/// This structure owns its runtime internally using an unsafe self-referential pattern.
/// The CPU model borrows from the runtime with a 'static lifetime, which is safe because:
/// 1. The runtime is boxed (stable heap address)
/// 2. Field drop order ensures CPU drops before runtime (fields drop in declaration order)
pub struct Simulator<F, T>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    // CRITICAL: Fields must be in this order for safe drop semantics
    // 1. CPU (dependent) MUST be declared FIRST - drops first
    pub(crate) cpu: Top<'static>,
    vcd: Option<Vcd<'static>>,

    // 2. Runtime (owner) MUST be declared AFTER cpu - drops last
    // Box ensures stable heap address so moving Simulator doesn't invalidate cpu's reference
    _runtime: Box<VerilatorRuntime>,

    // Other fields can be in any order
    pub bus: SystemBus,
    cycle_count: u64,
    total_elapsed_time_us: u64, // Cumulative elapsed time in microseconds
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_time: u64, // VCD timestamp counter (incremented independently from cycle_count)
    // Memory latency simulation
    mem_latency_cycles: u32, // Number of cycles to delay memory operations
    // Host bus handler
    pub(crate) host_bus_handler: HostBusHandler,
    // Pending response for memory latency simulation
    pending_response: Option<PendingResponse>,
    // Hung state detection
    pub(crate) hung_detector: Option<HungDetector>,
}

impl<F, T> Simulator<F, T>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    /// Create a new simulator with optional callbacks
    ///
    /// The runtime, bus, and hung detector are created and owned internally using
    /// an unsafe self-referential pattern. This is safe because:
    /// 1. The runtime is boxed (stable heap address)
    /// 2. Field drop order ensures CPU drops before runtime
    ///
    /// # Arguments
    /// * `print_inst_trace` - Enable instruction trace printing
    /// * `print_fsm_state` - Enable FSM state printing
    /// * `inst_complete_callback` - Optional callback invoked after each instruction completes
    /// * `trace_callback` - Optional callback for instruction traces
    /// * `vcd_path` - Optional path to VCD file for waveform tracing
    /// * `mem_latency_cycles` - Number of cycles to delay memory operations
    /// * `verilator_optimization` - Verilator optimization level (0-3), higher values increase execution speed but slow compilation
    pub fn new(
        print_inst_trace: bool,
        print_fsm_state: bool,
        inst_complete_callback: Option<F>,
        trace_callback: Option<T>,
        vcd_path: Option<&str>,
        mem_latency_cycles: u32,
        verilator_optimization: usize,
    ) -> Result<Self, String> {
        // Create system bus with internal DRAM (always default)
        let bus = SystemBus::new();

        // Create hung detector config (always default)
        let hung_detector = Some(HungDetector::new(HungDetectorConfig::default()));

        // 1. Create and box the runtime immediately for stable heap address
        let runtime = Box::new(
            riscv_core::create_cpu_runtime()
                .map_err(|e| format!("Failed to create CPU runtime: {}", e))?,
        );

        // 2. Create CPU model using unsafe lifetime extension
        let (cpu, vcd) = unsafe {
            // Get a raw pointer to the runtime on the heap
            let runtime_ptr: *const VerilatorRuntime = &*runtime;

            // Create an unbounded ('static) reference
            // SAFETY: We guarantee the runtime will not be dropped while cpu exists
            // because _runtime is declared after cpu in the struct, so cpu drops first
            let runtime_ref: &'static VerilatorRuntime = &*runtime_ptr;

            // Create CPU model with configuration
            let config = VerilatedModelConfig {
                enable_tracing: vcd_path.is_some(),
                verilator_optimization,
                ..Default::default()
            };

            let mut cpu = runtime_ref
                .create_model::<Top>(&config)
                .map_err(|e| format!("Failed to create CPU model: {}", e))?;

            // Open VCD file if path is provided
            let vcd = if let Some(vcd_file_path) = vcd_path {
                let vcd = cpu.open_vcd(vcd_file_path);
                log::info!("VCD tracing enabled, writing to: {}", vcd_file_path);
                Some(vcd)
            } else {
                None
            };

            (cpu, vcd)
        };

        log::info!("Memory latency configured to {} cycles", mem_latency_cycles);

        // 3. Bundle everything together
        // CRITICAL: Field declaration order ensures safe drop - cpu drops before _runtime
        Ok(Simulator {
            cpu,
            vcd,
            _runtime: runtime,
            bus,
            cycle_count: 0,
            total_elapsed_time_us: 0,
            print_inst_trace,
            print_fsm_state,
            inst_complete_callback,
            trace_callback,
            vcd_time: 0,
            mem_latency_cycles,
            host_bus_handler: HostBusHandler::new(),
            pending_response: None,
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
    fn dump_vcd(&mut self) {
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(self.vcd_time);
            self.vcd_time += 1;
        }
    }

    /// Handle the host bus interface protocol using the HostBusHandler
    ///
    /// This method:
    /// 1. Transfers TX bytes from FPGA to handler when host_tx_valid=1
    /// 2. Transfers TX bytes from handler to FPGA when host_rx_ready=1
    /// 3. Accepts incoming requests and processes them with memory latency support
    fn handle_host_bus_interface(&mut self) {
        // Step 1: If host_tx_valid is 1, pass host_tx_data to handler via transfer_rx_byte()
        if self.cpu.host_tx_valid != 0 {
            let byte = self.cpu.host_tx_data;
            self.host_bus_handler.transfer_rx_byte(byte).expect(
                "Protocol violation: handler buffer full while FPGA is sending data. \
                 This indicates the FPGA sent more data than expected before the host \
                 could process it. Ensure incoming requests are accepted and completed \
                 before the FPGA sends the next request.",
            );
        }

        // Step 2: If host_rx_ready is 1, try to get a byte to send
        if self.cpu.host_rx_ready != 0 {
            if let Some(byte) = self.host_bus_handler.transfer_tx_byte() {
                self.cpu.host_rx_data = byte;
                self.cpu.host_rx_valid = 1;
            } else {
                self.cpu.host_rx_valid = 0;
            }
        } else {
            self.cpu.host_rx_valid = 0;
        }

        // Step 3: Handle pending response completion (memory latency support)
        // For host-initiated requests, we also need to check if the FPGA
        // has completed a bus transaction and send the response back
        if let Some(ref pending) = self.pending_response {
            // Check if latency delay has elapsed
            if self.cycle_count >= pending.accepted_cycle + self.mem_latency_cycles as u64 {
                // Latency complete - send the response
                self.host_bus_handler
                    .complete_request(pending.response.clone())
                    .expect("Failed to complete request");
                self.pending_response = None;
            }
        }

        // Step 4: Accept new incoming requests (only if no pending response waiting for latency)
        if self.pending_response.is_none() {
            if let Ok(request) = self.host_bus_handler.accept_request() {
                // Perform the bus operation and create the response immediately
                let response = if request.we {
                    // Write operation
                    self.perform_write(request.addr, request.wdata, request.size);
                    BusResponse::write_ack(request.size)
                } else {
                    // Read operation
                    let rdata = self.perform_read(request.addr, request.size);
                    BusResponse::read_data(rdata, request.size)
                };

                // Apply memory latency if configured
                if self.mem_latency_cycles > 0 {
                    // Store pending response to complete after latency
                    self.pending_response = Some(PendingResponse {
                        response,
                        accepted_cycle: self.cycle_count,
                    });
                } else {
                    // No latency - complete immediately
                    self.host_bus_handler
                        .complete_request(response)
                        .expect("Failed to complete request");
                }
            }
        }
    }

    /// Perform a read operation from the bus
    fn perform_read(&mut self, addr: u32, size: AccessSize) -> u32 {
        match size {
            AccessSize::Byte => self.bus.read_byte(addr) as u32,
            AccessSize::Halfword => self.bus.read_halfword(addr) as u32,
            AccessSize::Word => self.bus.read_word(addr),
        }
    }

    /// Perform a write operation to the bus
    fn perform_write(&mut self, addr: u32, wdata: u32, size: AccessSize) {
        match size {
            AccessSize::Byte => self.bus.write_byte(addr, wdata as u8),
            AccessSize::Halfword => self.bus.write_halfword(addr, wdata as u16),
            AccessSize::Word => self.bus.write_word(addr, wdata),
        }
    }

    /// Reset the CPU
    /// After hardware reset, the system controller peripheral handles CPU booting
    /// via host bus requests. The host reads STATUS to confirm the CPU is waiting
    /// for boot, then writes the boot address to the BOOT register.
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

        // Initialize host bus interface signals
        // host_tx_ready is always 1 because the handler can buffer requests/responses
        // and the FPGA side never sends more than one request at a time
        self.cpu.host_tx_ready = 1;
        self.cpu.host_rx_valid = 0;
        self.cpu.host_rx_data = 0;
        self.cpu.reset_request = 0;

        // Drive reset low
        self.cpu.rst_n = 0;
        self.cpu.clk = 0;
        self.cpu.eval();
        self.dump_vcd(); // Capture initial state with reset asserted, clk=0

        // First clock edge during reset
        self.cpu.clk = 1;
        self.cpu.eval();
        self.dump_vcd(); // Capture state after rising edge during reset

        // Second clock cycle during reset (falling edge)
        self.cpu.clk = 0;
        self.cpu.eval();
        self.dump_vcd(); // Capture state after falling edge during reset

        // Release reset (still at clk=0)
        self.cpu.rst_n = 1;
        self.cpu.eval();
        self.dump_vcd(); // Capture state with reset released

        // Reset the hung detector state
        if let Some(ref mut detector) = self.hung_detector {
            detector.reset();
        }

        // Reset all bus devices
        self.bus.reset_all_devices();

        // Reset cumulative elapsed time
        self.total_elapsed_time_us = 0;

        // Reset the host bus handler
        self.host_bus_handler.reset();
        self.pending_response = None;

        // Boot the CPU via host bus requests to the system controller peripheral
        // First, run clock cycles until the internal reset controller has completed
        // (rst_n_out goes high). The reset controller holds internal reset for
        // RESET_CYCLES (default 8) after external rst_n goes high.
        loop {
            self.boot_clock_cycle();
            if self.cpu.rst_n_out != 0 {
                break;
            }
        }

        // Step 1: Read STATUS register to confirm CPU is waiting to be booted
        let status_addr = riscv_shared::bus::sysctrl_status_addr();
        let status_request = BusRequest::read(status_addr, AccessSize::Word);
        self.host_bus_handler
            .send_request(status_request)
            .expect("Failed to send STATUS read request");

        // Spin until we receive the STATUS response
        let mut boot_cycles = 0u32;
        loop {
            self.boot_clock_cycle();
            boot_cycles += 1;
            if boot_cycles > BOOT_TIMEOUT_CYCLES {
                panic!(
                    "Boot timed out waiting for STATUS response after {} cycles",
                    boot_cycles
                );
            }
            if let Some(response) = self.host_bus_handler.receive_response() {
                // Check that cpu_booting bit (bit 0) is set
                assert!(
                    (response.rdata & riscv_shared::bus::SYSCTRL_STATUS_CPU_BOOTING) != 0,
                    "CPU is not in boot state after reset - hardware issue (STATUS=0x{:08x})",
                    response.rdata
                );
                break;
            }
        }

        // Step 2: Write boot address to BOOT register to complete boot process
        let boot_addr = riscv_shared::bus::sysctrl_boot_addr();
        let boot_request = BusRequest::write(boot_addr, boot_pc, AccessSize::Word);
        self.host_bus_handler
            .send_request(boot_request)
            .expect("Failed to send BOOT write request");

        // Spin until we receive the write acknowledgement
        loop {
            self.boot_clock_cycle();
            if let Some(_response) = self.host_bus_handler.receive_response() {
                // Write acknowledged - CPU boot process is now complete
                break;
            }
        }

        log::info!("CPU reset complete with boot PC: 0x{:08x}", boot_pc);
        Ok(())
    }

    /// Execute a single clock cycle during the boot process
    ///
    /// This is a simplified version of execute_clock_cycle() that only handles
    /// the host bus interface protocol and clock edge. It does NOT:
    /// - Increment cycle_count
    /// - Call bus.clock_cycle_all_devices()
    /// - Run hung detector checks
    /// - Print FSM state
    fn boot_clock_cycle(&mut self) {
        self.cpu.eval();
        self.handle_host_bus_interface();
        self.cpu.eval();
        self.cpu.clk = 0;
        self.cpu.eval();
        self.cpu.clk = 1;
        self.cpu.eval();
        self.dump_vcd();
    }

    /// Get the current LED output value
    ///
    /// Returns the 8-bit LED output value from the LED controller peripheral.
    #[allow(dead_code)]
    pub fn led_out(&self) -> u8 {
        self.cpu.led_out
    }

    /// Execute a single clock cycle of the simulation
    ///
    /// This handles evaluation, host bus interface protocol, FSM state printing,
    /// clock edge, VCD dumping, and bus device updates.
    ///
    /// # Returns
    /// * `true` if instruction completed this cycle, `false` otherwise
    ///
    /// # Errors
    /// Returns `HungStateError` if the CPU is detected to be in a hung state
    fn execute_clock_cycle(&mut self) -> Result<bool, HungStateError> {
        // Evaluate combinational logic
        self.cpu.eval();

        // Handle host bus interface protocol
        // The CPU sends serialized bus transactions via host_tx_* signals
        // and we respond via host_rx_* signals
        self.handle_host_bus_interface();

        // Re-evaluate after setting memory signals
        self.cpu.eval();

        // Print FSM state if enabled (before clock edge)
        if self.print_fsm_state {
            let fsm_state = self.cpu.debug_fsm_state;
            let state_name = Self::fsm_state_name(fsm_state);
            println!(
                "Cycle {:6} | State: {:10} | PC: 0x{:08x} | host_tx_valid={} host_rx_ready={} | instr_complete={}",
                self.cycle_count,
                state_name,
                self.cpu.debug_current_pc,
                self.cpu.host_tx_valid,
                self.cpu.host_rx_ready,
                self.cpu.instr_complete
            );
        }

        // Clock edge
        self.cpu.clk = 0;
        self.cpu.eval();
        self.cpu.clk = 1;
        self.cpu.eval();

        // Increment cycle count
        self.cycle_count += 1;

        // Dump VCD if enabled (after clock edge)
        self.dump_vcd();

        // Call clock_cycle on all bus devices (after clock edge completes)
        self.bus.clock_cycle_all_devices();

        // Check if instruction complete (AFTER clock edge)
        // With delayed instr_complete, values have already settled by the time we see the signal
        let instruction_complete = self.cpu.instr_complete != 0;

        // Check for hung state on every cycle
        // This detects stuck FSM, invalid PC, and PC loops (when instruction completes)
        if let Some(ref mut detector) = self.hung_detector {
            // Use current PC and instruction for hung detection (not completed ones)
            // debug_current_pc: PC that was used to fetch the current instruction
            // debug_current_instruction: The instruction currently being executed
            let pc = self.cpu.debug_current_pc;
            let instruction = self.cpu.debug_current_instruction;
            let fsm_state = self.cpu.debug_fsm_state;
            detector.check_cycle(
                self.cycle_count,
                pc,
                instruction,
                fsm_state,
                instruction_complete,
            )?;
        }

        Ok(instruction_complete)
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
            let instruction_complete = self.execute_clock_cycle()?;

            if instruction_complete {
                break;
            }
        }

        // Call inst_complete callback if provided (after instruction completion)
        // This callback receives restricted access to the Simulator via SimulatorView
        if let Some(ref mut callback) = self.inst_complete_callback {
            let mut view = SimulatorView::new(
                &mut self.bus,
                &mut self.hung_detector,
                &self.cpu,
                &mut self.host_bus_handler,
            );
            callback(&mut view);
        }

        // Unified instruction trace handling
        // Check if trace callback is valid or instruction trace printing is enabled
        if self.trace_callback.is_some() || self.print_inst_trace {
            // Assemble InstructionTrace structure using debug signals from CPU
            let pc = self.cpu.debug_pc;
            let instruction = self.cpu.debug_instruction;
            let rs1_value = self.cpu.debug_rs1_data;
            let rs2_value = self.cpu.debug_rs2_data;
            let rd_value = self.cpu.debug_rd_data;

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

        // Accumulate elapsed time
        self.total_elapsed_time_us = self.total_elapsed_time_us.saturating_add(elapsed_us);

        // Update bus with cumulative elapsed time for devices
        // This ensures Video and other time-sensitive devices get accurate cumulative time
        self.bus.update_elapsed_time(self.total_elapsed_time_us);

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
                log::debug!(
                    "Cycle {}: PC=0x{:08x}",
                    self.cycle_count,
                    self.cpu.debug_current_pc
                );
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

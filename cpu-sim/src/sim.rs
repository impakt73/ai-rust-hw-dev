use crate::bus::SystemBus;
use crate::hung_detector::{HungDetector, HungDetectorConfig, HungStateError};
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, Vcd, VerilatedModelConfig, VerilatorRuntime};
use std::collections::VecDeque;
use std::path::Path;
use std::time::Instant;

/// DRAM memory range: DRAM_BASE to DRAM_END (inclusive)
use crate::bus::{is_valid_dram_range, DRAM_BASE, DRAM_END};

/// RTL peripheral address range (host can only access this range)
const RTL_PERIPH_BASE: u32 = 0x5000_0000;
const RTL_PERIPH_LIMIT: u32 = 0x6000_0000;

/// Packet types for the extended header format
const PACKET_TYPE_CPU_REQUEST: u8 = 0x00; // CPU→Host request
const PACKET_TYPE_HOST_RESPONSE: u8 = 0x10; // Host→CPU response
const PACKET_TYPE_HOST_REQUEST: u8 = 0x20; // Host→FPGA request
const PACKET_TYPE_FPGA_RESPONSE: u8 = 0x30; // FPGA→Host response
const PACKET_TYPE_ERROR: u8 = 0xF0; // Error response

/// Host Bus Interface packet processing state (CPU→Host direction)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum HostBusState {
    /// Idle - waiting for TX packet from CPU, will consume header when valid
    Idle,
    /// Receiving address bytes (4 bytes, little-endian)
    RxAddr { byte_idx: u8 },
    /// Receiving write data bytes (1-4 bytes based on size)
    RxWdata { byte_idx: u8 },
    /// Sending response header byte (packet type 0001)
    TxHeader,
    /// Sending write acknowledgement (after header) - kept for compatibility
    TxAck,
    /// Sending read data bytes (1-4 bytes based on size)
    TxRdata { byte_idx: u8 },
}

/// Host-side state machine for host-initiated requests (Host→FPGA direction)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusHostState {
    /// Idle - ready to send or receive
    Idle,
    /// Sending host request header (packet type 0010)
    TxHeader,
    /// Sending address bytes
    TxAddr { byte_idx: u8 },
    /// Sending write data bytes
    TxWdata { byte_idx: u8 },
    /// Waiting for response header
    RxWaitingHeader,
    /// Waiting for response ack (write completed)
    RxAck,
    /// Receiving read data bytes
    RxRdata { byte_idx: u8 },
}

/// Captured transaction from host bus interface (CPU→Host)
#[derive(Debug, Clone, Default)]
struct HostBusTransaction {
    /// Write enable (true = write, false = read)
    we: bool,
    /// Access size (0 = byte, 1 = halfword, 2 = word)
    size: u8,
    /// Address (accumulated little-endian)
    addr: u32,
    /// Write data (accumulated little-endian, only valid for writes)
    wdata: u32,
    /// Read data to send back (only valid for reads)
    rdata: u32,
}

/// Host-initiated bus request
#[derive(Debug, Clone)]
pub struct HostBusRequest {
    /// Address to access (must be in RTL peripheral range 0x5000_0000 - 0x5FFF_FFFF)
    pub addr: u32,
    /// Write data (for writes, ignored for reads)
    pub wdata: u32,
    /// Access size (0=byte, 1=half, 2=word)
    pub size: u8,
    /// Write enable (true = write, false = read)
    pub we: bool,
}

impl HostBusRequest {
    /// Create a byte read request
    pub fn read_byte(addr: u32) -> Self {
        HostBusRequest {
            addr,
            wdata: 0,
            size: 0,
            we: false,
        }
    }

    /// Create a byte write request
    pub fn write_byte(addr: u32, data: u8) -> Self {
        HostBusRequest {
            addr,
            wdata: data as u32,
            size: 0,
            we: true,
        }
    }

    /// Create a halfword read request
    pub fn read_halfword(addr: u32) -> Self {
        HostBusRequest {
            addr,
            wdata: 0,
            size: 1,
            we: false,
        }
    }

    /// Create a halfword write request
    pub fn write_halfword(addr: u32, data: u16) -> Self {
        HostBusRequest {
            addr,
            wdata: data as u32,
            size: 1,
            we: true,
        }
    }

    /// Create a word read request
    pub fn read_word(addr: u32) -> Self {
        HostBusRequest {
            addr,
            wdata: 0,
            size: 2,
            we: false,
        }
    }

    /// Create a word write request
    pub fn write_word(addr: u32, data: u32) -> Self {
        HostBusRequest {
            addr,
            wdata: data,
            size: 2,
            we: true,
        }
    }
}

/// Response to a host-initiated bus request
#[derive(Debug, Clone)]
pub enum HostBusResponse {
    /// Successful read with data
    ReadData(u32),
    /// Successful write acknowledgement
    WriteAck,
    /// Error response
    Error(FpgaError),
}

/// Error codes from FPGA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaError {
    /// Request targeted invalid address (would route back to host)
    InvalidAddress,
    /// Request timed out
    Timeout,
    /// Protocol error
    ProtocolError,
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
    /// Queue for host-initiated bus requests
    host_request_queue: &'a mut VecDeque<HostBusRequest>,
    /// Queue for host bus responses  
    host_response_queue: &'a mut VecDeque<HostBusResponse>,
}

impl<'a> SimulatorView<'a> {
    /// Create a new SimulatorView with access to the given components
    pub(crate) fn new(
        bus: &'a mut crate::bus::SystemBus,
        hung_detector: &'a mut Option<HungDetector>,
        cpu: &'a Top<'static>,
        host_request_queue: &'a mut VecDeque<HostBusRequest>,
        host_response_queue: &'a mut VecDeque<HostBusResponse>,
    ) -> Self {
        SimulatorView {
            bus,
            hung_detector,
            cpu,
            host_request_queue,
            host_response_queue,
        }
    }

    /// Queue a host-initiated bus request
    ///
    /// The request will be sent to the FPGA on subsequent simulation steps.
    /// Use receive_bus_response() to get the result.
    ///
    /// # Arguments
    /// * `request` - The bus request to send
    ///
    /// # Errors
    /// * Returns error if address is outside RTL peripheral range (0x5000_0000 - 0x5FFF_FFFF)
    pub fn send_bus_request(&mut self, request: HostBusRequest) -> Result<(), String> {
        // Validate address range
        if request.addr < RTL_PERIPH_BASE || request.addr >= RTL_PERIPH_LIMIT {
            return Err(format!(
                "Host-initiated request address 0x{:08x} is outside RTL peripheral range \
                 (0x{:08x} - 0x{:08x}). Host cannot access addresses that route back to host.",
                request.addr, RTL_PERIPH_BASE, RTL_PERIPH_LIMIT
            ));
        }

        // Queue the request
        self.host_request_queue.push_back(request);
        Ok(())
    }

    /// Receive response to a previously sent host-initiated bus request
    ///
    /// # Returns
    /// * `Some(response)` - Response received
    /// * `None` - No response available yet
    pub fn receive_bus_response(&mut self) -> Option<HostBusResponse> {
        self.host_response_queue.pop_front()
    }

    /// Check if there are pending host bus responses
    pub fn has_bus_response(&self) -> bool {
        !self.host_response_queue.is_empty()
    }

    /// Check if there are pending host bus requests in the queue
    pub fn has_pending_requests(&self) -> bool {
        !self.host_request_queue.is_empty()
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

    /// Write a 32-bit word to memory (little-endian)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds writes are logged as warnings and ignored.
    pub fn write_word(&mut self, addr: u32, value: u32) {
        if !is_valid_dram_range(addr, 4) {
            log::warn!(
                "write_word: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), ignoring write",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return;
        }
        self.bus.memory.write_word(addr, value);
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
}

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
    // Host bus interface state machine (CPU→Host direction)
    host_bus_state: HostBusState,
    host_bus_txn: HostBusTransaction,
    host_bus_delay_counter: u32, // Delay counter for memory latency simulation
    // Host-initiated request state machine (Host→FPGA direction)
    host_bus_host_state: HostBusHostState,
    current_host_request: Option<HostBusRequest>,
    pub(crate) host_request_queue: VecDeque<HostBusRequest>,
    pub(crate) host_response_queue: VecDeque<HostBusResponse>,
    host_resp_rdata: u32, // Accumulated read data from FPGA response
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
            host_bus_state: HostBusState::Idle,
            host_bus_txn: HostBusTransaction::default(),
            host_bus_delay_counter: 0,
            host_bus_host_state: HostBusHostState::Idle,
            current_host_request: None,
            host_request_queue: VecDeque::new(),
            host_response_queue: VecDeque::new(),
            host_resp_rdata: 0,
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

    /// Handle the host bus interface protocol (bidirectional)
    ///
    /// This method implements the host side of the serialized bus protocol.
    /// It handles both:
    /// 1. CPU→Host: Receives TX packets from the CPU (read/write requests) and sends
    ///    RX responses (acknowledgements or read data).
    /// 2. Host→FPGA: Sends host-initiated requests to the FPGA and receives responses.
    ///
    /// Extended Protocol (Variable Length, Little-Endian):
    ///   CPU Request:    [ext_header][addr0..3][data...]     (packet type 0000)
    ///   Host Response:  [ext_header][data...]               (packet type 0001)
    ///   Host Request:   [ext_header][addr0..3][data...]     (packet type 0010)
    ///   FPGA Response:  [ext_header][data...]               (packet type 0011)
    ///   Error Response: [ext_header][error_code]            (packet type 1111)
    ///
    /// Extended header format: {packet_type[3:0], size[1:0], 1'b0, we}
    fn handle_host_bus_interface(&mut self) {
        // Determine who controls the RX (host→FPGA) interface
        // Priority: Host→FPGA requests take precedence when sending
        let host_is_sending = matches!(
            self.host_bus_host_state,
            HostBusHostState::TxHeader
                | HostBusHostState::TxAddr { .. }
                | HostBusHostState::TxWdata { .. }
        );

        let cpu_is_responding = matches!(
            self.host_bus_state,
            HostBusState::TxHeader | HostBusState::TxAck | HostBusState::TxRdata { .. }
        );

        // Handle CPU→Host direction (receiving CPU requests, sending responses)
        self.handle_cpu_to_host(cpu_is_responding, host_is_sending);

        // Handle Host→FPGA direction (sending host requests, receiving responses)
        self.handle_host_to_fpga(host_is_sending);
    }

    /// Handle CPU→Host direction: receive CPU requests and send responses
    fn handle_cpu_to_host(&mut self, cpu_is_responding: bool, host_is_sending: bool) {
        // Always ready to receive from CPU (TX side)
        self.cpu.host_tx_ready = 1;

        match self.host_bus_state {
            HostBusState::Idle => {
                // Only set rx_valid=0 if neither side is actively sending
                if !host_is_sending && !cpu_is_responding {
                    self.cpu.host_rx_valid = 0;
                }

                if self.cpu.host_tx_valid != 0 {
                    // Handshake complete (valid && ready) - consume header byte now
                    let header = self.cpu.host_tx_data;
                    let packet_type = header & 0xF0;

                    if packet_type == PACKET_TYPE_CPU_REQUEST {
                        // CPU-initiated request (packet type 0000)
                        self.host_bus_txn.we = (header & 0x01) != 0;
                        self.host_bus_txn.size = (header >> 2) & 0x03;
                        self.host_bus_txn.addr = 0;
                        self.host_bus_txn.wdata = 0;
                        self.host_bus_state = HostBusState::RxAddr { byte_idx: 0 };
                    } else if packet_type == PACKET_TYPE_FPGA_RESPONSE {
                        // FPGA response to our host-initiated request (packet type 0011)
                        // Parse the header
                        let we = (header & 0x01) != 0;
                        let size = (header >> 2) & 0x03;

                        if we {
                            // Write ack - response is just the header
                            self.host_response_queue
                                .push_back(HostBusResponse::WriteAck);
                            self.host_bus_host_state = HostBusHostState::Idle;
                            self.current_host_request = None;
                        } else {
                            // Read response - need to receive data bytes
                            self.host_resp_rdata = 0;
                            if self.current_host_request.is_some() {
                                self.host_bus_host_state =
                                    HostBusHostState::RxRdata { byte_idx: 0 };
                            } else {
                                // Unexpected response
                                log::warn!(
                                    "Received FPGA response but no pending request, size={}",
                                    size
                                );
                            }
                        }
                    } else if packet_type == PACKET_TYPE_ERROR {
                        // Error response (packet type 1111)
                        // Need to receive the error code byte
                        self.host_bus_host_state = HostBusHostState::RxAck; // Reuse for error code
                    }
                }
            }

            HostBusState::RxAddr { byte_idx } => {
                // host_tx_ready is already set at the start of this function

                if self.cpu.host_tx_valid != 0 {
                    let byte = self.cpu.host_tx_data as u32;
                    // Accumulate address (little-endian)
                    self.host_bus_txn.addr |= byte << (byte_idx * 8);

                    if byte_idx == 3 {
                        // Address complete
                        if self.host_bus_txn.we {
                            // Write: continue receiving write data
                            self.host_bus_state = HostBusState::RxWdata { byte_idx: 0 };
                        } else {
                            // Read: perform read and start sending response
                            self.perform_read();
                            // Apply memory latency
                            if self.mem_latency_cycles > 0 {
                                self.host_bus_delay_counter = self.mem_latency_cycles;
                            }
                            self.host_bus_state = HostBusState::TxHeader;
                        }
                    } else {
                        self.host_bus_state = HostBusState::RxAddr {
                            byte_idx: byte_idx + 1,
                        };
                    }
                }
            }

            HostBusState::RxWdata { byte_idx } => {
                // host_tx_ready is already set at the start of this function

                if self.cpu.host_tx_valid != 0 {
                    let byte = self.cpu.host_tx_data as u32;
                    // Accumulate write data (little-endian)
                    self.host_bus_txn.wdata |= byte << (byte_idx * 8);

                    // Determine how many bytes we need based on size
                    let bytes_needed = match self.host_bus_txn.size {
                        0 => 1, // byte
                        1 => 2, // halfword
                        _ => 4, // word
                    };

                    if byte_idx + 1 >= bytes_needed {
                        // Write data complete - perform write and send ack
                        self.perform_write();
                        // Apply memory latency
                        if self.mem_latency_cycles > 0 {
                            self.host_bus_delay_counter = self.mem_latency_cycles;
                        }
                        self.host_bus_state = HostBusState::TxHeader;
                    } else {
                        self.host_bus_state = HostBusState::RxWdata {
                            byte_idx: byte_idx + 1,
                        };
                    }
                }
            }

            HostBusState::TxHeader => {
                // Apply memory latency delay before sending response header
                if self.host_bus_delay_counter > 0 {
                    self.host_bus_delay_counter -= 1;
                    self.cpu.host_tx_ready = 0;
                    self.cpu.host_rx_valid = 0;
                    return;
                }

                self.cpu.host_tx_ready = 0; // Not receiving during TX phase
                self.cpu.host_rx_valid = 1;
                // Response header: packet type 0001 (host response)
                let header = PACKET_TYPE_HOST_RESPONSE
                    | ((self.host_bus_txn.size & 0x03) << 2)
                    | if self.host_bus_txn.we { 0x01 } else { 0x00 };
                self.cpu.host_rx_data = header;

                if self.cpu.host_rx_ready != 0 {
                    // Header accepted
                    if self.host_bus_txn.we {
                        // Write: header is the ack, we're done
                        self.host_bus_state = HostBusState::Idle;
                    } else {
                        // Read: send data bytes
                        self.host_bus_state = HostBusState::TxRdata { byte_idx: 0 };
                    }
                }
            }

            HostBusState::TxAck => {
                // Legacy state - kept for compatibility but now we use TxHeader
                self.cpu.host_tx_ready = 0;
                self.cpu.host_rx_valid = 1;
                // Response header: packet type 0001 + write ack
                let header =
                    PACKET_TYPE_HOST_RESPONSE | ((self.host_bus_txn.size & 0x03) << 2) | 0x01; // we=1
                self.cpu.host_rx_data = header;

                if self.cpu.host_rx_ready != 0 {
                    self.host_bus_state = HostBusState::Idle;
                }
            }

            HostBusState::TxRdata { byte_idx } => {
                self.cpu.host_tx_ready = 0; // Not receiving during TX phase
                self.cpu.host_rx_valid = 1;

                // Send read data byte (little-endian)
                let byte = ((self.host_bus_txn.rdata >> (byte_idx * 8)) & 0xFF) as u8;
                self.cpu.host_rx_data = byte;

                if self.cpu.host_rx_ready != 0 {
                    // Determine how many bytes we need to send based on size
                    let bytes_needed = match self.host_bus_txn.size {
                        0 => 1, // byte
                        1 => 2, // halfword
                        _ => 4, // word
                    };

                    if byte_idx + 1 >= bytes_needed {
                        // All bytes sent, return to idle
                        self.host_bus_state = HostBusState::Idle;
                    } else {
                        self.host_bus_state = HostBusState::TxRdata {
                            byte_idx: byte_idx + 1,
                        };
                    }
                }
            }
        }
    }

    /// Handle Host→FPGA direction: send host-initiated requests and receive responses
    fn handle_host_to_fpga(&mut self, _host_is_sending: bool) {
        match self.host_bus_host_state {
            HostBusHostState::Idle => {
                // Check if we have a queued request to send
                if let Some(request) = self.host_request_queue.pop_front() {
                    // Start sending the request - we can interleave with CPU processing
                    // Host-initiated requests take priority on the RX line
                    self.current_host_request = Some(request);
                    self.host_bus_host_state = HostBusHostState::TxHeader;
                }
            }

            HostBusHostState::TxHeader => {
                if let Some(ref request) = self.current_host_request {
                    // Send host request header (packet type 0010)
                    let header = PACKET_TYPE_HOST_REQUEST
                        | ((request.size & 0x03) << 2)
                        | if request.we { 0x01 } else { 0x00 };

                    self.cpu.host_rx_valid = 1;
                    self.cpu.host_rx_data = header;

                    if self.cpu.host_rx_ready != 0 {
                        self.host_bus_host_state = HostBusHostState::TxAddr { byte_idx: 0 };
                    }
                }
            }

            HostBusHostState::TxAddr { byte_idx } => {
                if let Some(ref request) = self.current_host_request {
                    // Send address byte (little-endian)
                    let byte = ((request.addr >> (byte_idx * 8)) & 0xFF) as u8;

                    self.cpu.host_rx_valid = 1;
                    self.cpu.host_rx_data = byte;

                    if self.cpu.host_rx_ready != 0 {
                        if byte_idx == 3 {
                            // Address complete
                            if request.we {
                                // Write: send data bytes
                                self.host_bus_host_state =
                                    HostBusHostState::TxWdata { byte_idx: 0 };
                            } else {
                                // Read: wait for response
                                self.host_bus_host_state = HostBusHostState::RxWaitingHeader;
                            }
                        } else {
                            self.host_bus_host_state = HostBusHostState::TxAddr {
                                byte_idx: byte_idx + 1,
                            };
                        }
                    }
                }
            }

            HostBusHostState::TxWdata { byte_idx } => {
                if let Some(ref request) = self.current_host_request {
                    // Send write data byte (little-endian)
                    let byte = ((request.wdata >> (byte_idx * 8)) & 0xFF) as u8;

                    self.cpu.host_rx_valid = 1;
                    self.cpu.host_rx_data = byte;

                    if self.cpu.host_rx_ready != 0 {
                        let bytes_needed = match request.size {
                            0 => 1, // byte
                            1 => 2, // halfword
                            _ => 4, // word
                        };

                        if byte_idx + 1 >= bytes_needed {
                            // Write data complete, wait for response
                            self.host_bus_host_state = HostBusHostState::RxWaitingHeader;
                        } else {
                            self.host_bus_host_state = HostBusHostState::TxWdata {
                                byte_idx: byte_idx + 1,
                            };
                        }
                    }
                }
            }

            HostBusHostState::RxWaitingHeader => {
                // Waiting for FPGA response header
                // The response will be received through host_tx (FPGA's TX is our RX)
                self.cpu.host_rx_valid = 0; // Not sending
                                            // host_tx_ready is already set in handle_cpu_to_host

                // Response is handled in handle_cpu_to_host when packet type 0011 is received
            }

            HostBusHostState::RxAck => {
                // Waiting for error code byte (used when we got an error response)
                self.cpu.host_rx_valid = 0;

                if self.cpu.host_tx_valid != 0 && self.cpu.host_tx_ready != 0 {
                    let error_code = self.cpu.host_tx_data;
                    let error = match error_code {
                        0xFF => FpgaError::InvalidAddress,
                        0xFE => FpgaError::Timeout,
                        _ => FpgaError::ProtocolError,
                    };
                    self.host_response_queue
                        .push_back(HostBusResponse::Error(error));
                    self.host_bus_host_state = HostBusHostState::Idle;
                    self.current_host_request = None;
                }
            }

            HostBusHostState::RxRdata { byte_idx } => {
                // Receiving read data bytes from FPGA response
                self.cpu.host_rx_valid = 0;

                if self.cpu.host_tx_valid != 0 && self.cpu.host_tx_ready != 0 {
                    let byte = self.cpu.host_tx_data as u32;
                    self.host_resp_rdata |= byte << (byte_idx * 8);

                    let bytes_needed = if let Some(ref request) = self.current_host_request {
                        match request.size {
                            0 => 1, // byte
                            1 => 2, // halfword
                            _ => 4, // word
                        }
                    } else {
                        4 // Default to word if no request (shouldn't happen)
                    };

                    if byte_idx + 1 >= bytes_needed {
                        // All bytes received
                        self.host_response_queue
                            .push_back(HostBusResponse::ReadData(self.host_resp_rdata));
                        self.host_bus_host_state = HostBusHostState::Idle;
                        self.current_host_request = None;
                        self.host_resp_rdata = 0;
                    } else {
                        self.host_bus_host_state = HostBusHostState::RxRdata {
                            byte_idx: byte_idx + 1,
                        };
                    }
                }
            }
        }
    }

    /// Perform a read operation from the bus
    fn perform_read(&mut self) {
        let addr = self.host_bus_txn.addr;
        let rdata = match self.host_bus_txn.size {
            0 => self.bus.read_byte(addr) as u32,
            1 => self.bus.read_halfword(addr) as u32,
            _ => self.bus.read_word(addr),
        };
        self.host_bus_txn.rdata = rdata;
    }

    /// Perform a write operation to the bus
    fn perform_write(&mut self) {
        let addr = self.host_bus_txn.addr;
        let wdata = self.host_bus_txn.wdata;
        match self.host_bus_txn.size {
            0 => self.bus.write_byte(addr, wdata as u8),
            1 => self.bus.write_halfword(addr, wdata as u16),
            _ => self.bus.write_word(addr, wdata),
        }
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

        // Set the boot address BEFORE asserting and during reset
        // This is critical because the PC register uses an asynchronous reset that
        // loads boot_addr whenever rst_n is low; boot_addr must be stable while
        // reset is asserted so the PC will hold this value after reset is released.
        self.cpu.boot_addr = boot_pc;

        // Initialize host bus interface signals
        self.cpu.host_tx_ready = 0;
        self.cpu.host_rx_valid = 0;
        self.cpu.host_rx_data = 0;

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

        // Reset host bus interface state
        self.host_bus_state = HostBusState::Idle;
        self.host_bus_txn = HostBusTransaction::default();
        self.host_bus_delay_counter = 0;

        // Reset host-initiated request state
        self.host_bus_host_state = HostBusHostState::Idle;
        self.current_host_request = None;
        self.host_request_queue.clear();
        self.host_response_queue.clear();
        self.host_resp_rdata = 0;

        log::info!("CPU reset complete with boot PC: 0x{:08x}", boot_pc);
        Ok(())
    }

    /// Get the current LED output value
    ///
    /// Returns the 8-bit LED output value from the LED controller peripheral.
    #[allow(dead_code)]
    pub fn led_out(&self) -> u8 {
        self.cpu.led_out
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
                &mut self.host_request_queue,
                &mut self.host_response_queue,
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

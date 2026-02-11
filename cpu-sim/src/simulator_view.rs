use crate::hung_detector::HungDetector;
use device_runtime::{is_valid_dram_range, BusDevice, SystemBus};
use host_bus_handler::{BusRequest, BusResponse, HostBusHandler};
use riscv_core::Top;
use std::path::Path;

/// DRAM memory range: DRAM_BASE to DRAM_END (inclusive)
use device_runtime::{DRAM_BASE, DRAM_END};

/// Restricted view of the Simulator for use in callbacks
///
/// Provides controlled access to FIFO and memory operations without exposing
/// the full Simulator internals. This allows callbacks to interact with memory,
/// FIFO, and other simulator components while maintaining encapsulation.
pub struct SimulatorView<'a> {
    bus: &'a mut SystemBus,
    hung_detector: &'a mut Option<HungDetector>,
    cpu: &'a Top<'static>,
    host_bus_handler: &'a mut HostBusHandler,
}

impl<'a> SimulatorView<'a> {
    /// Create a new SimulatorView with access to the given components
    pub(crate) fn new(
        bus: &'a mut SystemBus,
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
    /// use cpu_sim::{run_program, InstructionTrace, SimulationResult, SimulatorView};
    /// use device_runtime::{BusDevice, BusDeviceError, SystemContext};
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
        device: Box<dyn BusDevice>,
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
        // Validate address is in RTL peripheral space using the SystemBus mapping
        // This prevents deadlock per Rule 1 (no self-routing)
        if !self.bus.is_rtl_peripheral(request.addr) {
            return Err(format!(
                "Invalid address 0x{:08x}: must be in RTL peripheral space",
                request.addr,
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

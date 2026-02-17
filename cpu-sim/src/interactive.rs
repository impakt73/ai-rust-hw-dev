use crate::sim::Simulator;
use crate::{
    load_elf, BusRequest, BusResponse, InstructionTrace, SimulationStepCycleResult,
    SimulationStepInstructionResult, SimulatorView,
};
use std::path::Path;

// Type alias for InteractiveSimulator's internal simulator type
type InteractiveSimulatorType = Simulator<fn(&mut SimulatorView), fn(&InstructionTrace)>;

/// Interactive wrapper around the Simulator for step-by-step execution
///
/// This structure provides a controlled interface for interactive use of the simulator,
/// allowing users to load ELF files and step through execution instruction-by-instruction.
/// Unlike the `run_elf` and `run_program` functions which run to completion,
/// `InteractiveSimulator` gives you fine-grained control over execution.
///
/// # Examples
/// ```no_run
/// use cpu_sim::InteractiveSimulator;
/// use std::path::Path;
///
/// let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
/// sim.load_elf(Path::new("program.elf")).expect("Failed to load ELF");
///
/// // Step through instructions one at a time
/// loop {
///     match sim.step_instruction() {
///         Ok(result) => {
///             if let Some(tohost) = result.tohost_value {
///                 println!("Program terminated with tohost: 0x{:08x}", tohost);
///                 break;
///             }
///         }
///         Err(e) => {
///             eprintln!("Error: {}", e);
///             break;
///         }
///     }
/// }
/// ```
pub struct InteractiveSimulator {
    /// Internal simulator instance with no callbacks
    simulator: InteractiveSimulatorType,
}

// SAFETY: InteractiveSimulator contains Verilator-generated C++ code which uses raw pointers
// that are not inherently Send. However, we can safely mark it as Send because:
// 1. The Verilator model is accessed from only one thread at a time
// 2. There are no shared mutable references across threads
// 3. The model is owned exclusively by this struct
// 4. When moved to another thread, all access happens on that single thread
//
// This is safe as long as the simulator is not accessed concurrently from multiple threads,
// which is enforced by Rust's ownership system.
unsafe impl Send for InteractiveSimulator {}

impl InteractiveSimulator {
    /// Create a new InteractiveSimulator with default configuration
    ///
    /// All optional parameters are set to None or disabled:
    /// - No instruction tracing
    /// - No FSM state printing
    /// - No callbacks
    /// - No VCD output
    /// - Zero memory latency
    /// - Verilator optimization level 3 (for interactive performance)
    ///
    /// # Returns
    /// A new `InteractiveSimulator` instance ready to load an ELF file
    ///
    /// # Errors
    /// Returns an error if the simulator fails to initialize (e.g., Verilator not available)
    pub fn new() -> Result<Self, String> {
        let simulator = Simulator::new(
            false, // print_inst_trace
            false, // print_fsm_state
            None,  // inst_complete_callback
            None,  // trace_callback
            None,  // vcd_path
            0,     // mem_latency_cycles
            3,     // verilator_optimization (level 3 for interactive performance)
        )?;

        Ok(InteractiveSimulator { simulator })
    }

    /// Load an ELF file into the simulator and reset to the entry point
    ///
    /// This function loads the ELF file into simulator memory, extracts the entry point,
    /// and resets the CPU to prepare for execution. After calling this function,
    /// you can use `step_instruction()` to execute the program.
    ///
    /// # Arguments
    /// * `path` - Path to the RISC-V ELF executable file
    ///
    /// # Returns
    /// * `Ok(entry_point)` with the ELF entry point address on success
    /// * `Err(String)` if the ELF file cannot be loaded or is invalid
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::InteractiveSimulator;
    /// # use std::path::Path;
    /// let mut sim = InteractiveSimulator::new().unwrap();
    /// let entry = sim.load_elf(Path::new("test.elf")).expect("Failed to load ELF");
    /// println!("Entry point: 0x{:08x}", entry);
    /// ```
    pub fn load_elf(&mut self, path: &Path) -> Result<u32, String> {
        self.load_elf_internal(path, true)
    }

    /// Load an ELF file into the simulator and reset without booting the CPU
    ///
    /// This function loads the ELF file into simulator memory, extracts the entry point,
    /// and performs a hardware reset but skips the CPU boot sequence. The CPU is left in
    /// the boot state (S_BOOT), allowing the calling code to handle the boot externally
    /// via bus requests (e.g., reading STATUS and writing BOOT address).
    ///
    /// This is used by the fpga-host integration where the boot sequence is managed
    /// by the host application.
    ///
    /// # Arguments
    /// * `path` - Path to the RISC-V ELF executable file
    ///
    /// # Returns
    /// * `Ok(entry_point)` with the ELF entry point address on success
    /// * `Err(String)` if the ELF file cannot be loaded or is invalid
    pub fn load_elf_no_boot(&mut self, path: &Path) -> Result<u32, String> {
        self.load_elf_internal(path, false)
    }

    /// Reset simulator state with boot deferred.
    ///
    /// This initializes internal reset/controller state without issuing a boot
    /// command, so external host-driven boot flows can start from a clean state.
    pub fn reset(&mut self) -> Result<(), String> {
        self.simulator
            .reset(0, false)
            .map_err(|e| format!("Reset failed: {}", e))
    }

    /// Internal helper for loading an ELF file with optional boot
    fn load_elf_internal(&mut self, path: &Path, boot_cpu: bool) -> Result<u32, String> {
        // Load ELF into simulator memory using the helper function
        let entry_point = {
            let mut view = SimulatorView::new(
                &mut self.simulator.bus,
                &self.simulator.cpu,
                &mut self.simulator.host_bus_handler,
                &mut self.simulator.host_bus_direct_response,
            );
            load_elf(&mut view, path).map_err(|e| format!("Error loading ELF: {}", e))?
        };

        log::info!(
            "ELF loaded successfully, entry point: 0x{:08x}",
            entry_point
        );

        // Reset the simulator to the entry point
        self.simulator
            .reset(entry_point, boot_cpu)
            .map_err(|e| format!("Reset failed: {}", e))?;

        Ok(entry_point)
    }

    /// Register a custom bus device at the specified base address
    ///
    /// This allows you to register custom peripherals (like Video or Audio devices)
    /// that will be accessible via memory-mapped I/O before loading an ELF file.
    /// Devices must be registered before calling `load_elf()`.
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device in the system memory map (must be word-aligned)
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(String)` - Address range conflicts with existing device or invalid alignment
    ///
    /// # Examples
    /// ```no_run
    /// use bus_shared::{BusDevice, Video, VideoConfig, VIDEO_BASE};
    /// use cpu_sim::InteractiveSimulator;
    /// use std::path::Path;
    ///
    /// fn frame_callback(_data: &[u8], config: &VideoConfig) {
    ///     println!("Frame received: {}x{}", config.width, config.height);
    /// }
    ///
    /// let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    ///
    /// // Register a video device with a callback
    /// let video: Box<dyn BusDevice> = Box::new(Video::new(Some(frame_callback)));
    /// sim.register_device(VIDEO_BASE, video).expect("Failed to register Video");
    ///
    /// // Now load and run your ELF
    /// sim.load_elf(Path::new("program.elf")).expect("Failed to load ELF");
    /// loop {
    ///     match sim.step_instruction() {
    ///         Ok(result) => {
    ///             if result.tohost_value.is_some() {
    ///                 break;
    ///             }
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Error: {}", e);
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn crate::BusDevice>,
    ) -> Result<(), String> {
        self.simulator
            .bus
            .register_device(base_addr, device)
            .map_err(|e| format!("{}", e))
    }

    /// Execute a single instruction and return the result
    ///
    /// Steps the simulator forward by one instruction. This may take multiple clock cycles
    /// depending on the instruction type and memory latency configuration.
    /// This can be called before loading an ELF when external boot controls are used.
    ///
    /// # Returns
    /// * `Ok(SimulationStepInstructionResult)` containing execution information (elapsed time
    ///   and cycles executed) and optional tohost termination value
    ///
    /// # Errors
    /// - Returns an error if the CPU enters a hung state
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::InteractiveSimulator;
    /// # use std::path::Path;
    /// let mut sim = InteractiveSimulator::new().unwrap();
    /// sim.load_elf(Path::new("test.elf")).unwrap();
    ///
    /// // Execute one instruction
    /// match sim.step_instruction() {
    ///     Ok(result) => {
    ///         if let Some(tohost) = result.tohost_value {
    ///             println!("Program halted with value: 0x{:08x}", tohost);
    ///         }
    ///     }
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn step_instruction(&mut self) -> Result<SimulationStepInstructionResult, String> {
        // Step the simulator by one instruction
        self.simulator
            .step_instruction()
            .map_err(|e| format!("Execution error: {}", e))
    }

    /// Execute a single clock cycle and return the result
    ///
    /// Steps the simulator forward by one clock cycle. This is a lower-level interface
    /// than `step_instruction()`, allowing cycle-by-cycle control for debugging or
    /// timing-sensitive testing.
    /// This can be called before loading an ELF when external boot controls are used.
    ///
    /// # Returns
    /// * `Ok(SimulationStepCycleResult)` containing:
    ///   - `instruction_completed`: true if the current instruction completed on this cycle
    ///   - `tohost_value`: Some(value) if halt detected, None otherwise
    ///   - `elapsed_cpu_time_us`: CPU time elapsed during this cycle in microseconds
    ///
    /// # Errors
    /// - Returns an error if the CPU enters a hung state
    ///
    /// # Examples
    /// ```no_run
    /// # use cpu_sim::InteractiveSimulator;
    /// # use std::path::Path;
    /// let mut sim = InteractiveSimulator::new().unwrap();
    /// sim.load_elf(Path::new("test.elf")).unwrap();
    ///
    /// // Execute cycles until instruction completes
    /// loop {
    ///     match sim.step_cycle() {
    ///         Ok(result) if result.instruction_completed => {
    ///             println!("Instruction completed");
    ///             break;
    ///         }
    ///         Ok(_) => {
    ///             println!("Cycle done, instruction still executing...");
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Error: {}", e);
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn step_cycle(&mut self) -> Result<SimulationStepCycleResult, String> {
        // Step the simulator by one cycle
        self.simulator
            .step_cycle()
            .map_err(|e| format!("Execution error: {}", e))
    }

    /// Send a bus request from the host to the RTL target
    ///
    /// This forwards the request to the simulator's internal host bus handler.
    /// The request will be processed during subsequent `step_instruction()` calls.
    ///
    /// # Arguments
    /// * `request` - Bus request (read or write) to send to the RTL target
    ///
    /// # Returns
    /// * `Ok(())` - Request queued successfully
    /// * `Err(String)` - Request rejected (already pending, or invalid address)
    pub fn send_bus_request(&mut self, request: BusRequest) -> Result<(), String> {
        let mut view = SimulatorView::new(
            &mut self.simulator.bus,
            &self.simulator.cpu,
            &mut self.simulator.host_bus_handler,
            &mut self.simulator.host_bus_direct_response,
        );
        view.send_bus_request(request)
    }

    /// Receive a bus response from the RTL target
    ///
    /// Returns the response for the most recently completed host-initiated request.
    ///
    /// # Returns
    /// * `Some(response)` - Response received
    /// * `None` - No response available yet
    pub fn receive_bus_response(&mut self) -> Option<BusResponse> {
        let mut view = SimulatorView::new(
            &mut self.simulator.bus,
            &self.simulator.cpu,
            &mut self.simulator.host_bus_handler,
            &mut self.simulator.host_bus_direct_response,
        );
        view.receive_bus_response()
    }

    /// Write a region of memory from a byte slice
    ///
    /// Writes bytes into the simulator's memory starting at `start_addr`.
    ///
    /// After writing, the simulator is reset with boot deferred (CPU left in
    /// S_BOOT state) so that `boot_cpu` can be called externally.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address (must be in DRAM range)
    /// * `data` - Byte slice containing the data to write
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` if the reset fails
    pub fn write_memory_region(&mut self, start_addr: u32, data: &[u8]) -> Result<(), String> {
        {
            let mut view = SimulatorView::new(
                &mut self.simulator.bus,
                &self.simulator.cpu,
                &mut self.simulator.host_bus_handler,
                &mut self.simulator.host_bus_direct_response,
            );
            view.write_memory_region(start_addr, data);
        }

        // Reset with boot deferred so boot_cpu can be called externally
        self.simulator
            .reset(start_addr, false)
            .map_err(|e| format!("Reset failed: {}", e))?;

        Ok(())
    }
}

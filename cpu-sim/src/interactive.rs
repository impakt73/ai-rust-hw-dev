use crate::sim::Simulator;
use crate::{
    BusRequest, BusResponse, InstructionTrace, SimulationStepCycleResult,
    SimulationStepInstructionResult, SimulatorView,
};

// Type alias for InteractiveSimulator's internal simulator type
type InteractiveSimulatorType = Simulator<fn(&mut SimulatorView), fn(&InstructionTrace)>;

/// Interactive wrapper around the Simulator for step-by-step execution
///
/// This structure provides a controlled interface for interactive use of the simulator,
/// allowing users to load programs and step through execution instruction-by-instruction.
/// Unlike the `run_program` function which runs to completion,
/// `InteractiveSimulator` gives you fine-grained control over execution.
///
/// # Examples
/// ```no_run
/// use cpu_sim::InteractiveSimulator;
///
/// let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
/// sim.load_program(0x8000_0000, &[]).expect("Failed to load program");
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

    /// Reset simulator state with boot deferred.
    ///
    /// This initializes internal reset/controller state without issuing a boot
    /// command, so external host-driven boot flows can start from a clean state.
    pub fn reset(&mut self) -> Result<(), String> {
        self.simulator
            .reset()
            .map_err(|e| format!("Reset failed: {}", e))
    }

    /// Register a custom bus device at the specified base address
    ///
    /// This allows you to register custom peripherals (like Video or Audio devices)
    /// that will be accessible via memory-mapped I/O before loading a program.
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
    /// // Now load and run your program
    /// sim.load_program(0x8000_0000, &[]).expect("Failed to load program");
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
    /// let mut sim = InteractiveSimulator::new().unwrap();
    /// sim.load_program(0x8000_0000, &[]).unwrap();
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
    /// let mut sim = InteractiveSimulator::new().unwrap();
    /// sim.load_program(0x8000_0000, &[]).unwrap();
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

    /// Load a program from raw bytes into the simulator and boot the CPU
    ///
    /// Writes the provided bytes into simulator memory starting at `start_addr`,
    /// then resets the CPU and boots it from that address.  This is the
    /// direct-instruction equivalent of `load_elf` and lets tests drive the
    /// simulator with hand-crafted instruction sequences.
    ///
    /// # Arguments
    /// * `start_addr` - Starting address for the program (must be in DRAM range)
    /// * `data` - Raw instruction bytes to load
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` if the reset / boot sequence fails
    pub fn load_program(&mut self, start_addr: u32, data: &[u8]) -> Result<(), String> {
        self.simulator
            .reset()
            .map_err(|e| format!("Reset failed: {}", e))?;

        {
            let mut view = SimulatorView::new(
                &mut self.simulator.bus,
                &self.simulator.cpu,
                &mut self.simulator.host_bus_handler,
                &mut self.simulator.host_bus_direct_response,
            );
            view.write_memory_region(start_addr, data);
        }

        self.simulator
            .boot(start_addr)
            .map_err(|e| format!("Boot failed: {}", e))?;

        Ok(())
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
        // Reset with boot deferred so boot_cpu can be called externally
        self.simulator
            .reset()
            .map_err(|e| format!("Reset failed: {}", e))?;

        {
            let mut view = SimulatorView::new(
                &mut self.simulator.bus,
                &self.simulator.cpu,
                &mut self.simulator.host_bus_handler,
                &mut self.simulator.host_bus_direct_response,
            );
            view.write_memory_region(start_addr, data);
        }

        Ok(())
    }
}

use crate::hung_detector::{HungDetector, HungDetectorConfig, HungStateError};
use crate::simulator_view::SimulatorView;
use bus_shared::SystemBus;
use host_bus_handler::{AccessSize, BusRequest, BusResponse, HostBusHandler};
use riscv_core::trace::InstructionTrace;
use riscv_core::{Top, Vcd, VerilatedModelConfig, VerilatorRuntime};
use std::time::Instant;

/// Maximum number of clock cycles allowed for any boot phase before timing out
const BOOT_TIMEOUT_CYCLES: u32 = 10_000;

/// Errors that can occur during the CPU boot/reset sequence
#[derive(Debug)]
pub enum BootError {
    /// A boot phase timed out waiting for a condition
    Timeout { phase: &'static str, cycles: u32 },
    /// The CPU STATUS register indicates an unexpected state
    UnexpectedStatus { status: u32 },
}

impl std::fmt::Display for BootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootError::Timeout { phase, cycles } => {
                write!(f, "Boot timeout in '{}' after {} cycles", phase, cycles)
            }
            BootError::UnexpectedStatus { status } => {
                write!(
                    f,
                    "CPU is not in boot state after reset - hardware issue (STATUS=0x{:08x})",
                    status
                )
            }
        }
    }
}

impl std::error::Error for BootError {}

/// Pending response for a host-initiated request (awaiting completion after latency)
#[derive(Debug, Clone)]
struct PendingResponse {
    /// The bus response to send
    response: BusResponse,
    /// Cycle at which the request was accepted
    accepted_cycle: u64,
}

/// Result of stepping a single clock cycle
#[derive(Debug)]
pub struct SimulationStepCycleResult {
    pub instruction_completed: bool,
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
    // Immediate response for host requests routed directly to SystemBus
    pub(crate) host_bus_direct_response: Option<BusResponse>,
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
            host_bus_direct_response: None,
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

    /// Handle callbacks and tracing when an instruction completes
    fn handle_instruction_complete(&mut self) {
        // Call inst_complete callback if provided (after instruction completion)
        // This callback receives restricted access to the Simulator via SimulatorView
        if let Some(ref mut callback) = self.inst_complete_callback {
            let mut view = SimulatorView::new(
                &mut self.bus,
                &self.cpu,
                &mut self.host_bus_handler,
                &mut self.host_bus_direct_response,
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

    /// Reset the CPU hardware and leave it in boot state (S_BOOT).
    ///
    /// # Returns
    /// * `Ok(())` if reset succeeds
    /// * `Err(BootError)` if a timeout occurs while waiting for reset completion
    pub fn reset(&mut self) -> Result<(), BootError> {
        // Initialize host bus interface signals
        // host_tx_ready is always 1 because the handler can buffer requests/responses
        // and the FPGA side never sends more than one request at a time
        self.cpu.host_tx_ready = 1;
        self.cpu.host_rx_valid = 0;
        self.cpu.host_rx_data = 0;
        self.cpu.com_err = 0;
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
        self.cycle_count = 0;
        self.total_elapsed_time_us = 0;

        // Reset the host bus handler
        self.host_bus_handler.reset();
        self.host_bus_direct_response = None;
        self.pending_response = None;

        // Boot the CPU via host bus requests to the system controller peripheral
        // First, run clock cycles until the internal reset controller has completed
        // (rst_n_out goes high). The reset controller holds internal reset for
        // RESET_CYCLES (default 8) after external rst_n goes high.
        for cycle in 0..BOOT_TIMEOUT_CYCLES {
            self.boot_clock_cycle();
            if self.cpu.rst_n_out != 0 {
                break;
            }
            if cycle == BOOT_TIMEOUT_CYCLES - 1 {
                return Err(BootError::Timeout {
                    phase: "waiting for reset controller",
                    cycles: BOOT_TIMEOUT_CYCLES,
                });
            }
        }

        log::info!("CPU hardware reset complete (boot deferred)");
        Ok(())
    }

    /// Boot the CPU from the S_BOOT state by writing the system-controller BOOT register.
    ///
    /// This method assumes [`Self::reset`] has completed and the CPU is waiting for a
    /// host boot command. It first reads the system-controller status register to verify
    /// the CPU is still in boot-wait state, then writes `boot_pc` to the BOOT register.
    ///
    /// # Arguments
    /// * `boot_pc` - Program counter value to boot from.
    ///
    /// # Returns
    /// * `Ok(())` if boot sequence completes successfully.
    ///
    /// # Errors
    /// * [`BootError::Timeout`] if a host-bus response times out.
    /// * [`BootError::UnexpectedStatus`] if the CPU is not in the expected boot-wait state.
    pub fn boot(&mut self, boot_pc: u32) -> Result<(), BootError> {
        // Step 1: Read STATUS register to confirm CPU is waiting to be booted
        let status_addr = riscv_shared::bus::sysctrl_status_addr();
        let status_request = BusRequest::read(status_addr, AccessSize::Word);
        self.host_bus_handler
            .send_request(status_request)
            .expect("Failed to send STATUS read request");

        let response = self.wait_for_bus_response("STATUS read")?;
        // Check that cpu_booting bit (bit 0) is set
        if (response.rdata & riscv_shared::bus::SYSCTRL_STATUS_CPU_BOOTING) == 0 {
            return Err(BootError::UnexpectedStatus {
                status: response.rdata,
            });
        }

        // Step 2: Write boot address to BOOT register to complete boot process
        let boot_addr = riscv_shared::bus::sysctrl_boot_addr();
        let boot_request = BusRequest::write(boot_addr, boot_pc, AccessSize::Word);
        self.host_bus_handler
            .send_request(boot_request)
            .expect("Failed to send BOOT write request");

        // Wait for write acknowledgement - CPU boot process is now complete
        self.wait_for_bus_response("BOOT write")?;
        Ok(())
    }

    /// Execute a single clock cycle during the boot process
    ///
    /// This is a simplified clock cycle that only handles the host bus interface
    /// protocol and clock edge. It does NOT increment cycle_count, call
    /// bus.clock_cycle_all_devices(), run hung detector checks, or print FSM state.
    fn boot_clock_cycle(&mut self) {
        self.cpu.clk = 0;
        self.cpu.eval();
        self.handle_host_bus_interface();
        self.cpu.clk = 1;
        self.cpu.eval();
        self.dump_vcd();
    }

    /// Cycle the design until a bus response is received, then return it.
    /// Returns a timeout error if no response is received within BOOT_TIMEOUT_CYCLES.
    fn wait_for_bus_response(&mut self, phase: &'static str) -> Result<BusResponse, BootError> {
        for cycle in 0..BOOT_TIMEOUT_CYCLES {
            self.boot_clock_cycle();
            if let Some(response) = self.host_bus_handler.receive_response() {
                return Ok(response);
            }
            if cycle == BOOT_TIMEOUT_CYCLES - 1 {
                return Err(BootError::Timeout {
                    phase,
                    cycles: BOOT_TIMEOUT_CYCLES,
                });
            }
        }
        unreachable!()
    }

    /// Get the current LED output value
    ///
    /// Returns the 8-bit LED output value from the LED controller peripheral.
    #[allow(dead_code)]
    pub fn led_out(&self) -> u8 {
        self.cpu.led_out
    }

    /// Execute a single clock cycle
    ///
    /// This method steps the simulation forward by one clock cycle and returns
    /// a result indicating whether the current instruction has completed, along
    /// with timing information and optional termination status.
    ///
    /// # Returns
    /// * `Ok(SimulationStepCycleResult)` - Cycle completed successfully
    /// * `Err(HungStateError)` - Hung state detected
    pub fn step_cycle(&mut self) -> Result<SimulationStepCycleResult, HungStateError> {
        let start_time = Instant::now();

        // Clock edge
        self.cpu.clk = 0;
        self.cpu.eval();

        // Handle host bus interface protocol
        // The CPU sends serialized bus transactions via host_tx_* signals
        // and we respond via host_rx_* signals
        self.handle_host_bus_interface();

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

        // Check for hung state on every cycle, but skip when CPU is in boot state
        // (S_BOOT) since it is not expected to make progress on instructions until booted.
        // Also skip once the CPU has halted, since halt is a terminal state.
        if self.cpu.cpu_booting == 0 && self.cpu.halted == 0 {
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
        }

        if instruction_complete {
            self.handle_instruction_complete();
        }

        let elapsed_us = start_time.elapsed().as_micros() as u64;

        // Accumulate elapsed time
        self.total_elapsed_time_us = self.total_elapsed_time_us.saturating_add(elapsed_us);

        // Update bus with cumulative elapsed time for devices
        // This ensures Video and other time-sensitive devices get accurate cumulative time
        self.bus.update_elapsed_time(self.total_elapsed_time_us);

        // Check for termination via SimControl device
        let halt_value = self.bus.sim_control.acknowledge_termination();

        Ok(SimulationStepCycleResult {
            instruction_completed: instruction_complete,
            tohost_value: halt_value,
            elapsed_cpu_time_us: elapsed_us,
        })
    }

    /// Run the simulation for up to max_cycles
    ///
    /// **Note:** This method does not reset or boot the CPU. Callers are responsible
    /// for invoking `reset()`/`boot()` before calling `run()`.
    ///
    /// Returns Ok(SimulationResult) on normal completion or Err on error
    ///
    /// # Arguments
    /// * `max_cycles` - Maximum number of cycles to run
    ///
    /// # Errors
    /// Returns error if hung state is detected or other simulation errors occur
    pub fn run(&mut self, max_cycles: u64) -> Result<SimulationResult, String> {
        log::info!("Starting simulation (max {} cycles)", max_cycles);

        let mut total_elapsed_us: u64 = 0;

        while self.cycle_count < max_cycles {
            // Execute one cycle and terminate immediately if a halt signal is observed.
            let step_result = self
                .step_cycle()
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

#[cfg(test)]
mod tests {
    use super::*;
    use bus_shared::SIM_CONTROL_BASE;

    #[test]
    fn test_run_returns_immediately_on_pending_tohost() {
        let mut sim: Simulator<fn(&mut SimulatorView), fn(&InstructionTrace)> =
            Simulator::new(false, false, None, None, None, 0, 0)
                .expect("simulator should initialize");
        sim.reset().expect("reset should succeed");

        let expected_tohost = 0x2a;
        sim.bus.write_word(SIM_CONTROL_BASE, expected_tohost);

        let result = sim
            .run(sim.cycle_count + 1)
            .expect("run should succeed with pending tohost");

        assert_eq!(result.tohost_value, Some(expected_tohost));
    }
}

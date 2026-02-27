use super::sim_core::Simulator;
use super::{InstructionTrace, SimulationResult, SimulatorView};
/// Unified program execution function that supports programmatic instruction loading
///
/// This is the single entry point for running programs on the simulator. It uses a pre-execution
/// callback to handle programmatic instruction loading and returns the entry point for simulation.
///
/// # Arguments
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `print_fsm_state` - Whether to print FSM state transitions
/// * `inst_complete_callback` - Optional callback invoked after each instruction completes
/// * `trace_callback` - Optional callback for instruction traces
/// * `vcd_path` - Optional path to VCD file for waveform dumping
/// * `setup_callback` - Pre-execution callback that loads the program and returns entry point
/// * `termination_callback` - Optional post-execution callback with access to simulator and result
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::run_program;
///
/// // Example: Load instruction array programmatically
/// run_program(
///     1000,
///     false,
///     false,
///     None::<fn(&mut cpu_sim::SimulatorView)>,
///     None::<fn(&cpu_sim::InstructionTrace)>,
///     None,
///     0, // Zero latency
///     |sim| {
///         let instructions = vec![0x00000093u32]; // addi x1, x0, 0
///         let start_addr = 0x8000_0000;
///         let bytes: Vec<u8> = instructions.iter()
///             .flat_map(|i| i.to_le_bytes())
///             .collect();
///         sim.write_memory_region(start_addr, &bytes);
///         Ok(start_addr)
///     },
///     None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>
/// )?;
/// # Ok::<(), String>(())
/// ```
#[allow(clippy::too_many_arguments)]
pub fn run_program<F, T, P, C>(
    max_cycles: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    setup_callback: P,
    termination_callback: Option<C>,
) -> Result<SimulationResult, String>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
    P: FnOnce(&mut SimulatorView) -> Result<u32, String>,
    C: FnOnce(&SimulatorView, &SimulationResult),
{
    // Initialize CPU Simulator (runtime, bus, and hung detector created internally)
    let mut sim = Simulator::new(
        print_inst_trace,
        print_fsm_state,
        inst_complete_callback,
        trace_callback,
        vcd_path,
        mem_latency_cycles,
        0, // verilator_optimization (default 0 for compatibility)
    )?;

    // Reset first so setup callback can initialize memory/FIFO on a clean state
    sim.reset().map_err(|e| format!("Reset failed: {}", e))?;

    // Execute pre-execution callback to load program and get entry point
    // Create a SimulatorView for the setup callback
    let entry_point = {
        let mut view = SimulatorView::new(
            &mut sim.bus,
            &sim.cpu,
            &mut sim.host_bus_handler,
            &mut sim.host_bus_direct_response,
        );
        setup_callback(&mut view)?
    };

    log::info!("Program loaded, entry point: 0x{:08x}", entry_point);

    // Boot CPU at entry point, then run simulation
    sim.boot(entry_point)
        .map_err(|e| format!("Boot failed: {}", e))?;

    let result = sim.run(max_cycles)?;

    // Execute optional post-execution callback with read-only SimulatorView and result
    if let Some(callback) = termination_callback {
        let view = SimulatorView::new(
            &mut sim.bus,
            &sim.cpu,
            &mut sim.host_bus_handler,
            &mut sim.host_bus_direct_response,
        );
        callback(&view, &result);
    }

    Ok(result)
}

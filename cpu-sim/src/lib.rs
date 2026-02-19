// Internal modules - not part of public API
mod constants;
mod hung_detector;
mod interactive;
pub mod packet_transport; // Public for integration tests
mod sim;
mod simulator_view;

// Public API exports - only what's needed for external use
pub use bus_shared::{
    is_valid_dram_range, Audio, AudioChannels, AudioConfig, AudioSampleRate, BusDevice,
    BusDeviceError, Dma, Fifo, FifoDataReceivedCallback, FifoDataSource, RegistrationError,
    SystemBus, SystemContext, Video, VideoConfig, VideoFormat, AUDIO_BASE, DRAM_BASE, DRAM_END,
    FIFO_BASE, LED_BASE, SIM_CONTROL_BASE, VIDEO_BASE,
};
pub use constants::GLOBAL_MAX_CYCLES;
pub use host_bus_handler::{AccessSize, BusRequest, BusResponse};
pub use interactive::InteractiveSimulator;
pub use riscv_core::trace::InstructionTrace;
pub use sim::{
    BootError, SimulationResult, SimulationStepCycleResult, SimulationStepInstructionResult,
};
pub use simulator_view::SimulatorView;

use sim::Simulator;
use std::path::Path;

/// Push a UTF-8 string into a FIFO RX queue as little-endian u32 words.
///
/// If the input length is a multiple of 4 bytes, a trailing zero word is appended.
pub fn push_string_to_fifo_rx(fifo_source: &FifoDataSource, s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut word = 0u32;
        for j in 0..4 {
            if i + j < bytes.len() {
                word |= (bytes[i + j] as u32) << (j * 8);
            }
        }
        fifo_source.write_word(word);
        i += 4;
    }
    if bytes.len().is_multiple_of(4) {
        fifo_source.write_word(0);
    }
}
/// Load an ELF file into a simulator's memory
///
/// This is a private helper function used by run_elf to load ELF files.
/// External users should use run_elf instead.
fn load_elf(sim: &mut SimulatorView, path: &Path) -> Result<u32, Box<dyn std::error::Error>> {
    let file_data = std::fs::read(path)?;
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)?;

    let mut entry_point = 0;

    // Get the entry point
    if let Ok(header) = elf_file.ehdr.e_entry.try_into() {
        entry_point = header;
    }

    // Load program headers (segments)
    if let Some(phdrs) = elf_file.segments() {
        for phdr in phdrs.iter() {
            // Only load LOAD segments
            if phdr.p_type == elf::abi::PT_LOAD {
                let vaddr = phdr.p_vaddr as u32;
                let file_size = phdr.p_filesz as usize;
                let _mem_size = phdr.p_memsz as usize; // May be larger than file_size (BSS)
                let offset = phdr.p_offset as usize;

                // Check if segment is executable (contains code)
                let is_executable = (phdr.p_flags & elf::abi::PF_X) != 0;

                if file_size > 0 {
                    // Validate that the segment lies within the file data to avoid panics
                    let end = match offset.checked_add(file_size) {
                        Some(end) if end <= file_data.len() => end,
                        _ => {
                            return Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!(
                                    "ELF segment out of bounds: offset=0x{:x}, size=0x{:x}, file_len=0x{:x}",
                                    offset,
                                    file_size,
                                    file_data.len()
                                ),
                            )));
                        }
                    };

                    let segment_data = &file_data[offset..end];
                    // Write segment bytes to memory
                    sim.write_memory_region(vaddr, segment_data);
                    log::info!(
                        "Loaded segment: vaddr=0x{:08x}, size=0x{:x} bytes{}",
                        vaddr,
                        file_size,
                        if is_executable { " (executable)" } else { "" }
                    );
                }
            }
        }
    }

    Ok(entry_point)
}

/// Run an ELF file on the simulated CPU with full configuration options
///
/// This function provides the same interface as `run_program`, but with an ELF file path
/// instead of a setup callback. It loads the ELF file into memory and executes it.
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `print_fsm_state` - Whether to print FSM state transitions
/// * `inst_complete_callback` - Optional callback invoked after each instruction completes
/// * `trace_callback` - Optional callback for instruction traces
/// * `vcd_path` - Optional path to VCD file for waveform dumping
/// * `mem_latency_cycles` - Number of cycles for memory latency simulation
/// * `setup_callback` - Optional callback for additional setup after ELF is loaded
/// * `termination_callback` - Optional post-execution callback with access to simulator and result
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::run_elf;
/// use std::path::Path;
///
/// // Simple usage
/// let result = run_elf(
///     Path::new("test.elf"),
///     1000,
///     false, // print_inst_trace
///     false, // print_fsm_state
///     None::<fn(&mut cpu_sim::SimulatorView)>,
///     None::<fn(&cpu_sim::InstructionTrace)>,
///     None, // vcd_path
///     0, // mem_latency_cycles
///     None::<fn(&mut cpu_sim::SimulatorView)>, // setup_callback
///     None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>
/// )?;
/// assert_eq!(result.tohost_value, Some(0x2a));
///
/// // With setup callback for additional initialization after ELF is loaded
/// run_elf(
///     Path::new("test.elf"),
///     1000,
///     false,
///     false,
///     None::<fn(&mut cpu_sim::SimulatorView)>,
///     None::<fn(&cpu_sim::InstructionTrace)>,
///     None,
///     0,
///     Some(|sim: &mut cpu_sim::SimulatorView| {
///         // Additional setup after ELF is loaded
///         sim.write_memory_region(0x8000_1000, &[1, 2, 3, 4]);
///     }),
///     None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>
/// )?;
/// # Ok::<(), String>(())
/// ```
#[allow(clippy::too_many_arguments)]
pub fn run_elf<F, T, P, C>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    setup_callback: Option<P>,
    termination_callback: Option<C>,
) -> Result<SimulationResult, String>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
    P: FnOnce(&mut SimulatorView),
    C: FnOnce(&SimulatorView, &SimulationResult),
{
    run_program(
        max_cycles,
        print_inst_trace,
        print_fsm_state,
        inst_complete_callback,
        trace_callback,
        vcd_path,
        mem_latency_cycles,
        |sim| {
            // Load ELF into simulator memory
            let entry_point =
                load_elf(sim, elf_path).map_err(|e| format!("Error loading ELF: {}", e))?;

            log::info!("ELF loaded successfully");

            // Call optional setup callback for additional setup after ELF loading
            if let Some(callback) = setup_callback {
                callback(sim);
            }

            Ok(entry_point)
        },
        termination_callback,
    )
}

/// Unified program execution function that supports both ELF and programmatic instruction loading
///
/// This is the single entry point for running programs on the simulator. It uses a pre-execution
/// callback to handle different loading strategies (ELF vs instruction array) and returns the
/// entry point for simulation.
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
/// use std::path::Path;
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
///
/// // For loading ELF files, use run_elf() instead
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

// Internal modules - not part of public API
mod bus;
mod bus_device;
mod dram;
mod fifo;
mod hung_detector;
mod memory;
pub mod packet_transport; // Public for integration tests
mod sim;
mod sim_control;

// Public API exports - only what's needed for external use
pub use bus::{DRAM_BASE, DRAM_END, FIFO_BASE, SIM_CONTROL_BASE};
pub use bus_device::{BusDevice, BusDeviceError, RegistrationError, SystemContext};
pub use riscv_core::trace::InstructionTrace;
pub use sim::{SimulationResult, SimulatorView};

use bus::SystemBus;
use hung_detector::HungDetectorConfig;
use sim::Simulator;
use std::path::Path;
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
                    // Write to memory (passing true for is_instructions if segment is executable)
                    sim.write_memory_region(vaddr, segment_data, is_executable);
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

    // PC range is automatically set by write_memory_region calls above for executable segments
    log::info!("ELF loaded with entry point: 0x{:08x}", entry_point);
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
/// // With setup callback to write to FIFO after ELF is loaded
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
///         sim.fifo_write_rx_string("test data");
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
            log::info!("Entry point: 0x{:08x}", entry_point);

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
///         sim.write_memory_region(start_addr, &bytes, true); // true = instructions
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
    // Create system bus with internal DRAM
    let bus = SystemBus::new();

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .map_err(|e| format!("Error creating CPU runtime: {}", e))?;

    let mut sim = Simulator::new(
        &runtime,
        bus,
        print_inst_trace,
        print_fsm_state,
        inst_complete_callback,
        trace_callback,
        vcd_path,
        mem_latency_cycles,
        Some(HungDetectorConfig::default()),
    )?;

    // Execute pre-execution callback to load program and get entry point
    // Create a SimulatorView for the setup callback
    let entry_point = {
        let mut view = SimulatorView::new(&mut sim.bus, &mut sim.hung_detector);
        setup_callback(&mut view)?
    };

    log::info!("Program loaded, entry point: 0x{:08x}", entry_point);

    // Run simulation with entry point as boot PC
    // Note: run() handles reset internally, so we don't call reset() here
    let result = sim.run(entry_point, max_cycles)?;

    // Execute optional post-execution callback with read-only SimulatorView and result
    if let Some(callback) = termination_callback {
        let view = SimulatorView::new(&mut sim.bus, &mut sim.hung_detector);
        callback(&view, &result);
    }

    Ok(result)
}

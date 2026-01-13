// Internal modules - not part of public API
mod bus;
mod dram;
mod fifo;
mod hung_detector;
pub mod packet_transport; // Public for integration tests
mod sim;

// Public API exports - only what's needed for external use
pub use riscv_core::trace::InstructionTrace;
pub use sim::{SimulationResult, SimulatorView};

use bus::SystemBus;
use hung_detector::HungDetectorConfig;
use sim::Simulator;
use std::path::Path;
/// Load an ELF file into a simulator's memory
///
/// This function reads an ELF file and loads its LOAD segments into the simulator's
/// memory using the write_memory_region function. This allows loading programs
/// after simulator initialization rather than requiring the ELF to be loaded into
/// DRAM before creating the simulator.
///
/// # Arguments
/// * `sim` - Mutable reference to the simulator to load the ELF into
/// * `path` - Path to the ELF file to load
///
/// # Returns
/// * `Ok(u32)` - The entry point address from the ELF file
/// * `Err(Box<dyn std::error::Error>)` - An error if loading fails
///
/// # Examples
/// ```no_run
/// # use cpu_sim::*;
/// # use std::path::Path;
/// #
/// # fn main() -> Result<(), String> {
/// // load_elf is typically used within run_program's prep_callback
/// let result = run_program(
///     1000,
///     false, // print_inst_trace
///     false, // print_fsm_state
///     None::<fn(&mut SimulatorView)>,
///     None::<fn(&InstructionTrace)>,
///     None, // vcd_path
///     0, // mem_latency_cycles
///     |sim| {
///         // load_elf is called here to load the program
///         load_elf(sim, Path::new("program.elf")).map_err(|e| e.to_string())
///     },
///     |_sim, _result| {},
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn load_elf(
    sim: &mut SimulatorView,
    path: &Path,
) -> Result<u32, Box<dyn std::error::Error>> {
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

/// Internal unified function for running ELF files with all possible options
///
/// This delegates to run_elf_in_simulator_internal with a no-op callback.
fn run_elf_internal<F, T>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
{
    run_elf_in_simulator_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        fifo_rx_data,
        trace_callback,
        vcd_path,
        0, // Zero latency for backward compatibility
        |_sim, _result| {
            // No-op callback - just run the simulation without post-processing
        },
    )
}

/// Run an ELF file on the simulated CPU
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
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
/// let result = run_elf(Path::new("test.elf"), 1000, false)?;
/// assert_eq!(result.tohost_value, Some(0x2a));
/// # Ok::<(), String>(())
/// ```
pub fn run_elf(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
) -> Result<SimulationResult, String> {
    run_elf_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(&mut SimulatorView)>,
        None,
        None::<fn(&InstructionTrace)>,
        None,
    )
}

/// Internal helper function that consolidates the common pattern for running an ELF
/// with a callback that has access to the simulator after execution.
///
/// This delegates to the unified run_program function.
#[allow(clippy::too_many_arguments)]
fn run_elf_in_simulator_internal<F, T, C>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    inst_complete_callback: Option<F>,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    callback: C,
) -> Result<SimulationResult, String>
where
    F: FnMut(&mut SimulatorView),
    T: FnMut(&InstructionTrace),
    C: FnOnce(&SimulatorView, &SimulationResult),
{
    run_program(
        max_cycles,
        print_inst_trace,
        false, // Don't print FSM state
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

            // Write data to RX FIFO if provided
            if let Some(data) = fifo_rx_data {
                sim.fifo_write_rx_string(data);
            }

            Ok(entry_point)
        },
        callback,
    )
}

/// Run an ELF file in a simulator with full configuration options
///
/// This is the most flexible simulator execution function, supporting all options
/// including instruction trace printing, VCD dumping, and callback access.
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `callback` - Function to execute with simulator access after the run completes
/// * `vcd_path` - Optional path to VCD file for waveform dumping
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_in_simulator_with_options<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    callback: F,
    vcd_path: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: FnOnce(&SimulatorView, &SimulationResult),
{
    run_elf_in_simulator_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(&mut SimulatorView)>,
        None,
        None::<fn(&InstructionTrace)>,
        vcd_path,
        0, // Zero latency for backward compatibility
        callback,
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
/// * `prep_callback` - Pre-execution callback that loads the program and returns entry point
/// * `post_callback` - Post-execution callback with access to simulator and result
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
/// // Example 1: Load ELF file
/// run_program(
///     1000,
///     false,
///     false,
///     None::<fn(&mut cpu_sim::SimulatorView)>,
///     None::<fn(&cpu_sim::InstructionTrace)>,
///     None,
///     0, // Zero latency
///     |sim| {
///         let entry = cpu_sim::load_elf(sim, Path::new("test.elf"))
///             .map_err(|e| e.to_string())?;
///         Ok(entry)
///     },
///     |_sim, _result| {}
/// )?;
///
/// // Example 2: Load instruction array
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
///     |_sim, _result| {}
/// )?;
/// # Ok::<(), String>(())
/// ```
#[allow(clippy::too_many_arguments)]
pub fn run_program<F, T, P, C>(
    max_cycles: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    fifo_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    prep_callback: P,
    post_callback: C,
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
        fifo_callback,
        trace_callback,
        vcd_path,
        mem_latency_cycles,
        Some(HungDetectorConfig::default()),
    )?;

    // Execute pre-execution callback to load program and get entry point
    // Create a SimulatorView for the prep callback
    let entry_point = {
        let mut view = SimulatorView::new(
            &mut sim.bus.fifo,
            &mut sim.bus.dram,
            &mut sim.hung_detector,
        );
        prep_callback(&mut view)?
    };

    log::info!("Program loaded, entry point: 0x{:08x}", entry_point);

    // Run simulation with entry point as boot PC
    // Note: run() handles reset internally, so we don't call reset() here
    let result = sim.run(entry_point, max_cycles)?;

    // Execute post-execution callback with read-only SimulatorView and result
    {
        let view = SimulatorView::new(
            &mut sim.bus.fifo,
            &mut sim.bus.dram,
            &mut sim.hung_detector,
        );
        post_callback(&view, &result);
    }

    Ok(result)
}

pub mod bus;
pub mod dram;
pub mod fifo;
pub mod packet_transport;
pub mod sim;

#[cfg(test)]
mod tests;

pub use riscv_core::trace::InstructionTrace;
pub use sim::{SimulationResult, SimulationStepResult, Simulator};

use bus::SystemBus;
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
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = riscv_core::create_cpu_runtime()?;
/// let bus = bus::SystemBus::new();
/// let mut sim = Simulator::new(
///     &runtime,
///     bus,
///     false,
///     false,
///     None::<fn(u32)>,
///     None::<fn(&riscv_core::trace::InstructionTrace)>,
///     0, // Zero latency
/// )?;
/// let entry_point = load_elf(&mut sim, Path::new("program.elf"))?;
/// let result = sim.run(entry_point, 1000)?;
/// # Ok(())
/// # }
/// ```
pub fn load_elf<F, T>(
    sim: &mut Simulator<F, T>,
    path: &Path,
) -> Result<u32, Box<dyn std::error::Error>>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
{
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
                let offset = phdr.p_offset as usize;

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
                    sim.write_memory_region(vaddr, segment_data);
                    log::info!(
                        "Loaded segment: vaddr=0x{:08x}, size=0x{:x} bytes",
                        vaddr,
                        file_size
                    );
                }
            }
        }
    }

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
    F: FnMut(u32),
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

/// Run an ELF file on the simulated CPU with an optional FIFO callback and RX data
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
/// * `fifo_rx_data` - Optional string to write to the FIFO RX queue before running
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_fifo<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
{
    run_elf_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        fifo_rx_data,
        None::<fn(&InstructionTrace)>,
        None,
    )
}

/// Run an ELF file on the simulated CPU with an optional trace callback
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `trace_callback` - Optional callback invoked for each instruction executed (receives InstructionTrace)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::{run_elf_with_trace_callback, InstructionTrace};
/// use std::{path::Path, sync::{Arc, Mutex}};
///
/// let trace_count = Arc::new(Mutex::new(0usize));
/// let trace_count_cloned = Arc::clone(&trace_count);
/// let trace_callback = move |trace: &InstructionTrace| {
///     let mut count = trace_count_cloned.lock().unwrap();
///     *count += 1;
///     println!("Instruction {}: {:?}", *count, trace.inst_type);
/// };
///
/// let result = run_elf_with_trace_callback(
///     Path::new("test.elf"),
///     1000,
///     false,
///     Some(trace_callback)
/// )?;
/// # Ok::<(), String>(())
/// ```
pub fn run_elf_with_trace_callback<T>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    trace_callback: Option<T>,
) -> Result<SimulationResult, String>
where
    T: FnMut(&InstructionTrace),
{
    run_elf_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(u32)>,
        None,
        trace_callback,
        None,
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
        None::<fn(u32)>,
        None,
        None::<fn(&InstructionTrace)>,
        None,
    )
}

/// Run an ELF file on the simulated CPU with VCD waveform dumping
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `vcd_path` - Path to the VCD file to generate
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::run_elf_with_vcd;
/// use std::path::Path;
///
/// let result = run_elf_with_vcd(
///     Path::new("test.elf"),
///     1000,
///     false,
///     "trace.vcd"
/// )?;
/// # Ok::<(), String>(())
/// ```
pub fn run_elf_with_vcd(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    vcd_path: &str,
) -> Result<SimulationResult, String> {
    run_elf_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(u32)>,
        None,
        None::<fn(&InstructionTrace)>,
        Some(vcd_path),
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
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    callback: C,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
    C: for<'a> FnOnce(&mut Simulator<'a, F, T>, &SimulationResult),
{
    run_program(
        max_cycles,
        print_inst_trace,
        false, // Don't print FSM state
        fifo_callback,
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

/// Run an ELF file in a simulator and execute a callback with access to the simulator
///
/// This function provides a safe way to access the simulator after running an ELF file.
/// The callback is executed with a reference to the simulator, ensuring proper lifetime
/// management without memory leaks.
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `callback` - Function to execute with simulator access after the run completes
/// * `vcd_path` - Optional path to VCD file for waveform dumping
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::run_elf_in_simulator;
/// use std::path::Path;
///
/// run_elf_in_simulator(
///     Path::new("test.elf"),
///     1000,
///     |sim, result| {
///         println!("Simulation completed in {} cycles", result.cycles);
///         let bytes: Vec<u8> = sim.dump_memory_region(0x80000000, 1024).collect();
///         // Process bytes...
///     },
///     None, // No VCD
/// )?;
/// # Ok::<(), String>(())
/// ```
pub fn run_elf_in_simulator<F>(
    elf_path: &Path,
    max_cycles: u64,
    callback: F,
    vcd_path: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: for<'a> FnOnce(&Simulator<'a, fn(u32), fn(&InstructionTrace)>, &SimulationResult),
{
    run_elf_in_simulator_internal(
        elf_path,
        max_cycles,
        false,
        None::<fn(u32)>,
        None,
        None::<fn(&InstructionTrace)>,
        vcd_path,
        0, // Zero latency for backward compatibility
        |sim, result| callback(sim, result),
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
    F: for<'a> FnOnce(&Simulator<'a, fn(u32), fn(&InstructionTrace)>, &SimulationResult),
{
    run_elf_in_simulator_internal(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(u32)>,
        None,
        None::<fn(&InstructionTrace)>,
        vcd_path,
        0, // Zero latency for backward compatibility
        |sim, result| callback(sim, result),
    )
}

/// Run an ELF file with trace callback and mutable simulator access
///
/// This function supports instruction trace callbacks and provides mutable
/// simulator access before the run for configuration (e.g., enabling debug flags).
///
/// This delegates to the unified run_program function.
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `callback_before` - Function to configure simulator before running (e.g., enable debug flags)
/// * `vcd_path` - Optional path to VCD file for waveform dumping
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `fifo_callback` - Optional callback for FIFO TX data
/// * `trace_callback` - Optional callback for instruction traces
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_in_simulator_with_trace<F, T, C>(
    elf_path: &Path,
    max_cycles: u64,
    callback_before: C,
    vcd_path: Option<&str>,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    trace_callback: Option<T>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
    C: for<'a> FnOnce(&mut Simulator<'a, F, T>),
{
    run_program(
        max_cycles,
        print_inst_trace,
        false, // Don't print FSM state
        fifo_callback,
        trace_callback,
        vcd_path,
        0, // Zero latency for backward compatibility
        |sim| {
            // Execute callback_before to configure simulator (e.g., enable debug flags)
            callback_before(sim);

            // Load ELF into simulator memory
            let entry_point =
                load_elf(sim, elf_path).map_err(|e| format!("Error loading ELF: {}", e))?;

            log::info!("ELF loaded successfully");
            log::info!("Entry point: 0x{:08x}", entry_point);

            Ok(entry_point)
        },
        |_sim, _result| {
            // No post-execution callback needed for this function
        },
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
/// * `fifo_callback` - Optional callback for FIFO TX data
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
///     None::<fn(u32)>,
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
///     None::<fn(u32)>,
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
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
    P: for<'a> FnOnce(&mut Simulator<'a, F, T>) -> Result<u32, String>,
    C: for<'a> FnOnce(&mut Simulator<'a, F, T>, &SimulationResult),
{
    // Create system bus with internal DRAM
    let bus = SystemBus::new();

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .map_err(|e| format!("Error creating CPU runtime: {}", e))?;

    let mut sim = if let Some(vcd) = vcd_path {
        Simulator::new_with_vcd(
            &runtime,
            bus,
            print_inst_trace,
            print_fsm_state,
            fifo_callback,
            trace_callback,
            vcd,
            mem_latency_cycles,
        )?
    } else {
        Simulator::new(
            &runtime,
            bus,
            print_inst_trace,
            print_fsm_state,
            fifo_callback,
            trace_callback,
            mem_latency_cycles,
        )?
    };

    // Execute pre-execution callback to load program and get entry point
    let entry_point = prep_callback(&mut sim)?;

    log::info!("Program loaded, entry point: 0x{:08x}", entry_point);

    // Run simulation with entry point as boot PC
    sim.reset(entry_point);
    let result = sim.run(entry_point, max_cycles)?;

    // Execute post-execution callback with mutable simulator and result
    post_callback(&mut sim, &result);

    Ok(result)
}

// Disabled broken test module
// #[cfg(test)]
#[cfg(test)]
mod test_minimal;

#[cfg(test)]
mod test_byte_enable;

#[cfg(test)]
mod test_simple_byte_store;

#[cfg(test)]
mod test_alloc_only;

#[cfg(test)]
mod test_programmatic_memory;

#[cfg(test)]
mod test_rtl_verification;

#[cfg(test)]
mod test_memory_latency;

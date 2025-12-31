pub mod bus;
pub mod dram;
pub mod fifo;
pub mod memory;
pub mod packet_transport;
pub mod sim;

#[cfg(test)]
mod tests;

pub use riscv_core::trace::InstructionTrace;
pub use sim::SimulationResult;

use bus::SystemBus;
use dram::Dram;
use sim::Simulator;
use std::path::Path;

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
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        fifo_rx_data,
        None::<fn(&InstructionTrace)>,
    )
}

/// Run an ELF file on the simulated CPU with optional FIFO and trace callbacks
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
/// * `fifo_rx_data` - Optional string to write to the FIFO RX queue before running
/// * `trace_callback` - Optional callback invoked for each instruction executed (receives InstructionTrace)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_all_callbacks<F, T>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<T>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
{
    // Initialize DRAM and load ELF
    let mut dram = Dram::new();
    let entry_point = dram
        .load_elf(elf_path)
        .map_err(|e| format!("Error loading ELF: {}", e))?;

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Create system bus with DRAM and FIFO
    let bus = SystemBus::new(dram);

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .map_err(|e| format!("Error creating CPU runtime: {}", e))?;
    let mut sim = Simulator::new(
        &runtime,
        bus,
        entry_point,
        print_inst_trace,
        fifo_callback,
        trace_callback,
    )?;

    // Write data to RX FIFO if provided
    if let Some(data) = fifo_rx_data {
        sim.fifo_write_rx_string(data);
    }

    // Run simulation
    sim.run(max_cycles)
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
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(u32)>,
        None,
        trace_callback,
    )
}

/// Run an ELF file on the simulated CPU with an optional FIFO callback
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_callback<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
{
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        None,
        None::<fn(&InstructionTrace)>,
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
    run_elf_with_callback(elf_path, max_cycles, print_inst_trace, None::<fn(u32)>)
}
#[cfg(test)]
mod test_minimal;

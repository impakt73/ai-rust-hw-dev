pub mod bus;
pub mod dram;
pub mod fifo;
pub mod memory;
pub mod packet_transport;
pub mod sim;

#[cfg(test)]
mod tests;

pub use riscv_core::trace::InstructionTrace;
pub use sim::{SimulationResult, Simulator};

use bus::SystemBus;
use dram::Dram;
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
        None, // No VCD
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
/// * `vcd_path` - Optional path to VCD file for waveform dumping
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
    vcd_path: Option<&str>,
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

    let mut sim = if let Some(vcd) = vcd_path {
        Simulator::new_with_vcd(
            &runtime,
            bus,
            entry_point,
            print_inst_trace,
            fifo_callback,
            trace_callback,
            vcd,
        )?
    } else {
        Simulator::new(
            &runtime,
            bus,
            entry_point,
            print_inst_trace,
            fifo_callback,
            trace_callback,
        )?
    };

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
        None, // No VCD
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
        None, // No VCD
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
    run_elf_with_all_callbacks(
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
/// This eliminates the duplication between `run_elf_in_simulator` and `run_elf_in_simulator_mut`.
fn run_elf_in_simulator_internal<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<fn(&InstructionTrace)>,
    vcd_path: Option<&str>,
    callback: F,
) -> Result<SimulationResult, String>
where
    F: for<'a> FnOnce(&mut Simulator<'a, fn(u32), fn(&InstructionTrace)>, &SimulationResult),
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

    let mut sim = if let Some(vcd) = vcd_path {
        Simulator::new_with_vcd(
            &runtime,
            bus,
            entry_point,
            print_inst_trace,
            None,
            trace_callback,
            vcd,
        )?
    } else {
        Simulator::new(&runtime, bus, entry_point, print_inst_trace, None, trace_callback)?
    };

    // Write data to RX FIFO if provided
    if let Some(data) = fifo_rx_data {
        sim.fifo_write_rx_string(data);
    }

    // Run simulation
    let result = sim.run(max_cycles)?;

    // Execute callback with mutable simulator and result
    callback(&mut sim, &result);

    Ok(result)
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
        None,
        None,
        vcd_path,
        |sim, result| callback(sim, result),
    )
}

/// Run an ELF file in a simulator and execute a callback with mutable access to the simulator
///
/// This variant allows the callback to have mutable access to the simulator.
/// Useful for operations that might need to modify simulator state.
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `callback` - Function to execute with mutable simulator access after the run completes
/// * `vcd_path` - Optional path to VCD file for waveform dumping
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_in_simulator_mut<F>(
    elf_path: &Path,
    max_cycles: u64,
    callback: F,
    vcd_path: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: for<'a> FnMut(&mut Simulator<'a, fn(u32), fn(&InstructionTrace)>, &SimulationResult),
{
    run_elf_in_simulator_internal(elf_path, max_cycles, false, None, None, vcd_path, callback)
}

// Disabled broken test module
// #[cfg(test)]
// mod test_minimal;

#[cfg(test)]
mod test_byte_enable;

#[cfg(test)]
mod test_simple_byte_store;

#[cfg(test)]
mod test_alloc_only;

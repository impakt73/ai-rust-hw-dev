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

/// Type alias for a simulator with static lifetime and no callbacks
pub type StaticSimulator = Simulator<'static, fn(u32), fn(&InstructionTrace)>;

/// Type alias for the result of creating a simulator from an ELF file
pub type SimulatorWithRuntime = (StaticSimulator, Box<riscv_core::VerilatorRuntime>);

/// Create a simulator from an ELF file for manual control and memory inspection
///
/// This function provides lower-level access to the simulator, allowing you to:
/// - Run the simulation step-by-step
/// - Inspect memory after execution
/// - Dump memory regions or images
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
///
/// # Returns
/// * `Ok((Simulator, VerilatorRuntime))` - Simulator instance and its runtime (must be kept alive)
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::create_simulator_from_elf;
/// use std::path::Path;
///
/// let (mut sim, _runtime) = create_simulator_from_elf(Path::new("test.elf"))?;
/// let result = sim.run(1000)?;
/// let bytes: Vec<u8> = sim.dump_memory_region(0x80000000, 1024).collect();
/// # Ok::<(), String>(())
/// ```
pub fn create_simulator_from_elf(elf_path: &Path) -> Result<SimulatorWithRuntime, String> {
    // Initialize DRAM and load ELF
    let mut dram = Dram::new();
    let entry_point = dram
        .load_elf(elf_path)
        .map_err(|e| format!("Error loading ELF: {}", e))?;

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Create system bus with DRAM and FIFO
    let bus = SystemBus::new(dram);

    // Initialize CPU Simulator - box the runtime to ensure stable address
    let mut runtime = Box::new(
        riscv_core::create_cpu_runtime()
            .map_err(|e| format!("Error creating CPU runtime: {}", e))?,
    );

    // SAFETY: We're creating a 'static lifetime simulator by:
    // 1. Boxing the runtime to get a stable address
    // 2. Converting the box reference to a raw pointer, then to 'static ref
    // 3. The simulator and runtime are returned together as a tuple
    // 4. The caller MUST ensure the runtime outlives the simulator
    //    by keeping both values alive (typically by binding them to variables)
    let runtime_ref: &'static riscv_core::VerilatorRuntime =
        unsafe { &*(runtime.as_mut() as *mut _) };

    let sim = Simulator::new(runtime_ref, bus, entry_point, false, None, None)?;

    Ok((sim, runtime))
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

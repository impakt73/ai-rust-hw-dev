pub mod bus;
pub mod dram;
pub mod fifo;
pub mod memory;
pub mod sim;

pub use sim::SimulationResult;

use bus::SystemBus;
use dram::Dram;
use sim::Simulator;
use std::path::Path;

/// Run an ELF file on the simulated CPU with an optional FIFO callback
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO
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
    F: FnMut(u8),
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
    let mut sim = Simulator::new(&runtime, bus, entry_point, print_inst_trace)?;

    // Set FIFO callback if provided
    if let Some(callback) = fifo_callback {
        sim.set_fifo_callback(callback);
    }

    // Run simulation
    sim.run(max_cycles)
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
    run_elf_with_callback::<fn(u8)>(elf_path, max_cycles, print_inst_trace, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_comprehensive_elf() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the test ELF file
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/test.elf");

        // Run the simulation
        let result = run_elf(&elf_path, 500, false).expect("Simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42)"
        );

        println!(
            "✓ Comprehensive test ELF executed successfully in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_instruction_trace() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the test ELF file
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/test.elf");

        // Run the simulation with instruction trace enabled
        // Note: We can't easily capture and verify the trace output programmatically,
        // but we can verify that the simulation still completes successfully
        let result = run_elf(&elf_path, 500, true).expect("Simulation with trace should succeed");

        // Verify the program halted with the correct exit code
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) with trace enabled"
        );

        println!(
            "✓ Instruction trace test passed in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_rust_bare_metal_elf() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the Rust test program ELF from test_programs directory
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/rust_test.elf");

        // Run the simulation
        let result =
            run_elf(&elf_path, 500, false).expect("Rust bare metal simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from Rust bare metal program"
        );

        println!(
            "✓ Rust bare metal test ELF executed successfully in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_fifo_hello_world() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the hello_world test program ELF
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/hello_world.elf");

        // Collect FIFO data via callback
        use std::sync::{Arc, Mutex};
        let fifo_data = Arc::new(Mutex::new(Vec::new()));
        let fifo_data_clone = Arc::clone(&fifo_data);

        let callback = move |byte: u8| {
            fifo_data_clone.lock().unwrap().push(byte);
        };

        // Run the simulation with FIFO callback
        let result = run_elf_with_callback(&elf_path, 1000, false, Some(callback))
            .expect("FIFO hello world simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from hello_world program"
        );

        // Verify the FIFO data
        let received_data = fifo_data.lock().unwrap();
        let received_string =
            String::from_utf8(received_data.clone()).expect("FIFO data should be valid UTF-8");

        assert_eq!(
            received_string, "Hello World!",
            "Expected to receive 'Hello World!' via FIFO"
        );

        println!("✓ FIFO hello world test passed in {} cycles", result.cycles);
        println!("✓ Received via FIFO: '{}'", received_string);
    }
}

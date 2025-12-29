pub mod memory;
pub mod sim;

use memory::Memory;
use sim::{SimulationResult, Simulator};
use std::path::Path;

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
pub fn run_elf(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
) -> Result<SimulationResult, String> {
    // Initialize Memory and load ELF
    let mut mem = Memory::new();
    let entry_point = mem
        .load_elf(elf_path)
        .map_err(|e| format!("Error loading ELF: {}", e))?;

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .map_err(|e| format!("Error creating CPU runtime: {}", e))?;
    let mut sim = Simulator::new(&runtime, mem, entry_point, print_inst_trace)?;

    // Run simulation
    sim.run(max_cycles)
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
        // Note: We can't easily verify the trace output in a unit test,
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
}

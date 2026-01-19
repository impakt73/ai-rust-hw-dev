use cpu_sim::InteractiveSimulator;
use std::path::PathBuf;

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}

#[test]
fn test_interactive_simulator_load_elf() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("simple_test.elf");

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Test that we can load an ELF file
    let result = sim.load_elf(&elf_path);
    assert!(result.is_ok(), "Should be able to load ELF file");
}

#[test]
fn test_interactive_simulator_step_without_elf() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Test that stepping without loading an ELF returns an error
    let result = sim.step_instruction();
    assert!(
        result.is_err(),
        "Should return error when stepping without loaded ELF"
    );
    assert!(
        result.unwrap_err().contains("No ELF file loaded"),
        "Error message should indicate no ELF loaded"
    );
}

#[test]
fn test_interactive_simulator_simple_program() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("simple_test.elf");

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_elf(&elf_path).expect("Failed to load ELF");

    // Step through instructions until we hit tohost or max iterations
    let max_instructions = 100;
    let mut instruction_count = 0;
    let mut tohost_value = None;

    for _ in 0..max_instructions {
        match sim.step_instruction() {
            Ok(result) => {
                instruction_count += 1;
                if let Some(value) = result.tohost_value {
                    tohost_value = Some(value);
                    break;
                }
            }
            Err(e) => {
                panic!("Unexpected error during execution: {}", e);
            }
        }
    }

    println!("Executed {} instructions", instruction_count);
    assert!(
        tohost_value.is_some(),
        "Program should terminate via tohost"
    );
    assert_eq!(
        tohost_value,
        Some(42),
        "Program should exit with tohost value 42"
    );
}

#[test]
fn test_interactive_simulator_multiple_programs() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test that we can load multiple programs sequentially
    let elf_path = test_program_path("simple_test.elf");

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Load and run first program
    sim.load_elf(&elf_path).expect("Failed to load first ELF");
    let mut tohost_1 = None;
    for _ in 0..100 {
        match sim.step_instruction() {
            Ok(result) => {
                if let Some(value) = result.tohost_value {
                    tohost_1 = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Error in first program: {}", e),
        }
    }
    assert_eq!(tohost_1, Some(42), "First program should exit with 42");

    // Load and run second program
    sim.load_elf(&elf_path).expect("Failed to load second ELF");
    let mut tohost_2 = None;
    for _ in 0..100 {
        match sim.step_instruction() {
            Ok(result) => {
                if let Some(value) = result.tohost_value {
                    tohost_2 = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Error in second program: {}", e),
        }
    }
    assert_eq!(tohost_2, Some(42), "Second program should exit with 42");
}

#[test]
fn test_interactive_simulator_step_result() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("simple_test.elf");

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_elf(&elf_path).expect("Failed to load ELF");

    // Step once and verify SimulationStepResult structure
    let result = sim.step_instruction().expect("First step should succeed");

    // Check that tohost is None initially (program hasn't terminated)
    assert_eq!(
        result.tohost_value, None,
        "First instruction should not terminate"
    );

    // Check that elapsed time is reasonable (should be non-zero but small)
    assert!(
        result.elapsed_cpu_time_us > 0,
        "Elapsed time should be greater than 0"
    );
    assert!(
        result.elapsed_cpu_time_us < 1000000,
        "Single instruction should take less than 1 second"
    );
}

#[test]
fn test_interactive_simulator_load_nonexistent_file() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    let bad_path = PathBuf::from("nonexistent_file.elf");
    let result = sim.load_elf(&bad_path);

    assert!(
        result.is_err(),
        "Loading nonexistent file should return error"
    );
}

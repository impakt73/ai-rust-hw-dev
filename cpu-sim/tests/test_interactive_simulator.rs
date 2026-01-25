use cpu_sim::InteractiveSimulator;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn test_program_path(name: &str) -> PathBuf {
    sim_tests::test_program_path(name)
        .unwrap_or_else(|e| panic!("Failed to find test program {}: {}", name, e))
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

#[test]
fn test_interactive_simulator_register_video_device() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_video_pattern.elf");

    // Storage for captured frames
    type CapturedFrames = Rc<RefCell<Vec<(Vec<u8>, cpu_sim::VideoConfig)>>>;
    let captured_frames: CapturedFrames = Rc::new(RefCell::new(Vec::new()));

    let frames_clone = captured_frames.clone();

    // Create callback that captures frame data
    let present_callback = move |data: &[u8], config: &cpu_sim::VideoConfig| {
        frames_clone.borrow_mut().push((data.to_vec(), *config));
        log::info!(
            "Frame captured: {}x{} {:?}",
            config.width,
            config.height,
            config.format
        );
    };

    // Create simulator and register video device
    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Register Video device at VIDEO_BASE with callback
    let video = Box::new(cpu_sim::Video::with_fps(10000, Some(present_callback)));
    let register_result = sim.register_device(cpu_sim::VIDEO_BASE, video);
    assert!(
        register_result.is_ok(),
        "Should be able to register Video device: {:?}",
        register_result
    );

    // Load ELF and run
    sim.load_elf(&elf_path).expect("Failed to load ELF");

    // Step through instructions
    let max_instructions = 1000000; // Increase to 1M to match run_elf test
    let mut tohost_value = None;
    let mut instruction_count = 0;

    for _ in 0..max_instructions {
        match sim.step_instruction() {
            Ok(result) => {
                instruction_count += 1;
                if instruction_count % 100000 == 0 {
                    log::info!("Executed {} instructions", instruction_count);
                }
                if let Some(value) = result.tohost_value {
                    tohost_value = Some(value);
                    log::info!(
                        "Program terminated after {} instructions with tohost={}",
                        instruction_count,
                        value
                    );
                    break;
                }
            }
            Err(e) => {
                panic!(
                    "Unexpected error during execution at instruction {}: {}",
                    instruction_count, e
                );
            }
        }
    }

    log::info!(
        "Test completed after {} instructions, tohost={:?}",
        instruction_count,
        tohost_value
    );

    // Verify program completed successfully
    assert_eq!(
        tohost_value,
        Some(42),
        "Program should exit with tohost value 42 (executed {} instructions)",
        instruction_count
    );

    // Verify we captured at least one frame
    let frames = captured_frames.borrow();
    assert!(
        !frames.is_empty(),
        "Should have captured at least one video frame"
    );

    log::info!("Captured {} frames", frames.len());
}

#[test]
fn test_interactive_simulator_register_audio_device() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_audio_pattern.elf");

    // Storage for captured samples
    let captured_samples: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));

    let samples_clone = captured_samples.clone();

    // Create callback that captures sample data
    let sample_callback = move |samples: &[i16]| {
        samples_clone.borrow_mut().push(samples.to_vec());
    };

    // Create simulator and register audio device
    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Register Audio device at AUDIO_BASE with callback
    let audio: Box<dyn cpu_sim::BusDevice> = Box::new(cpu_sim::Audio::new(
        Some(sample_callback),
        None::<fn(&cpu_sim::AudioConfig)>,
    ));
    let register_result = sim.register_device(cpu_sim::AUDIO_BASE, audio);
    assert!(
        register_result.is_ok(),
        "Should be able to register Audio device: {:?}",
        register_result
    );

    // Load ELF and run
    sim.load_elf(&elf_path).expect("Failed to load ELF");

    // Step through instructions
    let max_instructions = 1000000; // Increase to 1M
    let mut tohost_value = None;

    for _ in 0..max_instructions {
        match sim.step_instruction() {
            Ok(result) => {
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

    // Verify program completed successfully
    assert_eq!(
        tohost_value,
        Some(42),
        "Program should exit with tohost value 42"
    );

    // Verify we captured audio samples
    let samples = captured_samples.borrow();
    assert!(
        !samples.is_empty(),
        "Should have captured at least one audio sample"
    );

    log::info!("Captured {} sample batches", samples.len());
}

#[test]
fn test_interactive_simulator_register_device_address_conflict() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Try to register a device at the FIFO base address (should conflict)
    let video: Box<dyn cpu_sim::BusDevice> = Box::new(cpu_sim::Video::new(
        None::<fn(&[u8], &cpu_sim::VideoConfig)>,
    ));
    let register_result = sim.register_device(cpu_sim::FIFO_BASE, video);

    assert!(
        register_result.is_err(),
        "Should not be able to register device at FIFO_BASE (conflicts with internal FIFO)"
    );

    // Verify error message mentions the conflict
    let err_msg = register_result.unwrap_err();
    assert!(
        err_msg.contains("overlap") || err_msg.contains("Overlap") || err_msg.contains("conflict"),
        "Error message should mention overlap/conflict: {}",
        err_msg
    );
}

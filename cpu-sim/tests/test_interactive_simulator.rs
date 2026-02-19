mod common;

use bus_shared::{
    Audio, AudioConfig, BusDevice, Video, VideoConfig, AUDIO_BASE, FIFO_BASE, VIDEO_BASE,
};
use common::create_simple_exit_program;
use cpu_sim::{AccessSize, BusRequest, InteractiveSimulator};
use riscv_shared::SUCCESS_CODE;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[test]
fn test_interactive_simulator_load_elf() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = sim_tests::test_program_path("simple_test").expect("Failed to find simple_test");

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Test that we can load an ELF file
    let result = sim.load_elf(&elf_path);
    assert!(result.is_ok(), "Should be able to load ELF file");
}

#[test]
fn test_interactive_simulator_step_without_elf() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Test that stepping a cycle without loading an ELF is allowed
    let result = sim.step_cycle();
    assert!(
        result.is_ok(),
        "Should allow stepping cycles without loaded ELF"
    );
}

#[test]
fn test_interactive_simulator_step_cycle() {
    let _ = env_logger::builder().is_test(true).try_init();

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

    let max_cycles = 1000;
    let mut instruction_completed = false;

    for _ in 0..max_cycles {
        match sim.step_cycle() {
            Ok(result) if result.instruction_completed => {
                instruction_completed = true;
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("Unexpected error during cycle stepping: {}", e),
        }
    }

    assert!(
        instruction_completed,
        "Instruction should complete within {} cycles",
        max_cycles
    );

    let max_cycles_for_tohost = 10_000;
    let mut tohost_value = None;

    for _ in 0..max_cycles_for_tohost {
        match sim.step_cycle() {
            Ok(result) => {
                if let Some(value) = result.tohost_value {
                    tohost_value = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Unexpected error during cycle stepping: {}", e),
        }
    }

    assert_eq!(
        tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with tohost value {SUCCESS_CODE}"
    );
}

#[test]
fn test_interactive_simulator_simple_program() {
    let _ = env_logger::builder().is_test(true).try_init();

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

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
    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Load and run first program
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load first program");
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
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load second program");
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

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

    // Step once and verify SimulationStepInstructionResult structure
    let result = sim.step_instruction().expect("First step should succeed");

    // Check that tohost is None initially (program hasn't terminated)
    assert_eq!(
        result.tohost_value, None,
        "First instruction should not terminate"
    );

    // Check that cycles executed is reasonable (should be non-zero)
    assert!(
        result.cycles_executed > 0,
        "Cycles executed should be greater than 0"
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
fn test_interactive_simulator_rejects_overflowing_host_request_range() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    let result = sim.send_bus_request(BusRequest::read(0xFFFF_FFFE, AccessSize::Word));

    assert!(
        result.is_err(),
        "Overflowing request range should be rejected"
    );
}

#[test]
fn test_interactive_simulator_register_video_device() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = sim_tests::test_program_path("test_video_pattern")
        .expect("Failed to find test_video_pattern");

    // Storage for captured frames
    type CapturedFrames = Arc<Mutex<Vec<(Vec<u8>, VideoConfig)>>>;
    let captured_frames: CapturedFrames = Arc::new(Mutex::new(Vec::new()));

    let frames_clone = captured_frames.clone();

    // Create callback that captures frame data
    let present_callback = move |data: &[u8], config: &VideoConfig| {
        frames_clone
            .lock()
            .expect("frames lock poisoned")
            .push((data.to_vec(), *config));
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
    let video = Box::new(Video::with_fps(10000, Some(present_callback)));
    let register_result = sim.register_device(VIDEO_BASE, video);
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
    let frames = captured_frames.lock().expect("frames lock poisoned");
    assert!(
        !frames.is_empty(),
        "Should have captured at least one video frame"
    );

    log::info!("Captured {} frames", frames.len());
}

#[test]
fn test_interactive_simulator_register_audio_device() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = sim_tests::test_program_path("test_audio_pattern")
        .expect("Failed to find test_audio_pattern");

    // Storage for captured samples
    let captured_samples: Arc<Mutex<Vec<Vec<i16>>>> = Arc::new(Mutex::new(Vec::new()));

    let samples_clone = captured_samples.clone();

    // Create callback that captures sample data
    let sample_callback = move |samples: &[i16]| {
        samples_clone
            .lock()
            .expect("samples lock poisoned")
            .push(samples.to_vec());
    };

    // Create simulator and register audio device
    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Register Audio device at AUDIO_BASE with callback
    let audio: Box<dyn BusDevice> =
        Box::new(Audio::new(Some(sample_callback), None::<fn(&AudioConfig)>));
    let register_result = sim.register_device(AUDIO_BASE, audio);
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
    let samples = captured_samples.lock().expect("samples lock poisoned");
    assert!(
        !samples.is_empty(),
        "Should have captured at least one audio sample"
    );

    log::info!("Captured {} sample batches", samples.len());
}

#[test]
fn test_interactive_simulator_can_register_device_at_fifo_base() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Registering at FIFO_BASE should succeed because FIFO is now an external device.
    let video: Box<dyn BusDevice> = Box::new(Video::new(None::<fn(&[u8], &VideoConfig)>));
    let register_result = sim.register_device(FIFO_BASE, video);

    assert!(register_result.is_ok());
}

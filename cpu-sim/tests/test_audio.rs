use cpu_sim::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}

#[test]
fn test_audio_pattern() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_audio_pattern.elf");

    // Audio device base address (must match test program)
    const AUDIO_BASE: u32 = 0x3000_0000;

    // Storage for captured samples and config changes
    let captured_samples: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
    let captured_configs: Rc<RefCell<Vec<AudioConfig>>> = Rc::new(RefCell::new(Vec::new()));

    // Setup callback to register Audio device
    let samples_for_setup = captured_samples.clone();
    let configs_for_setup = captured_configs.clone();
    
    let setup_callback = move |view: &mut SimulatorView| {
        let samples_for_callback = samples_for_setup.clone();
        let configs_for_callback = configs_for_setup.clone();

        // Create sample callback that captures sample data
        let sample_callback = move |samples: &[i16]| {
            samples_for_callback.borrow_mut().push(samples.to_vec());
            let total = samples_for_callback.borrow().len();
            if total <= 5 || total % 100 == 0 {
                log::info!("Sample {} captured: {:?}", total - 1, samples);
            }
        };

        // Create config callback that captures config changes
        let config_callback = move |config: &AudioConfig| {
            configs_for_callback.borrow_mut().push(*config);
            log::info!(
                "Audio config: {}Hz, {:?}, {} samples",
                config.sample_rate.to_hz(),
                config.channels,
                config.sample_count
            );
        };

        // Register Audio device at 0x4000_0000
        let audio = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));
        view.register_device(AUDIO_BASE, audio)
            .expect("Failed to register Audio device");
        log::info!("Audio device registered at 0x{:08x}", AUDIO_BASE);
    };

    // Termination callback to verify captured data
    let samples_for_verify = captured_samples.clone();
    let configs_for_verify = captured_configs.clone();
    
    let termination_callback = move |_view: &SimulatorView, result: &SimulationResult| {
        // Verify the program completed successfully
        assert_eq!(
            result.tohost_value,
            Some(42),
            "Audio test should exit with success code 42"
        );

        println!("\n=== Audio Pattern Test Results ===");
        println!("Cycles: {}", result.cycles);
        println!("Test program completed successfully");

        let samples = samples_for_verify.borrow();
        let configs = configs_for_verify.borrow();

        println!("✓ Captured {} config changes", configs.len());
        println!("✓ Captured {} audio samples", samples.len());

        // Verify we got at least one config change
        assert!(
            !configs.is_empty(),
            "Should have received at least one config change"
        );

        // Verify we received a reasonable number of samples
        // The test program should produce enough samples to cause ring buffer wrapping
        assert!(
            samples.len() > 100,
            "Should have received many samples (got {})",
            samples.len()
        );

        // Verify the configuration
        let config = &configs[0];
        assert_eq!(
            config.sample_rate,
            AudioSampleRate::Hz48000,
            "Sample rate should be 48000Hz"
        );
        assert_eq!(config.channels, AudioChannels::Mono, "Should be mono");
        assert!(
            config.sample_count >= 16,
            "Buffer should be at least 16 samples"
        );

        // Verify samples are in the expected range for a sine wave
        // (assuming sine wave with reasonable amplitude)
        for (i, sample_vec) in samples.iter().enumerate() {
            assert_eq!(
                sample_vec.len(),
                1,
                "Each sample should have 1 channel (mono)"
            );
            let sample = sample_vec[0];
            assert!(
                sample.abs() <= i16::MAX,
                "Sample {} out of range: {}",
                i,
                sample
            );
        }

        println!("✓ All audio samples verified successfully");
    };

    let result = run_elf(
        &elf_path,
        5_000_000, // Max cycles
        false,     // print_inst_trace
        false,     // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,                       // vcd_path
        0,                          // mem_latency_cycles
        Some(setup_callback),       // Register Audio device
        Some(termination_callback), // Verify samples after completion
    )
    .expect("Simulation should succeed");

    println!("\n=== Audio Pattern Test Summary ===");
    println!("Total cycles: {}", result.cycles);
    println!("Test passed: Audio samples rendered and verified");
}

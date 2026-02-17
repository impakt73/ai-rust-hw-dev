use bus_shared::{Audio, AudioConfig, AUDIO_BASE};
use cpu_sim::{run_elf, InstructionTrace, SimulationResult, SimulatorView};
use std::sync::{Arc, Mutex};

mod audio_test_common;
use audio_test_common::generate_stereo_sample;

#[test]
fn test_audio_pattern() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = sim_tests::test_program_path("test_audio_pattern")
        .expect("Failed to find test_audio_pattern");

    // Storage for captured samples and config changes
    let captured_samples: Arc<Mutex<Vec<Vec<i16>>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_configs: Arc<Mutex<Vec<AudioConfig>>> = Arc::new(Mutex::new(Vec::new()));

    // Setup callback to register Audio device
    let samples_for_setup = captured_samples.clone();
    let configs_for_setup = captured_configs.clone();

    let setup_callback = move |view: &mut SimulatorView| {
        let samples_for_callback = samples_for_setup.clone();
        let configs_for_callback = configs_for_setup.clone();

        // Create sample callback that captures sample data
        // With DMA, we receive batches of samples in each callback
        let sample_callback = move |samples: &[i16]| {
            let mut batches = samples_for_callback.lock().expect("samples lock poisoned");
            batches.push(samples.to_vec());
            let batch_count = batches.len();
            let total_samples: usize = batches
                .iter()
                .map(|v| v.len() / 2) // Divide by 2 for stereo (2 channels per sample)
                .sum();
            log::info!(
                "DMA batch {} captured: {} channel values ({} stereo samples), total samples so far: {}",
                batch_count - 1,
                samples.len(),
                samples.len() / 2,
                total_samples
            );
        };

        // Create config callback that captures config changes
        let config_callback = move |config: &AudioConfig| {
            configs_for_callback
                .lock()
                .expect("configs lock poisoned")
                .push(*config);
            log::info!(
                "Audio config: {}Hz, {:?}, {} samples",
                config.sample_rate.to_hz(),
                config.channels,
                config.sample_count
            );
        };

        // Register Audio device at AUDIO_BASE
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

        let sample_batches = samples_for_verify.lock().expect("samples lock poisoned");
        let configs = configs_for_verify.lock().expect("configs lock poisoned");

        println!("✓ Captured {} config changes", configs.len());
        println!("✓ Captured {} DMA batches", sample_batches.len());

        // Verify we got the expected number of config changes
        // Test program writes AUDIO_CONFIG once initially, then once per batch
        // With 500 samples total and 64 samples per batch:
        // - 7 batches of 64 = 448 samples
        // - 1 batch of 52 = 52 samples
        // Total: 8 batches
        assert_eq!(
            configs.len(),
            9,
            "Should have received exactly 9 config changes (1 initial + 8 batch updates)"
        );

        // Flatten all batches into a single list of samples
        let mut all_samples = Vec::new();
        for batch in sample_batches.iter() {
            all_samples.extend_from_slice(batch);
        }

        // Convert channel values to stereo samples (groups of 2)
        let total_stereo_samples = all_samples.len() / 2;
        println!("✓ Total stereo samples captured: {}", total_stereo_samples);

        // Verify we received exactly 500 stereo samples (as per test program)
        assert_eq!(
            total_stereo_samples, 500,
            "Should have received exactly 500 stereo samples"
        );

        // Verify samples match expected sine wave pattern exactly
        const FREQUENCY_DIV: u32 = 4; // Must match test program
        for i in 0..total_stereo_samples {
            let left_idx = i * 2;
            let right_idx = i * 2 + 1;

            // Generate expected values using same algorithm as test program
            let (expected_left, expected_right) = generate_stereo_sample(i as u32, FREQUENCY_DIV);

            assert_eq!(
                all_samples[left_idx], expected_left,
                "Sample {} left channel mismatch: expected {}, got {}",
                i, expected_left, all_samples[left_idx]
            );
            assert_eq!(
                all_samples[right_idx], expected_right,
                "Sample {} right channel mismatch: expected {}, got {}",
                i, expected_right, all_samples[right_idx]
            );
        }

        println!("✓ All audio samples verified successfully");
    };

    let result = run_elf(
        &elf_path,
        2_000_000, // High limit for audio sample generation (increased due to serialized bus protocol)
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

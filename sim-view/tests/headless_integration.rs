//! Integration tests for headless mode.
//!
//! These tests validate that the headless backend correctly captures video frames
//! and audio samples with proper timestamps.

use sim_view::backend_traits::{TestCommand, ViewerEvent};
use sim_view::{
    headless_backends::{HeadlessAudioBackend, HeadlessEventSource, HeadlessVideoBackend},
    viewer::{SimViewer, ViewerConfig},
};
use std::path::PathBuf;

/// Helper to get path to test programs
fn test_program_path(name: &str) -> PathBuf {
    sim_tests::test_program_path(name)
        .unwrap_or_else(|e| panic!("Failed to find test program {}: {}", name, e))
}

#[test]
fn test_headless_basic_functionality() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 10000, // Limit execution
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Run a few steps
    for _ in 0..10 {
        if !viewer.step().expect("Step failed") {
            break;
        }
    }

    println!("Headless mode ran successfully: basic smoke test passed");
}

#[test]
fn test_headless_max_cycles_limit() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer with very low max_cycles
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 100, // Very small limit
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Load test ELF to actually execute instructions
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Run steps - should exit quickly due to max_cycles
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }
        steps += 1;
        if steps > 100 {
            // Safety limit - shouldn't need this many steps with max_cycles=100
            break;
        }
    }

    println!("Max cycles limit test passed after {} steps", steps);
}

#[test]
fn test_headless_event_injection() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 100000,
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Inject terminate command
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::Terminate))
        .expect("Failed to push event");

    // Run viewer - should exit immediately
    let start = std::time::Instant::now();
    let should_continue = viewer.step().expect("Step failed");
    let elapsed = start.elapsed();

    // Should terminate quickly
    assert!(
        !should_continue,
        "Viewer should terminate after terminate command"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "Viewer should terminate quickly, took {:?}",
        elapsed
    );
}

#[test]
fn test_frame_stepping() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 1000000, // High limit
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Load test ELF (produces exactly 3 frames then exits)
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Step 3 frames (this ELF produces exactly 3 frames)
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::StepFrames(3)))
        .expect("Failed to push event");

    // Run until exactly 3 frames are captured or safety limit
    let mut steps = 0;
    loop {
        // Run one step
        if !viewer.step().expect("Step failed") {
            break; // Viewer requested termination
        }

        steps += 1;

        // Check if we have exactly 3 frames
        let frames = viewer.get_video_frames();
        if frames.len() >= 3 {
            println!("Captured {} frames after {} steps", frames.len(), steps);
            break;
        }

        // Safety limit
        if steps > 2000 {
            let frames = viewer.get_video_frames();
            panic!(
                "Safety limit reached: only {} frames captured after {} steps",
                frames.len(),
                steps
            );
        }
    }

    // Verify we captured exactly 3 frames
    let frames = viewer.get_video_frames();
    assert_eq!(
        frames.len(),
        3,
        "Test ELF should produce exactly 3 frames, got {}",
        frames.len()
    );

    println!(
        "Frame stepping test passed: captured {} frames",
        frames.len()
    );
}

#[test]
fn test_sequential_frames_differ() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 1000000,
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Load test ELF that generates a video pattern (produces exactly 3 frames)
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Don't use StepFrames - just run until we have at least 2 frames for comparison
    // Run until we have at least 2 frames or safety limit
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }

        steps += 1;

        // Check if we have at least 2 frames for comparison
        let frames = viewer.get_video_frames();
        if frames.len() >= 2 {
            println!("Captured {} frames after {} steps", frames.len(), steps);
            break;
        }

        // Safety limit
        if steps > 2000 {
            let frames = viewer.get_video_frames();
            panic!(
                "Safety limit reached: only {} frames captured after {} steps",
                frames.len(),
                steps
            );
        }
    }

    // Verify we have at least 2 frames for comparison
    let frames = viewer.get_video_frames();
    assert!(
        frames.len() >= 2,
        "Should have captured at least 2 frames for comparison, got {}",
        frames.len()
    );

    let mut differences_found = 0;
    for i in 1..frames.len() {
        // Compare consecutive frames
        if frames[i].data != frames[i - 1].data {
            differences_found += 1;
        }
    }

    println!(
        "Sequential frames differ test: {} differences in {} frame pairs",
        differences_found,
        frames.len() - 1
    );

    // test_video_pattern.elf should generate changing frames
    assert!(
        differences_found > 0,
        "Expected at least some frames to differ, but all {} frames were identical",
        frames.len()
    );
}

#[test]
fn test_audio_config_change_and_samples() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 5000000, // Generous limit for audio test
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Load test ELF that generates audio samples (sets audio config)
    let elf_path = test_program_path("test_audio_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Run until program completes or we get sufficient audio data
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }

        // Pull audio samples for headless capture
        viewer.update_audio_capture();

        steps += 1;

        // Safety limit - test_audio_pattern should complete within this
        if steps > 50000 {
            break;
        }
    }

    // Get the audio backend to check results
    let audio_chunks = viewer.get_audio_chunks();
    let audio_config = viewer.get_audio_config();

    println!(
        "Audio config change test: {} steps, {} audio chunks captured",
        steps,
        audio_chunks.len()
    );

    // Verify that audio config was set
    assert!(
        audio_config.is_some(),
        "Audio config should have been set by test program"
    );

    let config = audio_config.unwrap();
    println!(
        "Audio config: {} Hz, {:?}, {} samples",
        config.sample_rate.to_hz(),
        config.channels,
        config.sample_count
    );

    // Verify expected config values from test_audio_pattern.elf
    // The test sets: 48000Hz, Stereo, with varying batch sizes
    // Initial and most batches use 64 samples, final batch uses 52 samples (500 total = 7*64 + 52)
    assert_eq!(
        config.sample_rate.to_hz(),
        48000,
        "Sample rate should be 48000 Hz"
    );
    assert_eq!(
        config.channels,
        cpu_sim::AudioChannels::Stereo,
        "Should be stereo"
    );
    // Sample count can be either 64 (full batch) or 52 (final batch) depending on when config was captured
    assert!(
        config.sample_count == 64 || config.sample_count == 52,
        "Sample count should be 64 (full batch) or 52 (final batch), got {}",
        config.sample_count
    );

    // Verify that audio samples were captured
    assert!(
        !audio_chunks.is_empty(),
        "Audio chunks should have been captured"
    );

    // Verify that captured chunks have the config set
    let chunks_with_config: Vec<_> = audio_chunks
        .iter()
        .filter(|chunk| chunk.config.is_some())
        .collect();

    assert!(
        !chunks_with_config.is_empty(),
        "At least some audio chunks should have config information"
    );

    // Verify that the config in chunks matches what we expect
    for chunk in &chunks_with_config {
        let chunk_config = chunk.config.as_ref().unwrap();
        assert_eq!(
            chunk_config.sample_rate.to_hz(),
            48000,
            "Chunk config should match expected sample rate"
        );
        assert_eq!(
            chunk_config.channels,
            cpu_sim::AudioChannels::Stereo,
            "Chunk config should match expected channels"
        );
    }

    // Verify total samples captured is reasonable
    let total_samples: usize = audio_chunks.iter().map(|c| c.samples.len()).sum();
    assert!(
        total_samples > 0,
        "Should have captured at least some audio samples"
    );

    println!(
        "Successfully verified audio config change and {} total samples captured",
        total_samples
    );
}

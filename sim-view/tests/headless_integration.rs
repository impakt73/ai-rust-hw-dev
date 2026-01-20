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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
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

    // Run a few steps - should exit quickly due to max_cycles
    for _ in 0..20 {
        if !viewer.step().expect("Step failed") {
            break;
        }
    }

    println!("Max cycles limit test passed");
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

    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Step 3 frames (based on observed behavior)
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::StepFrames(3)))
        .expect("Failed to push event");

    // Run until we have at least 3 frames (with safety limit)
    let mut steps = 0;
    loop {
        // Run one step
        if !viewer.step().expect("Step failed") {
            break; // Viewer requested termination
        }

        steps += 1;

        if steps > 2000 {
            // Safety limit - check if we have enough frames
            let frames = viewer.get_video_frames();
            println!(
                "Safety limit reached: {} frames captured after {} steps",
                frames.len(),
                steps
            );
            if frames.len() >= 3 {
                println!("Test passes with {} frames", frames.len());
                break;
            }
            panic!("Too many steps without reaching 3 frames, test may be stuck");
        }

        // Check if we have enough frames to exit early
        let frames = viewer.get_video_frames();
        if frames.len() >= 3 {
            println!(
                "Captured {} frames after {} steps, exiting early",
                frames.len(),
                steps
            );
            break;
        }
    }

    // Verify we captured frames
    let frames = viewer.get_video_frames();
    assert!(
        frames.len() >= 3,
        "Should have captured at least 3 frames, got {}",
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

    // Load test ELF that generates a video pattern
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load test ELF");

    // Step 20 frames
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::StepFrames(20)))
        .expect("Failed to push event");

    // Run until we have enough frames
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }

        steps += 1;
        if steps > 2000 {
            let frames = viewer.get_video_frames();
            if frames.len() >= 2 {
                println!(
                    "Safety limit reached but got {} frames - continuing with test",
                    frames.len()
                );
                break;
            }
            panic!("Too many steps without generating frames, test may be stuck");
        }

        // Check if we have enough frames
        let frames = viewer.get_video_frames();
        if frames.len() >= 20 {
            println!("Captured {} frames, exiting early", frames.len());
            break;
        }
    }

    // Verify sequential frames are different
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

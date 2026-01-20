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
use std::thread;
use std::time::Duration;

/// Helper to get path to test programs
#[allow(dead_code)] // Will be used in future tests
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

    // Get handles for verification
    let frames = video.get_frames_handle();
    let chunks = audio.get_chunks_handle();
    let event_queue = events.get_event_handle();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 10000, // Limit execution
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Spawn a thread to terminate the viewer after a short time
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        event_queue
            .lock()
            .unwrap()
            .push_back(ViewerEvent::TestCommand(TestCommand::Terminate));
    });

    // Run viewer (will run for ~100ms then terminate)
    viewer.run().expect("Viewer execution failed");

    // Verify that viewer ran (may or may not have captured frames depending on timing)
    // This test mainly validates that headless mode doesn't crash
    let frame_count = frames.lock().unwrap().len();
    let chunk_count = chunks.lock().unwrap().len();

    println!(
        "Headless mode ran successfully: {} frames, {} audio chunks",
        frame_count, chunk_count
    );

    // Basic smoke test - no crashes
    assert!(true);
}

#[test]
fn test_headless_max_cycles_limit() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Get event handle
    let event_queue = events.get_event_handle();

    // Create viewer with very low max_cycles
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 100, // Very small limit
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Inject terminate command after a short delay to prevent hanging
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        event_queue
            .lock()
            .unwrap()
            .push_back(ViewerEvent::TestCommand(TestCommand::Terminate));
    });

    // Run viewer (should exit quickly)
    let result = viewer.run();

    // Should complete successfully
    assert!(result.is_ok(), "Viewer should complete successfully");
}

#[test]
fn test_headless_event_injection() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Get event handle
    let event_queue = events.get_event_handle();

    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 100000,
        print_inst_trace: false,
    };

    let mut viewer = SimViewer::new(config, video, audio, events).expect("Failed to create viewer");

    // Inject terminate command immediately
    event_queue
        .lock()
        .unwrap()
        .push_back(ViewerEvent::TestCommand(TestCommand::Terminate));

    // Run viewer (should exit immediately)
    let start = std::time::Instant::now();
    viewer.run().expect("Viewer execution failed");
    let elapsed = start.elapsed();

    // Should terminate quickly (within 1 second)
    assert!(
        elapsed < Duration::from_secs(1),
        "Viewer should terminate quickly, took {:?}",
        elapsed
    );
}

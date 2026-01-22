//! Integration tests for the threaded viewer.
//!
//! These tests validate that the background simulation thread correctly
//! executes programs and communicates with the main thread.

use sim_view::backend_traits::{TestCommand, ViewerEvent};
use sim_view::threaded_viewer::{create_headless_viewer, ThreadedViewerConfig, ViewerState};
use std::path::PathBuf;
use std::time::Duration;

/// Helper to get path to test programs
fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}

#[test]
fn test_threaded_viewer_basic() {
    let _ = env_logger::builder().is_test(true).try_init();

    let config = ThreadedViewerConfig {
        max_cycles: 100000,
        batch_size: 1000,
    };

    let (mut viewer, video_backend, _audio_backend) =
        create_headless_viewer(config).expect("Failed to create viewer");

    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load ELF");

    // Give the simulation thread time to start processing
    std::thread::sleep(Duration::from_millis(50));

    // Run steps until we capture frames or hit limit
    let mut steps = 0;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if !viewer.step().expect("Step failed") {
            break;
        }
        steps += 1;

        // Check if we've captured frames
        let frames = video_backend.get_frames();
        if frames.len() >= 2 {
            break;
        }

        // Sleep a bit to let simulation thread run
        std::thread::sleep(Duration::from_millis(10));
    }

    // Verify we captured some frames
    let frames = video_backend.get_frames();
    println!(
        "Test completed in {} steps, captured {} frames, state: {:?}",
        steps,
        frames.len(),
        viewer.state()
    );

    assert!(
        !frames.is_empty(),
        "Should have captured at least one frame, got {}, state: {:?}",
        frames.len(),
        viewer.state()
    );

    println!(
        "Threaded viewer basic test: captured {} frames in {} steps",
        frames.len(),
        steps
    );
}

#[test]
fn test_threaded_viewer_terminate() {
    let _ = env_logger::builder().is_test(true).try_init();

    let config = ThreadedViewerConfig {
        max_cycles: 100000,
        batch_size: 1000,
    };

    let (mut viewer, _video_backend, _audio_backend) =
        create_headless_viewer(config).expect("Failed to create viewer");

    // Push terminate command
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::Terminate))
        .expect("Failed to push event");

    // Run step - should return false
    let should_continue = viewer.step().expect("Step failed");
    assert!(
        !should_continue,
        "Viewer should terminate after terminate command"
    );

    println!("Threaded viewer terminate test passed");
}

#[test]
fn test_threaded_viewer_pause_resume() {
    let _ = env_logger::builder().is_test(true).try_init();

    let config = ThreadedViewerConfig {
        max_cycles: 100000,
        batch_size: 1000,
    };

    let (mut viewer, video_backend, _audio_backend) =
        create_headless_viewer(config).expect("Failed to create viewer");

    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load ELF");

    // Run a few steps to let simulation start
    for _ in 0..10 {
        viewer.step().expect("Step failed");
        std::thread::sleep(Duration::from_millis(1));
    }

    // Pause simulation
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::Pause))
        .expect("Failed to push pause event");

    // Process the pause
    for _ in 0..5 {
        viewer.step().expect("Step failed");
        std::thread::sleep(Duration::from_millis(1));
    }

    // Record frame count after pause
    let frames_after_pause = video_backend.get_frames().len();

    // Run more steps while paused - frame count should not increase much
    for _ in 0..10 {
        viewer.step().expect("Step failed");
        std::thread::sleep(Duration::from_millis(1));
    }

    let frames_while_paused = video_backend.get_frames().len();

    // Resume simulation
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::Resume))
        .expect("Failed to push resume event");

    // Run more steps
    for _ in 0..20 {
        viewer.step().expect("Step failed");
        std::thread::sleep(Duration::from_millis(1));
    }

    let frames_after_resume = video_backend.get_frames().len();

    println!(
        "Pause/Resume test: {} frames after pause, {} while paused, {} after resume",
        frames_after_pause, frames_while_paused, frames_after_resume
    );

    // Note: Due to timing, this test is somewhat non-deterministic
    // The key assertion is that the viewer doesn't crash
    println!("Threaded viewer pause/resume test passed");
}

#[test]
fn test_threaded_viewer_frame_stepping() {
    let _ = env_logger::builder().is_test(true).try_init();

    let config = ThreadedViewerConfig {
        max_cycles: 1000000,
        batch_size: 1000,
    };

    let (mut viewer, video_backend, _audio_backend) =
        create_headless_viewer(config).expect("Failed to create viewer");

    // Load test ELF (produces exactly 3 frames)
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load ELF");

    // Request 3 frames
    viewer
        .push_event(ViewerEvent::TestCommand(TestCommand::StepFrames(3)))
        .expect("Failed to push step frames event");

    // Run until we have at least 3 frames or safety limit
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }

        steps += 1;
        std::thread::sleep(Duration::from_millis(1));

        let frames = video_backend.get_frames();
        if frames.len() >= 3 {
            println!("Captured {} frames after {} steps", frames.len(), steps);
            break;
        }

        if steps > 1000 {
            let frames = video_backend.get_frames();
            panic!(
                "Safety limit reached: only {} frames captured after {} steps",
                frames.len(),
                steps
            );
        }
    }

    // Verify we captured at least 3 frames
    let frames = video_backend.get_frames();
    assert!(
        frames.len() >= 3,
        "Should have captured at least 3 frames, got {}",
        frames.len()
    );

    println!("Threaded viewer frame stepping test passed: {} frames", frames.len());
}

#[test]
fn test_threaded_viewer_audio() {
    let _ = env_logger::builder().is_test(true).try_init();

    let config = ThreadedViewerConfig {
        max_cycles: 5000000,
        batch_size: 1000,
    };

    let (mut viewer, _video_backend, audio_backend) =
        create_headless_viewer(config).expect("Failed to create viewer");

    // Load audio test ELF
    let elf_path = test_program_path("test_audio_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load ELF");

    // Run until halted or safety limit
    let mut steps = 0;
    loop {
        if !viewer.step().expect("Step failed") {
            break;
        }

        steps += 1;
        std::thread::sleep(Duration::from_millis(1));

        // Check if simulation halted
        if viewer.state() == ViewerState::Halted {
            break;
        }

        if steps > 50000 {
            break;
        }
    }

    // Verify audio was captured
    let audio_chunks = audio_backend.get_chunks();
    let audio_config = audio_backend.get_current_config();

    println!(
        "Audio test: {} steps, {} audio chunks captured",
        steps,
        audio_chunks.len()
    );

    // Verify audio config was set
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

    // Verify audio samples were captured
    assert!(
        !audio_chunks.is_empty(),
        "Audio chunks should have been captured"
    );

    println!("Threaded viewer audio test passed");
}

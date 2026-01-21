//! Integration tests for the threaded simulation architecture.
//!
//! These tests verify that the simulation thread correctly executes programs
//! and delivers video/audio data via the shared state.

use sim_view::{SimCommand, SimNotification, SimState, SimulationThread};
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
fn test_simulation_thread_load_and_run() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create simulation thread
    let thread = SimulationThread::new().expect("Failed to create simulation thread");

    // Initial state should be idle
    assert_eq!(thread.shared_state().get_state(), SimState::Idle);

    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    thread
        .send_command(SimCommand::LoadElf(elf_path))
        .expect("Failed to send load command");

    // Wait for ELF to load (with timeout)
    let start = std::time::Instant::now();
    let mut loaded = false;
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(notification) = thread.try_recv_notification() {
            match notification {
                SimNotification::ElfLoaded => {
                    loaded = true;
                    break;
                }
                SimNotification::ElfLoadError(e) => {
                    panic!("Failed to load ELF: {}", e);
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(loaded, "ELF should have loaded within timeout");

    // State should be running
    assert_eq!(thread.shared_state().get_state(), SimState::Running);

    // Let simulation run for a bit
    std::thread::sleep(Duration::from_millis(200));

    // Terminate
    thread.terminate();
}

#[test]
fn test_simulation_thread_video_frame_delivery() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create simulation thread
    let thread = SimulationThread::new().expect("Failed to create simulation thread");

    // Load test ELF that produces video frames
    let elf_path = test_program_path("test_video_pattern.elf");
    thread
        .send_command(SimCommand::LoadElf(elf_path))
        .expect("Failed to send load command");

    // Wait for ELF to load
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(notification) = thread.try_recv_notification() {
            if matches!(notification, SimNotification::ElfLoaded) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Wait for video frame to appear (with timeout)
    let start = std::time::Instant::now();
    let mut frame_received = false;
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(frame) = thread.shared_state().take_video_frame() {
            // Verify frame data
            assert!(frame.data.len() > 0, "Frame data should not be empty");
            assert!(frame.config.width > 0, "Frame width should be > 0");
            assert!(frame.config.height > 0, "Frame height should be > 0");
            frame_received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        frame_received,
        "Should have received a video frame within timeout"
    );

    // Terminate
    thread.terminate();
}

#[test]
fn test_simulation_thread_pause_resume() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create simulation thread
    let thread = SimulationThread::new().expect("Failed to create simulation thread");

    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    thread
        .send_command(SimCommand::LoadElf(elf_path))
        .expect("Failed to send load command");

    // Wait for ELF to load
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(notification) = thread.try_recv_notification() {
            if matches!(notification, SimNotification::ElfLoaded) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Verify running
    assert_eq!(thread.shared_state().get_state(), SimState::Running);

    // Pause
    thread
        .send_command(SimCommand::Pause)
        .expect("Failed to send pause command");

    // Wait for pause notification
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(notification) = thread.try_recv_notification() {
            if matches!(notification, SimNotification::Paused) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Verify paused
    assert_eq!(thread.shared_state().get_state(), SimState::Paused);

    // Resume
    thread
        .send_command(SimCommand::Resume)
        .expect("Failed to send resume command");

    // Wait for resume notification
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(notification) = thread.try_recv_notification() {
            if matches!(notification, SimNotification::Resumed) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Verify running
    assert_eq!(thread.shared_state().get_state(), SimState::Running);

    // Terminate
    thread.terminate();
}

#[test]
fn test_simulation_thread_audio_delivery() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create simulation thread
    let thread = SimulationThread::new().expect("Failed to create simulation thread");

    // Load test ELF that produces audio samples
    let elf_path = test_program_path("test_audio_pattern.elf");
    thread
        .send_command(SimCommand::LoadElf(elf_path))
        .expect("Failed to send load command");

    // Wait for ELF to load
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(notification) = thread.try_recv_notification() {
            if matches!(notification, SimNotification::ElfLoaded) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Wait for audio data to appear (with timeout)
    let start = std::time::Instant::now();
    let mut audio_received = false;
    let mut config_received = false;
    while start.elapsed() < Duration::from_secs(10) {
        // Check for config
        if let Some(config) = thread.shared_state().take_audio_config() {
            assert!(config.sample_rate.to_hz() > 0, "Sample rate should be > 0");
            config_received = true;
        }

        // Check for samples
        let samples = thread.shared_state().take_audio_samples(1000);
        if !samples.is_empty() {
            audio_received = true;
        }

        if config_received && audio_received {
            break;
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        config_received,
        "Should have received audio config within timeout"
    );
    assert!(
        audio_received,
        "Should have received audio samples within timeout"
    );

    // Terminate
    thread.terminate();
}

#[test]
fn test_simulation_thread_program_halt() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create simulation thread
    let thread = SimulationThread::new().expect("Failed to create simulation thread");

    // Load test ELF that halts (test_video_pattern.elf halts after 3 frames)
    let elf_path = test_program_path("test_video_pattern.elf");
    thread
        .send_command(SimCommand::LoadElf(elf_path))
        .expect("Failed to send load command");

    // Wait for program to halt (with timeout)
    let start = std::time::Instant::now();
    let mut halted = false;
    while start.elapsed() < Duration::from_secs(30) {
        if let Some(notification) = thread.try_recv_notification() {
            if let SimNotification::Halted(tohost) = notification {
                // Verify tohost value
                log::info!("Program halted with tohost: 0x{:08x}", tohost);
                halted = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(halted, "Program should have halted within timeout");
    assert_eq!(thread.shared_state().get_state(), SimState::Halted);

    // Terminate
    thread.terminate();
}

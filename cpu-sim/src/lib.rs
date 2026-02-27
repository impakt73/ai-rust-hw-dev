// Internal modules - not part of public API
mod constants;
mod hung_detector;
mod interactive;
mod sim;
mod simulator_view;

// Public API exports - only what's needed for external use
pub use bus_shared::{
    is_valid_dram_range, Audio, AudioChannels, AudioConfig, AudioSampleRate, BusDevice,
    BusDeviceError, Dma, Fifo, FifoDataReceivedCallback, FifoDataSource, RegistrationError,
    SharedFifoDataSource, SystemBus, SystemContext, Video, VideoConfig, VideoFormat, AUDIO_BASE,
    DRAM_BASE, DRAM_END, FIFO_BASE, LED_BASE, SIM_CONTROL_BASE, VIDEO_BASE,
};
pub use constants::GLOBAL_MAX_CYCLES;
pub use host_bus_handler::{AccessSize, BusRequest, BusResponse};
pub use interactive::InteractiveSimulator;
pub use riscv_core::trace::InstructionTrace;
pub use sim::{BootError, SimulationResult, SimulationStepCycleResult};
pub use simulator_view::SimulatorView;

/// Push a UTF-8 string into a FIFO RX queue as individual bytes.
///
/// Appends a null terminator (0x00) if the string length is a multiple of 4 bytes.
pub fn push_string_to_fifo_rx(fifo_source: &SharedFifoDataSource, s: &str) {
    let bytes = s.as_bytes();
    let mut source = fifo_source
        .lock()
        .expect("push_string_to_fifo_rx source lock poisoned");

    for &byte in bytes {
        source.write_byte(byte);
    }

    // Add null terminator if string length is multiple of 4
    if bytes.len().is_multiple_of(4) {
        source.write_byte(0);
    }
}

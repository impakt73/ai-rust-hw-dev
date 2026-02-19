mod audio;
mod bus;
mod bus_device;
mod dma;
mod dram;
mod fifo;
mod memory;
mod sim_control;
mod video;

pub use audio::{Audio, AudioChannels, AudioConfig, AudioSampleRate};
pub use bus::{
    is_valid_dram_range, AUDIO_BASE, DRAM_BASE, DRAM_END, FIFO_BASE, LED_BASE, RTL_PERIPH_BASE,
    RTL_PERIPH_LIMIT, SIM_CONTROL_BASE, VIDEO_BASE,
};
pub use bus_device::{BusDevice, BusDeviceError, RegistrationError, SystemContext};
pub use dma::Dma;
pub use dram::Dram;
pub use fifo::{Fifo, FifoDataReceivedCallback, FifoDataSource};
pub use memory::Memory;
pub use sim_control::SimControl;
pub use video::{Video, VideoConfig, VideoFormat};

pub use bus::SystemBus;

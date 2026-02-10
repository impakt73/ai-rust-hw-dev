//! Serial connection and bus protocol handling
//!
//! This module re-exports types from the `device-runtime` crate and provides
//! the FPGA-specific device runtime for communicating with the FPGA using
//! the host-bus-handler protocol over a serial port.

pub use device_runtime::fpga::FpgaDeviceRuntime;
pub use device_runtime::{access_size_name, bytes_for_size, size_name, BusEvent, DeviceRuntime};

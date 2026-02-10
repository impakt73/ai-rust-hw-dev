//! Serial connection and bus protocol handling
//!
//! This module re-exports types from the `device-runtime` crate for
//! communicating with a RISC-V CPU device.

pub use device_runtime::{
    access_size_name, bytes_for_size, create_device_runtime, size_name, BusEvent, DeviceRuntime,
    DeviceRuntimeType,
};

//! Memory model shared with the bus subsystem.
//!
//! This re-exports the shared Memory implementation from the bus-shared crate
//! so device runtime code can use the same backing store as the simulator bus.

pub use bus_shared::Memory;

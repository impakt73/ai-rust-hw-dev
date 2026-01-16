use crate::bus_device::{BusDevice, BusDeviceError};

/// Simulator control device
///
/// Provides memory-mapped registers for controlling the simulator,
/// including the tohost register for signaling program completion.
pub struct SimControl {
    tohost_value: Option<u32>,
}

impl SimControl {
    /// Create a new SimControl device
    pub fn new() -> Self {
        SimControl { tohost_value: None }
    }

    /// Check if a termination request has been made
    ///
    /// Returns `Some(value)` if a write to tohost occurred, `None` otherwise.
    pub fn termination_requested(&self) -> Option<u32> {
        self.tohost_value
    }

    /// Clear the termination request (for reset)
    #[allow(dead_code)]
    pub fn clear_termination(&mut self) {
        self.tohost_value = None;
    }
}

impl Default for SimControl {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for SimControl {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => {
                // TOHOST register is write-only
                Err(BusDeviceError::ReadFromWriteOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // TOHOST register - write triggers termination
                self.tohost_value = Some(value);
                log::info!("SimControl: tohost write detected, value={:#010x}", value);
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // Single 32-bit register: TOHOST
        4
    }

    fn name(&self) -> &str {
        "SimControl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_control_write_tohost() {
        let mut sim_control = SimControl::new();
        assert_eq!(sim_control.termination_requested(), None);

        sim_control.write_word(0, 42).unwrap();
        assert_eq!(sim_control.termination_requested(), Some(42));
    }

    #[test]
    fn test_sim_control_read_tohost_fails() {
        let mut sim_control = SimControl::new();
        let result = sim_control.read_word(0);
        assert!(matches!(
            result,
            Err(BusDeviceError::ReadFromWriteOnly { offset: 0 })
        ));
    }

    #[test]
    fn test_sim_control_invalid_address() {
        let mut sim_control = SimControl::new();
        assert!(matches!(
            sim_control.read_word(4),
            Err(BusDeviceError::InvalidAddress { offset: 4 })
        ));
        assert!(matches!(
            sim_control.write_word(4, 0),
            Err(BusDeviceError::InvalidAddress { offset: 4 })
        ));
    }

    #[test]
    fn test_sim_control_clear_termination() {
        let mut sim_control = SimControl::new();
        sim_control.write_word(0, 42).unwrap();
        assert_eq!(sim_control.termination_requested(), Some(42));

        sim_control.clear_termination();
        assert_eq!(sim_control.termination_requested(), None);
    }
}

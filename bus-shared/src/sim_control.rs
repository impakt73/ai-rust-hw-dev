use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// Simulator control device
///
/// Provides memory-mapped registers for controlling the simulator,
/// including the tohost register for signaling program completion.
///
/// Uses a one-shot termination mechanism: the tohost value is consumed
/// when acknowledged via `acknowledge_termination()`.
pub struct SimControl {
    tohost_value: Option<u32>,
}

impl SimControl {
    /// Create a new SimControl device
    pub fn new() -> Self {
        SimControl { tohost_value: None }
    }

    /// Check if a termination request is pending
    ///
    /// Returns `true` if a write to tohost occurred and has not yet been acknowledged.
    pub fn is_termination_pending(&self) -> bool {
        self.tohost_value.is_some()
    }

    /// Acknowledge and consume the pending termination request
    ///
    /// Returns `Some(value)` if a termination was pending, moving the value out
    /// and clearing the internal state. Returns `None` if no termination was pending.
    pub fn acknowledge_termination(&mut self) -> Option<u32> {
        self.tohost_value.take()
    }
}

impl Default for SimControl {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for SimControl {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => {
                // TOHOST register is write-only
                Err(BusDeviceError::ReadFromWriteOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
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

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.tohost_value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn test_sim_control_write_tohost() {
        let mut sim_control = SimControl::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        assert!(!sim_control.is_termination_pending());

        sim_control.write_word(&mut ctx, 0, 42).unwrap();
        assert!(sim_control.is_termination_pending());
    }

    #[test]
    fn test_sim_control_read_tohost_fails() {
        let mut sim_control = SimControl::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = sim_control.read_word(&mut ctx, 0);
        assert!(matches!(
            result,
            Err(BusDeviceError::ReadFromWriteOnly { offset: 0 })
        ));
    }

    #[test]
    fn test_sim_control_invalid_address() {
        let mut sim_control = SimControl::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        assert!(matches!(
            sim_control.read_word(&mut ctx, 4),
            Err(BusDeviceError::InvalidAddress { offset: 4 })
        ));
        assert!(matches!(
            sim_control.write_word(&mut ctx, 4, 0),
            Err(BusDeviceError::InvalidAddress { offset: 4 })
        ));
    }

    #[test]
    fn test_sim_control_acknowledge_termination() {
        let mut sim_control = SimControl::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        sim_control.write_word(&mut ctx, 0, 42).unwrap();
        assert!(sim_control.is_termination_pending());

        // Acknowledge consumes the value
        assert_eq!(sim_control.acknowledge_termination(), Some(42));

        // After acknowledgment, no longer pending
        assert!(!sim_control.is_termination_pending());
        assert_eq!(sim_control.acknowledge_termination(), None);
    }

    #[test]
    fn test_sim_control_reset_clears_termination() {
        let mut sim_control = SimControl::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        sim_control.write_word(&mut ctx, 0, 42).unwrap();
        assert!(sim_control.is_termination_pending());

        sim_control.reset(&mut ctx);

        assert!(!sim_control.is_termination_pending());
        assert_eq!(sim_control.acknowledge_termination(), None);
    }
}

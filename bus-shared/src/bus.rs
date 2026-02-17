use crate::bus_device::{ranges_overlap, BusDevice, RegistrationError, SystemContext};
use crate::dram::Dram;
use crate::fifo::Fifo;
use crate::memory::Memory;
use crate::sim_control::SimControl;

// Re-export constants from riscv_shared for backward compatibility
pub use riscv_shared::bus::{
    is_valid_dram_range, AUDIO_BASE, DRAM_BASE, DRAM_END, FIFO_BASE, LED_BASE, RTL_PERIPH_BASE,
    RTL_PERIPH_LIMIT, SIM_CONTROL_BASE, VIDEO_BASE,
};

/// Lightweight handle identifying which device owns an address range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceId {
    /// Internal DRAM device (SystemBus.dram field)
    Dram,
    /// Internal FIFO device (SystemBus.fifo field)
    Fifo,
    /// Internal SimControl device (SystemBus.sim_control field)
    SimControl,
    /// External device (index into SystemBus.external_devices Vec)
    External(usize),
}

/// Memory map entry separating "where" from "what"
/// Maps an address range to a device handle
struct MemoryMapEntry {
    base: u32,
    end: u32, // Exclusive end address (base + size)
    id: DeviceId,
}

/// System bus that routes memory accesses to the correct device
pub struct SystemBus {
    // Shared memory accessible by all devices
    pub memory: Memory,

    // Internal devices as concrete public fields (for SimulatorView access)
    pub dram: Dram,
    pub fifo: Fifo,
    pub sim_control: SimControl,

    // External devices (owned by the bus)
    external_devices: Vec<Box<dyn BusDevice>>,

    // Address map (lightweight handles, not device references)
    memory_map: Vec<MemoryMapEntry>,

    // Accumulated elapsed time in microseconds (host CPU time)
    elapsed_time_us: u64,
}

impl SystemBus {
    /// Create a new system bus with internal devices initialized
    ///
    /// The bus owns DRAM, FIFO, and SimControl as concrete fields,
    /// pre-registered in the memory map. External devices can be added later.
    pub fn new() -> Self {
        let memory = Memory::new();
        let dram = Dram::new();
        let fifo = Fifo::new();
        let sim_control = SimControl::new();

        // Pre-populate memory map with internal devices
        let memory_map = vec![
            MemoryMapEntry {
                base: SIM_CONTROL_BASE,
                end: SIM_CONTROL_BASE.saturating_add(sim_control.size()),
                id: DeviceId::SimControl,
            },
            MemoryMapEntry {
                base: FIFO_BASE,
                end: FIFO_BASE.saturating_add(fifo.size()),
                id: DeviceId::Fifo,
            },
            MemoryMapEntry {
                base: DRAM_BASE,
                end: DRAM_BASE.saturating_add(dram.size()),
                id: DeviceId::Dram,
            },
        ];

        SystemBus {
            memory,
            dram,
            fifo,
            sim_control,
            external_devices: Vec::new(),
            memory_map,
            elapsed_time_us: 0,
        }
    }

    /// Check if an address is in the RTL peripheral address space
    ///
    /// RTL peripherals are handled directly by the Verilator top module,
    /// not by the Rust SystemBus. This method helps identify such addresses.
    pub fn is_rtl_peripheral(&self, addr: u32) -> bool {
        (RTL_PERIPH_BASE..RTL_PERIPH_LIMIT).contains(&addr)
    }

    /// Update the elapsed time (called by simulator after each step)
    pub fn update_elapsed_time(&mut self, elapsed_time_us: u64) {
        self.elapsed_time_us = elapsed_time_us;
    }

    /// Register an external device at the specified base address
    ///
    /// Takes ownership of the device and adds it to the memory map.
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device (must be word-aligned)
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(RegistrationError)` - Address range conflicts or invalid alignment
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), RegistrationError> {
        let size = device.size();

        // Validate size and alignment
        if size == 0 {
            return Err(RegistrationError::ZeroSize);
        }
        if !size.is_multiple_of(4) {
            return Err(RegistrationError::InvalidAlignment { size });
        }
        if !base_addr.is_multiple_of(4) {
            return Err(RegistrationError::InvalidBaseAlignment { base_addr });
        }

        let end = base_addr.saturating_add(size);

        // Check for overlaps with existing devices
        for entry in &self.memory_map {
            if ranges_overlap(base_addr, end, entry.base, entry.end) {
                let device_name = match entry.id {
                    DeviceId::Dram => self.dram.name(),
                    DeviceId::Fifo => self.fifo.name(),
                    DeviceId::SimControl => self.sim_control.name(),
                    DeviceId::External(idx) => self.external_devices[idx].name(),
                };
                return Err(RegistrationError::AddressOverlap {
                    new_base: base_addr,
                    new_end: end,
                    existing_base: entry.base,
                    existing_end: entry.end,
                    existing_name: device_name.to_string(),
                });
            }
        }

        // Add device to storage and create memory map entry
        let device_idx = self.external_devices.len();
        let device_name = device.name().to_string();
        self.external_devices.push(device);

        self.memory_map.push(MemoryMapEntry {
            base: base_addr,
            end,
            id: DeviceId::External(device_idx),
        });

        log::info!(
            "Registered device '{}' at 0x{:08x} - 0x{:08x} (size: {} bytes)",
            device_name,
            base_addr,
            end - 1,
            size
        );

        Ok(())
    }

    /// Get immutable access to registered devices (for introspection/debugging)
    ///
    /// Returns an iterator over (base_addr, size, name) tuples for all registered devices.
    #[allow(dead_code)]
    pub fn registered_devices(&self) -> impl Iterator<Item = (u32, u32, &str)> + '_ {
        self.memory_map.iter().map(|entry| {
            let size = entry.end.wrapping_sub(entry.base);
            let name = match entry.id {
                DeviceId::Dram => self.dram.name(),
                DeviceId::Fifo => self.fifo.name(),
                DeviceId::SimControl => self.sim_control.name(),
                DeviceId::External(idx) => self.external_devices[idx].name(),
            };
            (entry.base, size, name)
        })
    }

    /// Find the device ID for the given address
    ///
    /// Returns the DeviceId handle and the offset relative to the device's base address.
    fn find_device_id(&self, addr: u32) -> Option<(DeviceId, u32)> {
        for entry in &self.memory_map {
            if addr >= entry.base && addr < entry.end {
                let offset = addr - entry.base;
                return Some((entry.id, offset));
            }
        }
        None
    }

    /// Read a 32-bit word from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches, logs a warning and returns 0.
    pub fn read_word(&mut self, addr: u32) -> u32 {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral read should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => BusDevice::read_word(&mut self.dram, &mut ctx, offset),
                DeviceId::Fifo => BusDevice::read_word(&mut self.fifo, &mut ctx, offset),
                DeviceId::SimControl => {
                    BusDevice::read_word(&mut self.sim_control, &mut ctx, offset)
                }
                DeviceId::External(idx) => self.external_devices[idx].read_word(&mut ctx, offset),
            };

            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_word error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!(
                "Bus read_word from unmapped address 0x{:08x}, returning 0",
                addr
            );
            0
        }
    }

    /// Write a 32-bit word to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches, logs a warning and discards the write.
    pub fn write_word(&mut self, addr: u32, value: u32) {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral write should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => BusDevice::write_word(&mut self.dram, &mut ctx, offset, value),
                DeviceId::Fifo => BusDevice::write_word(&mut self.fifo, &mut ctx, offset, value),
                DeviceId::SimControl => {
                    BusDevice::write_word(&mut self.sim_control, &mut ctx, offset, value)
                }
                DeviceId::External(idx) => {
                    self.external_devices[idx].write_word(&mut ctx, offset, value)
                }
            };

            if let Err(e) = result {
                log::warn!("Bus write_word error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_word to unmapped address 0x{:08x} (value=0x{:08x}), discarding",
                addr,
                value
            );
        }
    }

    /// Read a 16-bit halfword from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support halfword access,
    /// logs a warning and returns 0.
    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral read should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => BusDevice::read_halfword(&mut self.dram, &mut ctx, offset),
                DeviceId::Fifo => BusDevice::read_halfword(&mut self.fifo, &mut ctx, offset),
                DeviceId::SimControl => {
                    BusDevice::read_halfword(&mut self.sim_control, &mut ctx, offset)
                }
                DeviceId::External(idx) => {
                    self.external_devices[idx].read_halfword(&mut ctx, offset)
                }
            };

            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_halfword error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!(
                "Bus read_halfword from unmapped address 0x{:08x}, returning 0",
                addr
            );
            0
        }
    }

    /// Write a 16-bit halfword to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support halfword access,
    /// logs a warning and discards the write.
    pub fn write_halfword(&mut self, addr: u32, value: u16) {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral write should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => {
                    BusDevice::write_halfword(&mut self.dram, &mut ctx, offset, value)
                }
                DeviceId::Fifo => {
                    BusDevice::write_halfword(&mut self.fifo, &mut ctx, offset, value)
                }
                DeviceId::SimControl => {
                    BusDevice::write_halfword(&mut self.sim_control, &mut ctx, offset, value)
                }
                DeviceId::External(idx) => {
                    self.external_devices[idx].write_halfword(&mut ctx, offset, value)
                }
            };

            if let Err(e) = result {
                log::warn!("Bus write_halfword error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_halfword to unmapped address 0x{:08x} (value=0x{:04x}), discarding",
                addr,
                value
            );
        }
    }

    /// Read a single byte from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support byte access,
    /// logs a warning and returns 0.
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral read should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => BusDevice::read_byte(&mut self.dram, &mut ctx, offset),
                DeviceId::Fifo => BusDevice::read_byte(&mut self.fifo, &mut ctx, offset),
                DeviceId::SimControl => {
                    BusDevice::read_byte(&mut self.sim_control, &mut ctx, offset)
                }
                DeviceId::External(idx) => self.external_devices[idx].read_byte(&mut ctx, offset),
            };

            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_byte error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!(
                "Bus read_byte from unmapped address 0x{:08x}, returning 0",
                addr
            );
            0
        }
    }

    /// Write a single byte to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support byte access,
    /// logs a warning and discards the write.
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        // RTL peripherals should never reach Rust - they're handled by Verilator
        if self.is_rtl_peripheral(addr) {
            panic!(
                "RTL peripheral write should be handled by Verilator, not Rust: 0x{:08x}",
                addr
            );
        }

        if let Some((id, offset)) = self.find_device_id(addr) {
            // Create SystemContext for device access to memory
            let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

            let result = match id {
                DeviceId::Dram => BusDevice::write_byte(&mut self.dram, &mut ctx, offset, value),
                DeviceId::Fifo => BusDevice::write_byte(&mut self.fifo, &mut ctx, offset, value),
                DeviceId::SimControl => {
                    BusDevice::write_byte(&mut self.sim_control, &mut ctx, offset, value)
                }
                DeviceId::External(idx) => {
                    self.external_devices[idx].write_byte(&mut ctx, offset, value)
                }
            };

            if let Err(e) = result {
                log::warn!("Bus write_byte error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_byte to unmapped address 0x{:08x} (value=0x{:02x}), discarding",
                addr,
                value
            );
        }
    }

    /// Call reset() on all registered devices
    ///
    /// This should be called when the simulator is reset to allow devices
    /// to clear their internal state.
    pub fn reset_all_devices(&mut self) {
        let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

        // Reset internal devices
        self.dram.reset(&mut ctx);
        self.fifo.reset(&mut ctx);
        self.sim_control.reset(&mut ctx);

        // Reset external devices
        for device in &mut self.external_devices {
            device.reset(&mut ctx);
        }
    }

    /// Call clock_cycle() on all registered devices
    ///
    /// This should be called once per simulated clock cycle to allow devices
    /// to perform multi-cycle operations.
    pub fn clock_cycle_all_devices(&mut self) {
        let mut ctx = SystemContext::with_elapsed_time(&mut self.memory, self.elapsed_time_us);

        // Tick internal devices
        self.dram.clock_cycle(&mut ctx);
        self.fifo.clock_cycle(&mut ctx);
        self.sim_control.clock_cycle(&mut ctx);

        // Tick external devices
        for device in &mut self.external_devices {
            device.clock_cycle(&mut ctx);
        }
    }
}

impl Default for SystemBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;

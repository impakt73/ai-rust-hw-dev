use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// DRAM bus device for the RISC-V CPU simulator
///
/// This device no longer stores data directly. Instead, it forwards all
/// memory operations to the SystemContext (which accesses the shared Memory),
/// translating device-relative offsets to absolute addresses.
///
/// The DRAM device is mapped at base address 0x8000_0000 in the system
/// memory map, and this device translates offsets to absolute addresses
/// by adding the base address before accessing memory through SystemContext.
pub struct Dram {
    base_addr: u32,
}

impl Dram {
    /// Create a new DRAM device
    pub fn new() -> Self {
        Dram {
            base_addr: crate::bus::DRAM_BASE,
        }
    }
}

impl Default for Dram {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for Dram {
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        Ok(ctx.read_word(addr))
    }

    fn write_word(
        &mut self,
        ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        ctx.write_word(addr, value);
        Ok(())
    }

    fn read_halfword(
        &mut self,
        ctx: &mut SystemContext,
        offset: u32,
    ) -> Result<u16, BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        Ok(ctx.read_halfword(addr))
    }

    fn write_halfword(
        &mut self,
        ctx: &mut SystemContext,
        offset: u32,
        value: u16,
    ) -> Result<(), BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        ctx.write_halfword(addr, value);
        Ok(())
    }

    fn read_byte(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u8, BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        Ok(ctx.read_byte(addr))
    }

    fn write_byte(
        &mut self,
        ctx: &mut SystemContext,
        offset: u32,
        value: u8,
    ) -> Result<(), BusDeviceError> {
        // Convert device-relative offset to absolute address
        let addr = self.base_addr.wrapping_add(offset);
        ctx.write_byte(addr, value);
        Ok(())
    }

    fn size(&self) -> u32 {
        // DRAM size: 2 GiB mapped from 0x8000_0000 to 0xFFFF_FFFF
        // Size = 0xFFFF_FFFF - 0x8000_0000 + 1 = 0x8000_0000 bytes
        0x8000_0000
    }

    fn name(&self) -> &str {
        "DRAM"
    }
}

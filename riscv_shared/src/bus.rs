//! Bus device base addresses and memory range validation

/// Rust Peripheral Address Space
/// Base address for Rust peripherals handled by SystemBus (including DRAM)
/// and reached through the host-bus path on FPGA backends.
pub const RUST_PERIPH_BASE: u32 = 0x8000_0000;

/// Limit marker for Rust peripheral region (inclusive upper-half address space)
pub const RUST_PERIPH_LIMIT: u32 = 0xFFFF_FFFF;

/// Base address for SimControl device (tohost register)
pub const SIM_CONTROL_BASE: u32 = 0xF000_0000;

/// Base address for Video device
/// This is the recommended base address for external Video bus devices
pub const VIDEO_BASE: u32 = 0x9000_0000;

/// Base address for Audio device
/// This is the recommended base address for external Audio bus devices
pub const AUDIO_BASE: u32 = 0xA000_0000;

/// Base address for FIFO device
pub const FIFO_BASE: u32 = 0xB000_0000;

/// RTL Peripheral Address Space
/// Base address for RTL peripherals (synthesizable peripherals in Verilog)
pub const RTL_PERIPH_BASE: u32 = 0x0000_0000;

/// Limit address for RTL peripherals (exclusive)
pub const RTL_PERIPH_LIMIT: u32 = 0x8000_0000;

/// SRAM Peripheral (RTL)
pub const SRAM_BASE: u32 = 0x7000_0000;
pub const SRAM_SIZE: u32 = 0x0000_3000; // 12KB

/// System Controller Peripheral (RTL)
pub const SYSCTRL_BASE: u32 = 0x2000_0000;
pub const SYSCTRL_SIZE: u32 = 0x0000_0020; // 32 bytes

/// System Controller register offsets
pub const SYSCTRL_STATUS_OFFSET: u32 = 0x00;
pub const SYSCTRL_RESET_OFFSET: u32 = 0x04;
pub const SYSCTRL_BOOT_OFFSET: u32 = 0x08;
pub const SYSCTRL_HALT_OFFSET: u32 = 0x0C;
pub const SYSCTRL_LED_OUT_OFFSET: u32 = 0x10;
pub const SYSCTRL_ELAPSED_US_OFFSET: u32 = 0x14;
pub const SYSCTRL_ELAPSED_MS_OFFSET: u32 = 0x18;
pub const SYSCTRL_ELAPSED_S_OFFSET: u32 = 0x1C;

/// System Controller RESET register write values.
///
/// Only write-data bit 0 selects the reset type:
/// - bit 0 = 0 => system reset
/// - bit 0 = 1 => CPU reset (halts the CPU first, then pulses reset)
pub const SYSCTRL_RESET_SYSTEM: u32 = 0x0000_0000;
pub const SYSCTRL_RESET_CPU: u32 = 0x0000_0001;

/// System Controller status register bit masks
pub const SYSCTRL_STATUS_CPU_BOOTING: u32 = 1 << 0;
pub const SYSCTRL_STATUS_CPU_HALTED: u32 = 1 << 1;

/// Helper function to get System Controller STATUS register address
pub const fn sysctrl_status_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_STATUS_OFFSET
}

/// Helper function to get System Controller RESET register address
pub const fn sysctrl_reset_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_RESET_OFFSET
}

/// Helper function to get System Controller BOOT register address
pub const fn sysctrl_boot_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_BOOT_OFFSET
}

/// Helper function to get System Controller HALT register address
pub const fn sysctrl_halt_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_HALT_OFFSET
}

/// Helper function to get System Controller LED_OUT register address
pub const fn sysctrl_led_out_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_LED_OUT_OFFSET
}

/// Helper function to get System Controller ELAPSED_US register address
pub const fn sysctrl_elapsed_us_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_ELAPSED_US_OFFSET
}

/// Helper function to get System Controller ELAPSED_MS register address
pub const fn sysctrl_elapsed_ms_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_ELAPSED_MS_OFFSET
}

/// Helper function to get System Controller ELAPSED_S register address
pub const fn sysctrl_elapsed_s_addr() -> u32 {
    SYSCTRL_BASE + SYSCTRL_ELAPSED_S_OFFSET
}

/// Check if an address targets RTL peripheral space.
///
/// By convention, all RTL peripherals live in the lower half of the address
/// space and all Rust/host-routed devices live in the upper half.
pub const fn is_rtl_peripheral_addr(addr: u32) -> bool {
    (addr & 0x8000_0000) == 0
}

/// Base address for DRAM (a Rust peripheral in this project)
pub const DRAM_BASE: u32 = 0x8000_0000;

/// End address for DRAM (inclusive)
/// DRAM range is [DRAM_BASE, DRAM_END] = [0x8000_0000, 0x8FFF_FFFF] (256 MiB)
pub const DRAM_END: u32 = 0x8FFF_FFFF;

/// Check if an address range is within the valid DRAM range
///
/// # Arguments
/// * `addr` - Starting address
/// * `size` - Number of bytes the access will span
///
/// # Returns
/// `true` if the entire range [addr, addr+size-1] is within DRAM range
pub fn is_valid_dram_range(addr: u32, size: u32) -> bool {
    // A zero-sized access corresponds to an empty range and is not considered valid
    if size == 0 {
        return false;
    }

    // Check for overflow when computing the inclusive end address
    let end_addr = addr.checked_add(size.saturating_sub(1));
    if end_addr.is_none() {
        return false;
    }
    let end_addr = end_addr.unwrap();

    // Check if both start and end are within the inclusive DRAM range [DRAM_BASE, DRAM_END]
    // Using explicit range checks for maintainability. If DRAM_END is changed in the future
    // (e.g., to reserve high addresses for memory-mapped I/O), this validation will continue
    // to work correctly.
    (DRAM_BASE..=DRAM_END).contains(&addr) && (DRAM_BASE..=DRAM_END).contains(&end_addr)
}

//! Bus device base addresses and memory range validation

/// Rust Peripheral Address Space
/// Legacy contiguous Rust peripheral base (deprecated for decode).
pub const RUST_PERIPH_BASE: u32 = 0x0000_0000;

/// Legacy contiguous Rust peripheral limit (exclusive, deprecated for decode)
pub const RUST_PERIPH_LIMIT: u32 = 0x8000_0000;

/// Base address for SimControl device (tohost register)
pub const SIM_CONTROL_BASE: u32 = 0xF000_0000;

/// Base address for Video device
/// This is the recommended base address for external Video bus devices
pub const VIDEO_BASE: u32 = 0x0000_0000;

/// Base address for Audio device
/// This is the recommended base address for external Audio bus devices
pub const AUDIO_BASE: u32 = 0x1000_0000;

/// Base address for FIFO device
pub const FIFO_BASE: u32 = 0x3000_0000;

/// RTL Peripheral Address Space
/// Legacy contiguous RTL peripheral base (deprecated for decode).
///
/// Kept for compatibility only. Do not use this range for decode logic because
/// RTL peripherals are now placed on non-contiguous 256MB windows and decoded
/// by top nibble via [`is_rtl_peripheral_addr()`].
pub const RTL_PERIPH_BASE: u32 = 0x5000_0000;

/// Legacy contiguous RTL peripheral limit (exclusive, deprecated for decode)
pub const RTL_PERIPH_LIMIT: u32 = 0x8000_0000;

/// LED Controller Peripheral (RTL)
pub const LED_BASE: u32 = 0x5000_0000;

/// LED Controller size (16 bytes)
pub const LED_SIZE: u32 = 0x0000_0010;

/// LED register offset: LED_OUT (output data register)
pub const LED_OUT_OFFSET: u32 = 0x00;

/// Helper function to get LED_OUT register address
pub const fn led_out_addr() -> u32 {
    LED_BASE + LED_OUT_OFFSET
}

/// SRAM Peripheral (RTL)
pub const SRAM_BASE: u32 = 0x7000_0000;
pub const SRAM_SIZE: u32 = 0x0000_3000; // 12KB

/// Clock Peripheral (RTL)
pub const CLOCK_BASE: u32 = 0x6000_0000;
pub const CLOCK_SIZE: u32 = 0x0000_0010; // 16 bytes

/// Clock peripheral register offsets
pub const CLOCK_ELAPSED_US_OFFSET: u32 = 0x00;
pub const CLOCK_ELAPSED_MS_OFFSET: u32 = 0x04;
pub const CLOCK_ELAPSED_S_OFFSET: u32 = 0x08;

/// Helper function to get CLOCK ELAPSED_US register address
pub const fn clock_elapsed_us_addr() -> u32 {
    CLOCK_BASE + CLOCK_ELAPSED_US_OFFSET
}

/// Helper function to get CLOCK ELAPSED_MS register address
pub const fn clock_elapsed_ms_addr() -> u32 {
    CLOCK_BASE + CLOCK_ELAPSED_MS_OFFSET
}

/// Helper function to get CLOCK ELAPSED_S register address
pub const fn clock_elapsed_s_addr() -> u32 {
    CLOCK_BASE + CLOCK_ELAPSED_S_OFFSET
}

/// System Controller Peripheral (RTL)
pub const SYSCTRL_BASE: u32 = 0x2000_0000;
pub const SYSCTRL_SIZE: u32 = 0x0000_0010; // 16 bytes

/// System Controller register offsets
pub const SYSCTRL_STATUS_OFFSET: u32 = 0x00;
pub const SYSCTRL_RESET_OFFSET: u32 = 0x04;
pub const SYSCTRL_BOOT_OFFSET: u32 = 0x08;
pub const SYSCTRL_HALT_OFFSET: u32 = 0x0C;

/// System Controller reset control values
pub const SYSCTRL_RESET_SYSTEM: u32 = 0x0000_0001;
pub const SYSCTRL_RESET_CPU: u32 = 0x0000_0002;

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

/// Check if an address targets an RTL peripheral window.
///
/// RTL peripherals are selected by top nibble:
/// - `0x2xxxxxxx`: System Controller
/// - `0x5xxxxxxx`: LED Controller
/// - `0x6xxxxxxx`: Clock Peripheral
/// - `0x7xxxxxxx`: SRAM Peripheral
pub const fn is_rtl_peripheral_addr(addr: u32) -> bool {
    matches!(addr >> 28, 0x2 | 0x5 | 0x6 | 0x7)
}

/// Base address for DRAM
pub const DRAM_BASE: u32 = 0x8000_0000;

/// End address for DRAM (inclusive)
/// DRAM range is [DRAM_BASE, DRAM_END] = [0x8000_0000, 0xFFFF_FFFF]
pub const DRAM_END: u32 = 0xFFFF_FFFF;

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

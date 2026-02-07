//! Bus device base addresses and memory range validation

/// Base address for SimControl device (tohost register)
pub const SIM_CONTROL_BASE: u32 = 0x1000_0000;

/// Base address for Video device
/// This is the recommended base address for external Video bus devices
pub const VIDEO_BASE: u32 = 0x2000_0000;

/// Base address for Audio device
/// This is the recommended base address for external Audio bus devices
pub const AUDIO_BASE: u32 = 0x3000_0000;

/// Base address for FIFO device
pub const FIFO_BASE: u32 = 0x4000_0000;

/// RTL Peripheral Address Space
/// Base address for RTL peripherals (synthesizable peripherals in Verilog)
pub const RTL_PERIPH_BASE: u32 = 0x5000_0000;

/// Limit address for RTL peripherals (exclusive)
pub const RTL_PERIPH_LIMIT: u32 = 0x6000_0000;

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

/// UART Controller Peripheral (RTL)
pub const UART_BASE: u32 = 0x5200_0000;
pub const UART_SIZE: u32 = 0x0000_0100; // 256 bytes

/// UART register offsets
pub const UART_TXDATA_OFFSET: u32 = 0x00;
pub const UART_RXDATA_OFFSET: u32 = 0x04;
pub const UART_STATUS_OFFSET: u32 = 0x08;
pub const UART_CTRL_OFFSET: u32 = 0x0C;

/// UART status register bit masks
pub const UART_STATUS_TX_FULL: u32 = 1 << 0;
pub const UART_STATUS_TX_EMPTY: u32 = 1 << 1;
pub const UART_STATUS_TX_BUSY: u32 = 1 << 2;
pub const UART_STATUS_RX_FULL: u32 = 1 << 4;
pub const UART_STATUS_RX_EMPTY: u32 = 1 << 5;
pub const UART_STATUS_RX_BUSY: u32 = 1 << 6;
pub const UART_STATUS_RX_ERROR: u32 = 1 << 7;

/// Helper function to get UART TXDATA register address
pub const fn uart_txdata_addr() -> u32 {
    UART_BASE + UART_TXDATA_OFFSET
}

/// Helper function to get UART RXDATA register address
pub const fn uart_rxdata_addr() -> u32 {
    UART_BASE + UART_RXDATA_OFFSET
}

/// Helper function to get UART STATUS register address
pub const fn uart_status_addr() -> u32 {
    UART_BASE + UART_STATUS_OFFSET
}

/// Clock Peripheral (RTL)
pub const CLOCK_BASE: u32 = 0x5100_0000;
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
pub const SYSCTRL_BASE: u32 = 0x5300_0000;
pub const SYSCTRL_SIZE: u32 = 0x0000_0010; // 16 bytes

/// System Controller register offsets
pub const SYSCTRL_STATUS_OFFSET: u32 = 0x00;
pub const SYSCTRL_RESET_OFFSET: u32 = 0x04;
pub const SYSCTRL_BOOT_OFFSET: u32 = 0x08;

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

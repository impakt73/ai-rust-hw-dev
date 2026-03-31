use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{Uart, Uart1MBaud};
use testbench::{uart_1m_runtime, uart_runtime};
// UART timing for 1M baud (based on 50MHz clock)
// CLKS_PER_BIT = 50_000_000 / 1_000_000 = 50
const CLKS_PER_BIT: u32 = 50;
// UART timing for 1M baud in uart_1m_baud_wrapper.sv (based on 25MHz clock)
const CLKS_PER_BIT_1M: u32 = 25;
const UART_BYTE_TIMEOUT_BITS: u32 = 15;

// Clock cycle macro for UART tests
macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

/// Helper function to apply reset to the UART
fn reset_uart(dut: &mut Uart) {
    dut.rst = 1;
    dut.tx_valid = 0;
    dut.rx_ready = 0;
    dut.rx_error_clr = 0;
    dut.rx_in = 1; // RX line idle high
    clock_cycle!(dut);
    dut.rst = 0;
    clock_cycle!(dut);
}

/// Helper function to wait for a specific number of clock cycles
fn wait_cycles(dut: &mut Uart, cycles: u32) {
    for _ in 0..cycles {
        clock_cycle!(dut);
    }
}

/// Helper function to transmit a byte and return number of cycles taken
#[allow(dead_code)]
fn transmit_byte(dut: &mut Uart, data: u8) -> u32 {
    // Set up TX data
    dut.tx_data = data;
    dut.tx_valid = 1;
    dut.eval();

    // Wait for ready to be asserted (should be immediate if idle)
    assert_eq!(dut.tx_ready, 1, "TX should be ready when idle");

    // Clock the data in
    clock_cycle!(dut);

    // Clear valid signal
    dut.tx_valid = 0;
    dut.eval();

    // Wait for transmission to complete (start + 8 data + stop = 10 bits)
    let total_cycles = CLKS_PER_BIT * 10;
    wait_cycles(dut, total_cycles);

    total_cycles
}

/// Helper function to receive a byte by driving rx_in manually
fn receive_byte(dut: &mut Uart, data: u8) {
    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8 {
        dut.rx_in = if (data >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(dut, CLKS_PER_BIT);
    }

    // Stop bit (high)
    dut.rx_in = 1;
    wait_cycles(dut, CLKS_PER_BIT);
}

#[test]
fn test_uart_reset_state() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Verify TX line is idle high after reset
    assert_eq!(dut.tx_out, 1, "TX should be idle high after reset");

    // Verify RX valid is low after reset
    assert_eq!(dut.rx_valid, 0, "RX valid should be low after reset");

    // Verify TX ready is high after reset (idle state)
    assert_eq!(dut.tx_ready, 1, "TX ready should be high after reset");

    // Verify RX error is low after reset
    assert_eq!(dut.rx_error, 0, "RX error should be low after reset");
}

#[test]
fn test_uart_tx_idle_high() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // TX line should stay high without activity
    for _ in 0..100 {
        assert_eq!(dut.tx_out, 1, "TX should remain idle high");
        clock_cycle!(dut);
    }
}

#[test]
fn test_uart_tx_start_bit() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Initiate transmission
    dut.tx_data = 0x55;
    dut.tx_valid = 1;
    dut.eval();

    // Verify ready is high (idle state)
    assert_eq!(dut.tx_ready, 1, "TX should be ready before transmission");

    clock_cycle!(dut);

    // TX ready should go low (transmission started)
    assert_eq!(
        dut.tx_ready, 0,
        "TX ready should go low during transmission"
    );

    // Clear valid
    dut.tx_valid = 0;
    dut.eval();

    // Wait a few cycles to let start bit stabilize
    wait_cycles(&mut dut, 10);

    // Verify start bit (TX should be low)
    assert_eq!(dut.tx_out, 0, "TX should be low during start bit");
}

#[test]
fn test_uart_tx_full_byte() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Initiate transmission
    dut.tx_data = 0xA5; // 10100101
    dut.tx_valid = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Track TX output during transmission
    let mut tx_bits = Vec::new();

    // Sample at middle of each bit period
    for _bit_num in 0..10 {
        // Start + 8 data + stop
        // Wait to middle of bit
        wait_cycles(&mut dut, CLKS_PER_BIT / 2);
        tx_bits.push(dut.tx_out);
        // Wait to next bit
        wait_cycles(&mut dut, CLKS_PER_BIT / 2);
    }

    // Verify bit pattern: start(0) + data(10100101 LSB first) + stop(1)
    // Data 0xA5 = 10100101, LSB first = 1,0,1,0,0,1,0,1
    assert_eq!(tx_bits[0], 0, "Start bit should be 0");
    assert_eq!(tx_bits[1], 1, "Data bit 0 should be 1");
    assert_eq!(tx_bits[2], 0, "Data bit 1 should be 0");
    assert_eq!(tx_bits[3], 1, "Data bit 2 should be 1");
    assert_eq!(tx_bits[4], 0, "Data bit 3 should be 0");
    assert_eq!(tx_bits[5], 0, "Data bit 4 should be 0");
    assert_eq!(tx_bits[6], 1, "Data bit 5 should be 1");
    assert_eq!(tx_bits[7], 0, "Data bit 6 should be 0");
    assert_eq!(tx_bits[8], 1, "Data bit 7 should be 1");
    assert_eq!(tx_bits[9], 1, "Stop bit should be 1");
}

#[test]
fn test_uart_tx_data_pattern() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Test with alternating pattern 0x55 = 01010101
    dut.tx_data = 0x55;
    dut.tx_valid = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Sample data bits
    let mut tx_bits = Vec::new();

    // Skip start bit
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Sample 8 data bits
    for _ in 0..8 {
        wait_cycles(&mut dut, CLKS_PER_BIT / 2);
        tx_bits.push(dut.tx_out);
        wait_cycles(&mut dut, CLKS_PER_BIT / 2);
    }

    // 0x55 = 01010101, LSB first = 1,0,1,0,1,0,1,0
    assert_eq!(tx_bits[0], 1, "Bit 0 should be 1");
    assert_eq!(tx_bits[1], 0, "Bit 1 should be 0");
    assert_eq!(tx_bits[2], 1, "Bit 2 should be 1");
    assert_eq!(tx_bits[3], 0, "Bit 3 should be 0");
    assert_eq!(tx_bits[4], 1, "Bit 4 should be 1");
    assert_eq!(tx_bits[5], 0, "Bit 5 should be 0");
    assert_eq!(tx_bits[6], 1, "Bit 6 should be 1");
    assert_eq!(tx_bits[7], 0, "Bit 7 should be 0");
}

#[test]
fn test_uart_tx_ready_signal() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Initially, TX should be ready
    assert_eq!(dut.tx_ready, 1, "TX should be ready initially");

    // Initiate transmission
    dut.tx_data = 0xAA;
    dut.tx_valid = 1;
    dut.eval();
    clock_cycle!(dut);

    // TX ready should go low during transmission
    assert_eq!(
        dut.tx_ready, 0,
        "TX ready should be low during transmission"
    );

    dut.tx_valid = 0;
    dut.eval();

    // Wait for transmission to complete
    wait_cycles(&mut dut, CLKS_PER_BIT * 10);

    // TX ready should be high again
    assert_eq!(
        dut.tx_ready, 1,
        "TX ready should be high after transmission"
    );
}

#[test]
fn test_uart_tx_back_to_back_no_idle_gap() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Ensure TX starts idle (line high for UART)
    assert_eq!(dut.tx_out, 1, "TX line should be idle high after reset");

    let first_byte: u8 = 0x55;
    let second_byte: u8 = 0xA3;

    // Handshake first byte into TX
    dut.tx_data = first_byte;
    dut.tx_valid = 1;
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Advance through start bit + 8 data bits
    wait_cycles(&mut dut, CLKS_PER_BIT * 9);

    // Traverse all but the last cycle of the stop bit
    wait_cycles(&mut dut, CLKS_PER_BIT - 1);

    // On the final stop-bit tick, TX should be high and ready for next byte
    assert_eq!(
        dut.tx_out, 1,
        "TX line should be high during the stop bit of the first frame"
    );
    assert_eq!(
        dut.tx_ready, 1,
        "TX should assert tx_ready on the final stop-bit tick for back-to-back transfer"
    );

    // Present the second byte exactly on the final stop-bit tick
    dut.tx_data = second_byte;
    dut.tx_valid = 1;
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Next cycle should be second start bit (low) with no extra idle cycle
    clock_cycle!(dut);
    assert_eq!(
        dut.tx_out, 0,
        "TX line should transition directly from first stop bit to second start bit"
    );
}

#[test]
fn test_uart_baud_timing() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Initiate transmission
    dut.tx_data = 0xFF;
    dut.tx_valid = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Measure timing of start bit
    let mut start_bit_cycles = 0;
    let mut last_tx = dut.tx_out;

    // Wait for start bit to end
    for _ in 0..(CLKS_PER_BIT + 50) {
        clock_cycle!(dut);
        if last_tx == 0 {
            start_bit_cycles += 1;
        }
        if last_tx == 0 && dut.tx_out == 1 {
            break;
        }
        last_tx = dut.tx_out;
    }

    // Start bit should last approximately CLKS_PER_BIT cycles
    // Allow ±2 cycles tolerance
    assert!(
        (CLKS_PER_BIT - 2..=CLKS_PER_BIT + 2).contains(&start_bit_cycles),
        "Start bit should last ~{} cycles, got {}",
        CLKS_PER_BIT,
        start_bit_cycles
    );
}

#[test]
fn test_uart_rx_idle() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // RX line stays high (idle)
    dut.rx_in = 1;

    // Run for many cycles
    for _ in 0..1000 {
        clock_cycle!(dut);
        // RX valid should stay low
        assert_eq!(dut.rx_valid, 0, "RX valid should stay low with idle line");
    }
}

#[test]
fn test_uart_rx_single_byte() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Receive byte 0x42
    let test_byte = 0x42;
    receive_byte(&mut dut, test_byte);

    // Give extra time for processing
    wait_cycles(&mut dut, 10);

    // RX valid should be high
    assert_eq!(
        dut.rx_valid, 1,
        "RX valid should be high after receiving byte"
    );

    // RX data should match
    assert_eq!(
        dut.rx_data, test_byte,
        "RX data should match transmitted byte"
    );

    // RX error should be low
    assert_eq!(dut.rx_error, 0, "RX error should be low for valid frame");
}

#[test]
fn test_uart_rx_valid_handshake() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Receive byte
    let test_byte = 0x7E;
    receive_byte(&mut dut, test_byte);
    wait_cycles(&mut dut, 10);

    // Verify RX valid is high
    assert_eq!(dut.rx_valid, 1, "RX valid should be high");
    assert_eq!(dut.rx_data, test_byte, "RX data should be correct");

    // Assert rx_ready to acknowledge
    dut.rx_ready = 1;
    dut.eval();
    clock_cycle!(dut);

    // RX valid should clear
    assert_eq!(dut.rx_valid, 0, "RX valid should clear after handshake");

    // Clear rx_ready
    dut.rx_ready = 0;
    dut.eval();
}

#[test]
fn test_uart_loopback_single_byte() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Start transmission
    let test_byte = 0x5A;
    dut.tx_data = test_byte;
    dut.tx_valid = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.tx_valid = 0;
    dut.eval();

    // Connect TX to RX (loopback)
    for _ in 0..(CLKS_PER_BIT * 12) {
        // Extra cycles for safety
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);

        // Check if we've received the byte
        if dut.rx_valid == 1 {
            break;
        }
    }

    // Verify reception
    assert_eq!(dut.rx_valid, 1, "RX valid should be high after loopback");
    assert_eq!(dut.rx_data, test_byte, "Loopback data should match TX data");
    assert_eq!(dut.rx_error, 0, "No framing error expected");
}

#[test]
fn test_uart_loopback_multiple_bytes() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    let test_bytes = [0x00, 0x55, 0xAA, 0xFF, 0x12, 0x34];

    for &test_byte in &test_bytes {
        // Wait for TX ready
        while dut.tx_ready == 0 {
            dut.rx_in = dut.tx_out;
            clock_cycle!(dut);
        }

        // Start transmission
        dut.tx_data = test_byte;
        dut.tx_valid = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.tx_valid = 0;
        dut.eval();

        // Loopback until received
        let mut timeout = CLKS_PER_BIT * 15;
        while dut.rx_valid == 0 && timeout > 0 {
            dut.rx_in = dut.tx_out;
            clock_cycle!(dut);
            timeout -= 1;
        }

        assert!(timeout > 0, "Timeout waiting for RX valid");
        assert_eq!(dut.rx_valid, 1, "RX valid should be high");
        assert_eq!(dut.rx_data, test_byte, "Byte 0x{:02X} mismatch", test_byte);

        // Acknowledge reception
        dut.rx_ready = 1;
        dut.eval();
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);
        dut.rx_ready = 0;
        dut.eval();

        // Wait for RX valid to clear
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);
    }
}

#[test]
fn test_uart_rx_framing_error() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Send byte with incorrect stop bit (keep low instead of high)
    let test_byte = 0x33;

    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8 {
        dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // INCORRECT stop bit (should be high, send low instead)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Give extra time for processing
    wait_cycles(&mut dut, 10);

    // Should detect framing error
    assert_eq!(
        dut.rx_error, 1,
        "RX error should be set for missing stop bit"
    );
}

#[test]
fn test_uart_rx_error_sticky() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Verify rx_error is initially low
    assert_eq!(dut.rx_error, 0, "RX error should be low after reset");

    // Send byte with framing error
    let test_byte = 0x42;

    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8 {
        dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // INCORRECT stop bit (should be high, send low instead)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Return line to idle
    dut.rx_in = 1;
    wait_cycles(&mut dut, 10);

    // Verify rx_error is set
    assert_eq!(
        dut.rx_error, 1,
        "RX error should be set after framing error"
    );

    // Wait many cycles - error should remain set (sticky)
    wait_cycles(&mut dut, 1000);
    assert_eq!(dut.rx_error, 1, "RX error should remain set (sticky)");

    // Receive a valid byte - error should still remain set
    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (LSB first) - send 0x55
    for i in 0..8 {
        dut.rx_in = if (0x55u8 >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Correct stop bit
    dut.rx_in = 1;
    wait_cycles(&mut dut, CLKS_PER_BIT);
    wait_cycles(&mut dut, 10);

    // Error should still be set even after valid reception
    assert_eq!(
        dut.rx_error, 1,
        "RX error should remain set even after valid byte reception"
    );
}

#[test]
fn test_uart_rx_error_clr() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Send byte with framing error to set rx_error
    let test_byte = 0x33;

    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8 {
        dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // INCORRECT stop bit
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Return line to idle
    dut.rx_in = 1;
    wait_cycles(&mut dut, 10);

    // Verify rx_error is set
    assert_eq!(
        dut.rx_error, 1,
        "RX error should be set after framing error"
    );

    // Assert rx_error_clr to clear the error
    dut.rx_error_clr = 1;
    dut.eval();
    clock_cycle!(dut);

    // Error should now be cleared
    assert_eq!(
        dut.rx_error, 0,
        "RX error should be cleared after rx_error_clr"
    );

    // De-assert rx_error_clr
    dut.rx_error_clr = 0;
    dut.eval();
    clock_cycle!(dut);

    // Error should stay cleared
    assert_eq!(
        dut.rx_error, 0,
        "RX error should remain cleared after rx_error_clr de-asserted"
    );
}

#[test]
fn test_uart_rx_overrun() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    // Apply reset
    reset_uart(&mut dut);

    // Receive first byte
    let first_byte = 0x42;
    receive_byte(&mut dut, first_byte);

    // Give extra time for processing
    wait_cycles(&mut dut, 10);

    // Verify first byte received correctly
    assert_eq!(dut.rx_valid, 1, "RX valid should be high after first byte");
    assert_eq!(dut.rx_data, first_byte, "First byte should be correct");
    assert_eq!(dut.rx_error, 0, "RX error should be low after valid byte");

    // Do NOT acknowledge the first byte (don't assert rx_ready)
    // Immediately receive a second byte - this should cause overrun
    let second_byte = 0x7E;

    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8 {
        dut.rx_in = if (second_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Valid stop bit
    dut.rx_in = 1;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Give extra time for processing
    wait_cycles(&mut dut, 10);

    // Verify overrun error was detected
    assert_eq!(dut.rx_error, 1, "RX error should be set due to overrun");

    // Original data should be preserved (second byte was dropped)
    assert_eq!(dut.rx_valid, 1, "RX valid should still be high");
    assert_eq!(
        dut.rx_data, first_byte,
        "Original byte should be preserved (second dropped)"
    );
}

#[test]
fn test_uart_rx_overrun_allows_simultaneous_read_and_new_byte() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    let first_byte = 0x42;
    let second_byte = 0x7E;

    // Receive first byte and keep it pending in output register
    receive_byte(&mut dut, first_byte);
    wait_cycles(&mut dut, 10);
    assert_eq!(dut.rx_valid, 1, "RX valid should hold first byte");
    assert_eq!(dut.rx_data, first_byte, "First byte should be present");

    // Begin second byte while first is still pending
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);
    for i in 0..8 {
        dut.rx_in = if (second_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Stop bit: consume first byte early in stop bit so the validation
    // can latch the second byte without overrun
    dut.rx_in = 1;
    wait_cycles(&mut dut, 10);

    // Pulse rx_ready to consume the first byte before midpoint validation
    dut.rx_ready = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.rx_ready = 0;
    dut.eval();
    clock_cycle!(dut);

    // Wait for stop bit validation to complete
    wait_cycles(&mut dut, CLKS_PER_BIT);
    wait_cycles(&mut dut, 10);

    assert_eq!(
        dut.rx_error, 0,
        "No overrun error expected when rx_ready is asserted during second-byte completion"
    );
    assert_eq!(dut.rx_valid, 1, "Second byte should be valid");
    assert_eq!(
        dut.rx_data, second_byte,
        "Second byte should replace first byte on simultaneous read/write"
    );
}

// ============================================================
// Tests for high-baud-rate reliability improvements
// ============================================================

// Derived timing constants for glitch injection
const CLKS_PER_SAMPLE: u32 = CLKS_PER_BIT / 16;

/// Helper: receive a byte with a single-sample-period glitch on a specific data bit.
/// The glitch inverts rx_in for CLKS_PER_SAMPLE cycles near the midpoint of the bit,
/// affecting at most 1 of the 3 majority voting samples.
fn receive_byte_with_data_glitch(dut: &mut Uart, data: u8, glitch_bit: u8) {
    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(dut, CLKS_PER_BIT);

    // Data bits (LSB first)
    for i in 0..8u8 {
        let correct_val: u8 = if (data >> i) & 1 == 1 { 1 } else { 0 };
        dut.rx_in = correct_val;

        if i == glitch_bit {
            // Drive correct value until glitch window
            let glitch_start = CLKS_PER_BIT / 2 - CLKS_PER_SAMPLE / 2;
            let glitch_duration = CLKS_PER_SAMPLE;
            let remaining = CLKS_PER_BIT - glitch_start - glitch_duration;

            wait_cycles(dut, glitch_start);
            // Inject glitch: invert the value for one sample period
            dut.rx_in = 1 - correct_val;
            wait_cycles(dut, glitch_duration);
            // Restore correct value
            dut.rx_in = correct_val;
            wait_cycles(dut, remaining);
        } else {
            wait_cycles(dut, CLKS_PER_BIT);
        }
    }

    // Stop bit (high)
    dut.rx_in = 1;
    wait_cycles(dut, CLKS_PER_BIT);
}

#[test]
fn test_uart_rx_majority_vote_data_glitch() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Test glitch rejection on multiple data bits with different byte patterns
    let test_cases: &[(u8, u8)] = &[
        (0xA5, 3), // Glitch on bit 3 (value 0, glitch to 1)
        (0x55, 0), // Glitch on bit 0 (value 1, glitch to 0)
        (0xFF, 5), // Glitch on bit 5 (value 1, glitch to 0)
        (0x00, 7), // Glitch on bit 7 (value 0, glitch to 1)
    ];

    for &(test_byte, glitch_bit) in test_cases {
        // Receive byte with glitch
        receive_byte_with_data_glitch(&mut dut, test_byte, glitch_bit);
        wait_cycles(&mut dut, 10);

        // Majority voting should filter the glitch
        assert_eq!(
            dut.rx_valid, 1,
            "RX valid should be high for byte 0x{:02X} with glitch on bit {}",
            test_byte, glitch_bit
        );
        assert_eq!(
            dut.rx_data, test_byte,
            "RX data should match 0x{:02X} despite glitch on bit {}",
            test_byte, glitch_bit
        );
        assert_eq!(
            dut.rx_error, 0,
            "No error expected for byte 0x{:02X} with glitch on bit {}",
            test_byte, glitch_bit
        );

        // Acknowledge and prepare for next byte
        dut.rx_ready = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.rx_ready = 0;
        dut.eval();
        clock_cycle!(dut);
    }
}

#[test]
fn test_uart_rx_majority_vote_start_bit_glitch() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    let test_byte = 0x42;

    // Start bit (low) with a brief high glitch near the midpoint
    dut.rx_in = 0;
    let glitch_start = CLKS_PER_BIT / 2 - CLKS_PER_SAMPLE / 2;
    let glitch_duration = CLKS_PER_SAMPLE;
    let remaining = CLKS_PER_BIT - glitch_start - glitch_duration;

    wait_cycles(&mut dut, glitch_start);
    dut.rx_in = 1; // Glitch high during start bit
    wait_cycles(&mut dut, glitch_duration);
    dut.rx_in = 0; // Restore low
    wait_cycles(&mut dut, remaining);

    // Data bits (normal, no glitches)
    for i in 0..8 {
        dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Stop bit (high)
    dut.rx_in = 1;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    wait_cycles(&mut dut, 10);

    // Majority voting should have validated the start bit despite the glitch
    assert_eq!(
        dut.rx_valid, 1,
        "RX valid should be high - start bit glitch should be filtered"
    );
    assert_eq!(
        dut.rx_data, test_byte,
        "RX data should be correct despite start bit glitch"
    );
    assert_eq!(
        dut.rx_error, 0,
        "No error expected - start bit glitch was filtered"
    );
}

#[test]
fn test_uart_rx_majority_vote_stop_bit_glitch() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    let test_byte = 0x7E;

    // Start bit (low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Data bits (normal)
    for i in 0..8 {
        dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Stop bit (high) with a brief low glitch near the midpoint
    dut.rx_in = 1;
    let glitch_start = CLKS_PER_BIT / 2 - CLKS_PER_SAMPLE / 2;
    let glitch_duration = CLKS_PER_SAMPLE;
    let remaining = CLKS_PER_BIT - glitch_start - glitch_duration;

    wait_cycles(&mut dut, glitch_start);
    dut.rx_in = 0; // Glitch low during stop bit
    wait_cycles(&mut dut, glitch_duration);
    dut.rx_in = 1; // Restore high
    wait_cycles(&mut dut, remaining);

    wait_cycles(&mut dut, 10);

    // Majority voting should have validated the stop bit despite the glitch
    assert_eq!(
        dut.rx_valid, 1,
        "RX valid should be high - stop bit glitch should be filtered"
    );
    assert_eq!(
        dut.rx_data, test_byte,
        "RX data should be correct despite stop bit glitch"
    );
    assert_eq!(
        dut.rx_error, 0,
        "No framing error expected - stop bit glitch was filtered"
    );
}

#[test]
fn test_uart_rx_falling_edge_detection() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Create a framing error that leaves the line low
    // Start bit
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Send 8 data bits (0x00)
    for _ in 0..8 {
        dut.rx_in = 0;
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Bad stop bit (keep low) - causes framing error
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Give time for processing
    wait_cycles(&mut dut, 10);
    assert_eq!(dut.rx_error, 1, "Should have framing error");

    // Clear error
    dut.rx_error_clr = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.rx_error_clr = 0;
    dut.eval();

    // Line is still low - with falling-edge detection, this should NOT
    // trigger a new start bit detection (no falling edge while line is held low)
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT * 2);

    // No new byte should be received
    assert_eq!(
        dut.rx_valid, 0,
        "No byte should be received while line is held low (no falling edge)"
    );
    assert_eq!(
        dut.rx_error, 0,
        "No new error should be generated without falling edge"
    );

    // Now bring line high and then back low - this creates a true falling edge
    dut.rx_in = 1;
    wait_cycles(&mut dut, 10); // Brief idle period

    // Now send a valid byte - falling edge should be detected
    let test_byte = 0x55;
    receive_byte(&mut dut, test_byte);
    wait_cycles(&mut dut, 10);

    assert_eq!(
        dut.rx_valid, 1,
        "RX valid should be high after proper falling edge"
    );
    assert_eq!(
        dut.rx_data, test_byte,
        "Data should be correct after proper falling edge"
    );
}

#[test]
fn test_uart_rx_full_stop_bit_timing() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Send two bytes back-to-back with minimal gap between them.
    // The full stop bit wait ensures the receiver completes the stop bit
    // before looking for the next start bit.
    let byte1 = 0xAA;
    let byte2 = 0x55;

    // First byte
    receive_byte(&mut dut, byte1);
    wait_cycles(&mut dut, 10);

    assert_eq!(dut.rx_valid, 1, "First byte should be received");
    assert_eq!(dut.rx_data, byte1, "First byte data should match");
    assert_eq!(dut.rx_error, 0, "No error on first byte");

    // Acknowledge first byte
    dut.rx_ready = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.rx_ready = 0;
    dut.eval();

    // Minimal idle gap before second byte
    wait_cycles(&mut dut, 5);

    // Second byte immediately
    receive_byte(&mut dut, byte2);
    wait_cycles(&mut dut, 10);

    assert_eq!(dut.rx_valid, 1, "Second byte should be received");
    assert_eq!(dut.rx_data, byte2, "Second byte data should match");
    assert_eq!(dut.rx_error, 0, "No error on second byte");
}

#[test]
fn test_uart_rx_accepts_early_next_start_after_stop_midpoint() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    let byte1 = 0xA6;
    let byte2 = 0x39;
    let rx_mid = (CLKS_PER_BIT * 8) / 16;

    // First byte: start + data bits
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);
    for i in 0..8 {
        dut.rx_in = if (byte1 >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }

    // Hold stop high only until just after midpoint, then start next byte early
    dut.rx_in = 1;
    wait_cycles(&mut dut, rx_mid + 2);
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT);

    // Acknowledge first byte during second-byte data phase
    dut.rx_ready = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.rx_ready = 0;
    dut.eval();

    // Second byte: data bits + normal stop bit
    for i in 0..8 {
        dut.rx_in = if (byte2 >> i) & 1 == 1 { 1 } else { 0 };
        wait_cycles(&mut dut, CLKS_PER_BIT);
    }
    dut.rx_in = 1;
    wait_cycles(&mut dut, CLKS_PER_BIT + 10);

    assert_eq!(dut.rx_error, 0, "No framing/overrun error expected");
    assert_eq!(dut.rx_valid, 1, "Second byte should be received");
    assert_eq!(
        dut.rx_data, byte2,
        "Receiver should detect early next start bit and capture second byte"
    );
}

#[test]
fn test_uart_rx_consecutive_bytes_no_gap() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Send multiple bytes with only 1 stop bit period between them
    // Tests that full stop bit timing correctly handles tight byte spacing
    let test_bytes: &[u8] = &[0x00, 0xFF, 0xA5, 0x5A, 0x01, 0x80];

    for &test_byte in test_bytes {
        // Start bit
        dut.rx_in = 0;
        wait_cycles(&mut dut, CLKS_PER_BIT);

        // Data bits (LSB first)
        for i in 0..8 {
            dut.rx_in = if (test_byte >> i) & 1 == 1 { 1 } else { 0 };
            wait_cycles(&mut dut, CLKS_PER_BIT);
        }

        // Stop bit - only 1 bit period (no extra idle gap)
        dut.rx_in = 1;
        wait_cycles(&mut dut, CLKS_PER_BIT);

        // Give a few extra cycles for the FSM to process
        wait_cycles(&mut dut, 10);

        // Verify reception
        assert_eq!(
            dut.rx_valid, 1,
            "RX valid should be high for byte 0x{:02X}",
            test_byte
        );
        assert_eq!(
            dut.rx_data, test_byte,
            "Data mismatch for byte 0x{:02X}",
            test_byte
        );
        assert_eq!(
            dut.rx_error, 0,
            "No error expected for byte 0x{:02X}",
            test_byte
        );

        // Acknowledge
        dut.rx_ready = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.rx_ready = 0;
        dut.eval();
        clock_cycle!(dut);
    }
}

#[test]
fn test_uart_rx_glitch_does_not_trigger_false_start() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Send a brief low pulse that's too short to be a valid start bit.
    // The pulse is long enough to pass the synchronizer (>2 cycles) but
    // short enough that the majority vote at sample 8 sees it as high.
    // With 16x oversampling, sample 8 is at about half the bit period.
    // A pulse of only CLKS_PER_BIT/4 cycles (quarter of a bit) should not
    // produce a valid start bit via majority voting.
    dut.rx_in = 0;
    wait_cycles(&mut dut, CLKS_PER_BIT / 4);
    dut.rx_in = 1;
    wait_cycles(&mut dut, CLKS_PER_BIT * 2);

    // No byte should be received - the short pulse should be rejected
    assert_eq!(
        dut.rx_valid, 0,
        "Short pulse should not trigger start bit detection"
    );
    assert_eq!(dut.rx_error, 0, "No error from rejected short pulse");
}

#[test]
fn test_uart_loopback_with_tight_spacing() {
    let runtime = uart_runtime();
    let mut dut = runtime
        .create_model_simple::<Uart>()
        .expect("Failed to create UART model");

    reset_uart(&mut dut);

    // Loopback test with multiple bytes sent as fast as possible
    // Verifies that full stop bit timing + falling-edge detection work
    // correctly under continuous TX/RX operation
    let test_bytes = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

    for &test_byte in &test_bytes {
        // Wait for TX ready
        while dut.tx_ready == 0 {
            dut.rx_in = dut.tx_out;
            clock_cycle!(dut);
        }

        // Start transmission
        dut.tx_data = test_byte;
        dut.tx_valid = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.tx_valid = 0;
        dut.eval();

        // Loopback until received
        let mut timeout = CLKS_PER_BIT * 15;
        while dut.rx_valid == 0 && timeout > 0 {
            dut.rx_in = dut.tx_out;
            clock_cycle!(dut);
            timeout -= 1;
        }

        assert!(timeout > 0, "Timeout for byte 0x{:02X}", test_byte);
        assert_eq!(dut.rx_valid, 1, "RX valid for 0x{:02X}", test_byte);
        assert_eq!(
            dut.rx_data, test_byte,
            "Loopback mismatch for 0x{:02X}",
            test_byte
        );
        assert_eq!(dut.rx_error, 0, "No error for 0x{:02X}", test_byte);

        // Acknowledge
        dut.rx_ready = 1;
        dut.eval();
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);
        dut.rx_ready = 0;
        dut.eval();
        dut.rx_in = dut.tx_out;
        clock_cycle!(dut);
    }
}

#[test]
fn test_uart_bidirectional_end_to_end_at_1m_baud() {
    let runtime = uart_1m_runtime();
    let mut uart_a = runtime
        .create_model_simple::<Uart1MBaud>()
        .expect("Failed to create UART A model");
    let mut uart_b = runtime
        .create_model_simple::<Uart1MBaud>()
        .expect("Failed to create UART B model");

    let reset_uart_1m = |dut: &mut Uart1MBaud| {
        dut.rst = 1;
        dut.tx_valid = 0;
        dut.rx_ready = 0;
        dut.rx_error_clr = 0;
        dut.rx_in = 1;
        clock_cycle!(dut);
        dut.rst = 0;
        clock_cycle!(dut);
    };

    reset_uart_1m(&mut uart_a);
    reset_uart_1m(&mut uart_b);

    let step_link = |a: &mut Uart1MBaud, b: &mut Uart1MBaud| {
        a.rx_in = b.tx_out;
        b.rx_in = a.tx_out;
        clock_cycle!(a);
        clock_cycle!(b);
    };

    let transfer_byte = |src: &mut Uart1MBaud, dst: &mut Uart1MBaud, data: u8| {
        while src.tx_ready == 0 {
            step_link(src, dst);
        }

        src.tx_data = data;
        src.tx_valid = 1;
        src.eval();
        step_link(src, dst);
        src.tx_valid = 0;
        src.eval();

        let mut timeout = CLKS_PER_BIT_1M * UART_BYTE_TIMEOUT_BITS;
        while dst.rx_valid == 0 && timeout > 0 {
            step_link(src, dst);
            timeout -= 1;
        }

        assert!(timeout > 0, "Timeout waiting for byte 0x{:02X}", data);
        assert_eq!(dst.rx_valid, 1, "RX valid should assert for 0x{:02X}", data);
        assert_eq!(dst.rx_data, data, "RX data corrupted for 0x{:02X}", data);
        assert_eq!(dst.rx_error, 0, "RX error asserted for 0x{:02X}", data);

        dst.rx_ready = 1;
        dst.eval();
        step_link(src, dst);
        dst.rx_ready = 0;
        dst.eval();
        step_link(src, dst);
    };

    let a_to_b = [0x00, 0x55, 0xA5, 0xFF, 0x3C];
    let b_to_a = [0x12, 0x34, 0x78, 0x9A, 0xE1];

    for (&tx_a, &tx_b) in a_to_b.iter().zip(b_to_a.iter()) {
        transfer_byte(&mut uart_a, &mut uart_b, tx_a);
        transfer_byte(&mut uart_b, &mut uart_a, tx_b);
    }
}

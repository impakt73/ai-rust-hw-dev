use riscv_core::{create_uart_runtime, Uart};

// UART timing (based on 50MHz clock, 115200 baud)
// CLKS_PER_BIT = 50_000_000 / 115200 ≈ 434
const CLKS_PER_BIT: u32 = 434;

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
    dut.rst_n = 0;
    dut.tx_valid = 0;
    dut.rx_ready = 0;
    dut.rx_error_clr = 0;
    dut.rx_in = 1; // RX line idle high
    clock_cycle!(dut);
    dut.rst_n = 1;
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
fn test_uart_baud_timing() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
        start_bit_cycles >= CLKS_PER_BIT - 2 && start_bit_cycles <= CLKS_PER_BIT + 2,
        "Start bit should last ~{} cycles, got {}",
        CLKS_PER_BIT,
        start_bit_cycles
    );
}

#[test]
fn test_uart_rx_idle() {
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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
    let runtime = create_uart_runtime().expect("Failed to create UART runtime");
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

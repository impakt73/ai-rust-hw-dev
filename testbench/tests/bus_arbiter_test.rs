use riscv_core::{create_bus_arbiter_runtime, BusArbiter};

// Clock cycle macro for bus_arbiter tests
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

// Helper function to reset the arbiter
fn reset_arbiter(dut: &mut BusArbiter) {
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

// Helper function to set CPU request inputs
fn set_cpu_request(dut: &mut BusArbiter, addr: u32, wdata: u32, we: u8, size: u8, req: u8) {
    dut.cpu_addr = addr;
    dut.cpu_wdata = wdata;
    dut.cpu_we = we;
    dut.cpu_size = size;
    dut.cpu_req = req;
}

// Helper function to set Host request inputs
fn set_host_request(dut: &mut BusArbiter, addr: u32, wdata: u32, we: u8, size: u8, req: u8) {
    dut.host_addr = addr;
    dut.host_wdata = wdata;
    dut.host_we = we;
    dut.host_size = size;
    dut.host_req = req;
}

// Helper function to simulate bus response
fn respond_to_bus(dut: &mut BusArbiter, rdata: u32, ready: u8) {
    dut.bus_rdata = rdata;
    dut.bus_ready = ready;
}

// Test 1: Basic Idle State - No requests
#[test]
fn test_arbiter_idle_state() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // No requests from either master
    set_cpu_request(&mut dut, 0, 0, 0, 0, 0);
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Verify idle state outputs
    assert_eq!(dut.cpu_ready, 0, "cpu_ready should be LOW in IDLE");
    assert_eq!(dut.host_ready, 0, "host_ready should be LOW in IDLE");
    assert_eq!(dut.bus_req, 0, "bus_req should be LOW in IDLE");
}

// Test 2: CPU-only transaction
#[test]
fn test_arbiter_cpu_only_transaction() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // CPU initiates a word write to address 0x1000
    set_cpu_request(&mut dut, 0x1000, 0xDEADBEEF, 1, 0b10, 1);
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Verify CPU grant and bus outputs
    assert_eq!(dut.bus_addr, 0x1000, "bus_addr should match CPU addr");
    assert_eq!(
        dut.bus_wdata, 0xDEADBEEF,
        "bus_wdata should match CPU wdata"
    );
    assert_eq!(dut.bus_we, 1, "bus_we should be HIGH for write");
    assert_eq!(dut.bus_size, 0b10, "bus_size should match CPU size");
    assert_eq!(dut.bus_req, 1, "bus_req should be HIGH");

    // Simulate bus completion
    respond_to_bus(&mut dut, 0x12345678, 1);
    clock_cycle!(dut);

    // Verify CPU receives response
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be HIGH on completion");
    assert_eq!(
        dut.cpu_rdata, 0x12345678,
        "cpu_rdata should match bus_rdata"
    );

    // Release CPU request
    set_cpu_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);
    clock_cycle!(dut);

    // Should return to idle
    assert_eq!(dut.cpu_ready, 0, "cpu_ready should be LOW after release");
    assert_eq!(dut.bus_req, 0, "bus_req should be LOW in idle");
}

// Test 3: Host-only transaction
#[test]
fn test_arbiter_host_only_transaction() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // Host initiates a word read from address 0x50000000
    set_cpu_request(&mut dut, 0, 0, 0, 0, 0);
    set_host_request(&mut dut, 0x50000000, 0, 0, 0b10, 1);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Verify Host grant and bus outputs
    assert_eq!(dut.bus_addr, 0x50000000, "bus_addr should match Host addr");
    assert_eq!(dut.bus_we, 0, "bus_we should be LOW for read");
    assert_eq!(dut.bus_size, 0b10, "bus_size should match Host size");
    assert_eq!(dut.bus_req, 1, "bus_req should be HIGH");

    // Simulate bus completion
    respond_to_bus(&mut dut, 0xAABBCCDD, 1);
    clock_cycle!(dut);

    // Verify Host receives response
    assert_eq!(dut.host_ready, 1, "host_ready should be HIGH on completion");
    assert_eq!(
        dut.host_rdata, 0xAABBCCDD,
        "host_rdata should match bus_rdata"
    );

    // Release Host request
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);
    clock_cycle!(dut);

    // Should return to idle
    assert_eq!(dut.host_ready, 0, "host_ready should be LOW after release");
    assert_eq!(dut.bus_req, 0, "bus_req should be LOW in idle");
}

// Test 4: Host priority over CPU (both request simultaneously from IDLE)
#[test]
fn test_arbiter_host_priority_from_idle() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // Both CPU and Host request simultaneously from IDLE
    set_cpu_request(&mut dut, 0x2000, 0x11111111, 1, 0b10, 1);
    set_host_request(&mut dut, 0x3000, 0x22222222, 1, 0b10, 1);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Host should win priority
    assert_eq!(
        dut.bus_addr, 0x3000,
        "bus_addr should match Host addr (priority)"
    );
    assert_eq!(
        dut.bus_wdata, 0x22222222,
        "bus_wdata should match Host wdata"
    );
    assert_eq!(dut.host_ready, 0, "host_ready should be LOW (pending)");
    assert_eq!(
        dut.cpu_ready, 0,
        "cpu_ready should be LOW (waiting for Host)"
    );

    // Complete Host transaction
    respond_to_bus(&mut dut, 0xAAAAAAAA, 1);
    clock_cycle!(dut);

    assert_eq!(dut.host_ready, 1, "host_ready should be HIGH");
    assert_eq!(
        dut.cpu_ready, 0,
        "cpu_ready should still be LOW (CPU waiting)"
    );
}

// Test 5: Host preempts CPU on next transaction
#[test]
fn test_arbiter_host_preempts_cpu() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // CPU starts a transaction
    set_cpu_request(&mut dut, 0x4000, 0x33333333, 1, 0b10, 1);
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // CPU granted
    assert_eq!(dut.bus_addr, 0x4000, "bus_addr should match CPU addr");

    // Before completing CPU transaction, Host also requests
    set_host_request(&mut dut, 0x6000, 0x55555555, 1, 0b10, 1);
    respond_to_bus(&mut dut, 0, 0);
    clock_cycle!(dut);

    // CPU transaction still active (Host waiting, CPU still has grant because cpu_req=1)
    assert_eq!(dut.bus_addr, 0x4000, "CPU transaction still active");

    // Complete CPU transaction (bus_ready HIGH) — CPU sees cpu_ready=1
    // CPU should then drop its request on the next cycle
    respond_to_bus(&mut dut, 0xBBBBBBBB, 1);
    clock_cycle!(dut);

    // CPU still has grant this cycle (cpu_req was still 1 on previous edge)
    // CPU sees cpu_ready=1 and completes its transaction
    assert_eq!(
        dut.cpu_ready, 1,
        "cpu_ready should be HIGH (transaction completing)"
    );

    // CPU drops its request after seeing ready=1, host_req still asserted
    // Arbiter will transition to HOST_GRANT on next clock edge
    set_cpu_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);
    clock_cycle!(dut);

    // Host should have the bus now
    assert_eq!(
        dut.cpu_ready, 0,
        "cpu_ready should be LOW (grant switched to Host)"
    );
    assert_eq!(
        dut.bus_addr, 0x6000,
        "bus_addr should match Host addr (preemption)"
    );
    assert_eq!(
        dut.bus_wdata, 0x55555555,
        "bus_wdata should match Host wdata"
    );
}

// Test 6: Consecutive CPU transactions (no Host interference)
#[test]
fn test_arbiter_consecutive_cpu_transactions() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // First CPU transaction
    set_cpu_request(&mut dut, 0x7000, 0x66666666, 1, 0b10, 1);
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);
    assert_eq!(dut.bus_addr, 0x7000, "bus_addr should match CPU addr");

    // Complete first transaction
    respond_to_bus(&mut dut, 0xCCCCCCCC, 1);
    clock_cycle!(dut);
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be HIGH");

    // Second CPU transaction (CPU still requesting, no Host)
    set_cpu_request(&mut dut, 0x8000, 0x77777777, 1, 0b10, 1);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Should stay in CPU grant
    assert_eq!(
        dut.bus_addr, 0x8000,
        "bus_addr should match second CPU addr"
    );

    // Complete second transaction
    respond_to_bus(&mut dut, 0xDDDDDDDD, 1);
    clock_cycle!(dut);
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be HIGH");
}

// Test 7: Variable latency bus response (multi-cycle wait)
#[test]
fn test_arbiter_variable_latency() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // CPU initiates request
    set_cpu_request(&mut dut, 0x9000, 0x88888888, 1, 0b10, 1);
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);
    assert_eq!(dut.bus_req, 1, "bus_req should be HIGH");

    // Simulate slow bus (ready stays LOW for multiple cycles)
    for _ in 0..5 {
        respond_to_bus(&mut dut, 0, 0);
        clock_cycle!(dut);
        assert_eq!(dut.cpu_ready, 0, "cpu_ready should be LOW while waiting");
    }

    // Bus finally responds
    respond_to_bus(&mut dut, 0xEEEEEEEE, 1);
    clock_cycle!(dut);

    assert_eq!(
        dut.cpu_ready, 1,
        "cpu_ready should be HIGH after delayed response"
    );
    assert_eq!(
        dut.cpu_rdata, 0xEEEEEEEE,
        "cpu_rdata should match bus_rdata"
    );
}

// Test 8: Request must be held until bus responds
#[test]
fn test_arbiter_request_must_be_held() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_arbiter(&mut dut);

    // Host starts transaction
    set_cpu_request(&mut dut, 0, 0, 0, 0, 0);
    set_host_request(&mut dut, 0xA000, 0x99999999, 0, 0b10, 1);
    respond_to_bus(&mut dut, 0, 0);

    clock_cycle!(dut);

    // Grant to Host
    assert_eq!(dut.bus_addr, 0xA000, "bus_addr should match Host addr");

    // Simulate multi-cycle bus access - Host keeps req HIGH
    for i in 0..3 {
        respond_to_bus(&mut dut, 0, 0);
        clock_cycle!(dut);
        assert_eq!(dut.bus_req, 1, "bus_req should stay HIGH (cycle {})", i);
        assert_eq!(dut.host_ready, 0, "host_ready should be LOW (pending)");
    }

    // Bus finally responds (Host still holding req)
    respond_to_bus(&mut dut, 0xFFFFFFFF, 1);
    clock_cycle!(dut);

    // Transaction completes
    assert_eq!(dut.host_ready, 1, "host_ready should be HIGH on completion");
    assert_eq!(
        dut.host_rdata, 0xFFFFFFFF,
        "host_rdata should match bus_rdata"
    );

    // Release Host request
    set_host_request(&mut dut, 0, 0, 0, 0, 0);
    respond_to_bus(&mut dut, 0, 0);
    clock_cycle!(dut);

    // Should return to IDLE
    assert_eq!(dut.host_ready, 0, "host_ready should be LOW after release");
    assert_eq!(dut.bus_req, 0, "bus_req should be LOW in idle");
}

// ============================================================
// CLOCK PERIPHERAL TESTS (in separate module)
// ============================================================

mod clock_peripheral {
    use riscv_core::{create_clock_peripheral_runtime, ClockPeripheral};
    use riscv_shared::bus::{
        CLOCK_ELAPSED_MS_OFFSET, CLOCK_ELAPSED_S_OFFSET, CLOCK_ELAPSED_US_OFFSET,
    };

    // Clock Peripheral Register Offsets (from riscv_shared)
    const ELAPSED_US: u32 = CLOCK_ELAPSED_US_OFFSET;
    const ELAPSED_MS: u32 = CLOCK_ELAPSED_MS_OFFSET;
    const ELAPSED_S: u32 = CLOCK_ELAPSED_S_OFFSET;

    // Access size encodings
    const SIZE_WORD: u8 = 0b10;

    // Clock cycle macro for clock peripheral tests
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

    /// Helper function to read a register from the clock peripheral
    fn read_register(dut: &mut ClockPeripheral, offset: u32) -> u32 {
        dut.addr = offset;
        dut.req = 1;
        dut.we = 0;
        dut.size = SIZE_WORD;
        dut.eval();
        let value = dut.rdata;
        dut.req = 0;
        dut.eval();
        value
    }

    /// Helper function to apply reset to the clock peripheral
    fn reset_clock_peripheral(dut: &mut ClockPeripheral) {
        dut.rst_n = 0;
        dut.we = 0;
        dut.req = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        // Don't do a clock cycle here - let tests control when the first cycle happens
        // This way, immediately after reset, the counter reads 0
    }

    #[test]
    fn test_clock_peripheral_reset_state() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // Verify ready signal is asserted (single-cycle peripheral)
        assert_eq!(dut.ready, 1, "Clock peripheral should be ready");

        // All time counters should be 0 after reset
        let elapsed_us = read_register(&mut dut, ELAPSED_US);
        let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
        let elapsed_s = read_register(&mut dut, ELAPSED_S);

        assert_eq!(elapsed_us, 0, "ELAPSED_US should be 0 after reset");
        assert_eq!(elapsed_ms, 0, "ELAPSED_MS should be 0 after reset");
        assert_eq!(elapsed_s, 0, "ELAPSED_S should be 0 after reset");
    }

    #[test]
    fn test_clock_peripheral_always_ready() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // Verify ready stays high for multiple cycles
        for i in 0..100 {
            assert_eq!(dut.ready, 1, "Ready should stay high at cycle {}", i);
            clock_cycle!(dut);
        }
    }

    #[test]
    fn test_clock_peripheral_microseconds() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // At 1 MHz (default CLK_FREQ_HZ), 1 cycle = 1 microsecond
        // Run for a few cycles and check that microseconds increment correctly

        // Initial value should be 0
        let us_0 = read_register(&mut dut, ELAPSED_US);
        assert_eq!(us_0, 0, "Initial ELAPSED_US should be 0");

        // Run 1 cycle (should be 1 microsecond)
        clock_cycle!(dut);
        let us_1 = read_register(&mut dut, ELAPSED_US);
        assert_eq!(us_1, 1, "After 1 cycle at 1MHz, should be 1 us");

        // Run 1 more cycle (should be 2 microseconds)
        clock_cycle!(dut);
        let us_2 = read_register(&mut dut, ELAPSED_US);
        assert_eq!(us_2, 2, "After 2 cycles at 1MHz, should be 2 us");

        // Run 8 more cycles (total 10 cycles = 10 microseconds)
        for _ in 0..8 {
            clock_cycle!(dut);
        }
        let us_10 = read_register(&mut dut, ELAPSED_US);
        assert_eq!(us_10, 10, "After 10 cycles at 1MHz, should be 10 us");
    }

    #[test]
    fn test_clock_peripheral_milliseconds() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // At 1 MHz, 1000 cycles = 1 millisecond

        // Initial value should be 0
        let ms_0 = read_register(&mut dut, ELAPSED_MS);
        assert_eq!(ms_0, 0, "Initial ELAPSED_MS should be 0");

        // Run 1000 cycles (should be 1 millisecond)
        for _ in 0..1000 {
            clock_cycle!(dut);
        }
        let ms_1 = read_register(&mut dut, ELAPSED_MS);
        assert_eq!(ms_1, 1, "After 1000 cycles at 1MHz, should be 1 ms");

        // Run 9000 more cycles (total 10000 cycles = 10 milliseconds)
        for _ in 0..9000 {
            clock_cycle!(dut);
        }
        let ms_10 = read_register(&mut dut, ELAPSED_MS);
        assert_eq!(ms_10, 10, "After 10000 cycles at 1MHz, should be 10 ms");
    }

    #[test]
    fn test_clock_peripheral_seconds() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // At 1 MHz, 1,000,000 cycles = 1 second

        // Initial value should be 0
        let s_0 = read_register(&mut dut, ELAPSED_S);
        assert_eq!(s_0, 0, "Initial ELAPSED_S should be 0");

        // Run 500,000 cycles (should be 0 seconds still)
        for _ in 0..500_000 {
            clock_cycle!(dut);
        }
        let s_500k = read_register(&mut dut, ELAPSED_S);
        assert_eq!(
            s_500k, 0,
            "After 500,000 cycles at 1MHz, should still be 0 s"
        );

        // Run 500,000 more cycles (total 1,000,000 cycles = 1 second)
        for _ in 0..500_000 {
            clock_cycle!(dut);
        }
        let s_1m = read_register(&mut dut, ELAPSED_S);
        assert_eq!(s_1m, 1, "After 1,000,000 cycles at 1MHz, should be 1 s");

        // Run 1,000,000 more cycles (total 2,000,000 cycles = 2 seconds)
        for _ in 0..1_000_000 {
            clock_cycle!(dut);
        }
        let s_2m = read_register(&mut dut, ELAPSED_S);
        assert_eq!(s_2m, 2, "After 2,000,000 cycles at 1MHz, should be 2 s");
    }

    #[test]
    fn test_clock_peripheral_all_counters_increment() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // Run 2,000,000 cycles (2 seconds at 1 MHz)
        for _ in 0..2_000_000 {
            clock_cycle!(dut);
        }

        // Read all counters
        let elapsed_us = read_register(&mut dut, ELAPSED_US);
        let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
        let elapsed_s = read_register(&mut dut, ELAPSED_S);

        // Verify all counters have incremented correctly
        assert_eq!(
            elapsed_us, 2_000_000,
            "After 2,000,000 cycles at 1MHz, should be 2,000,000 us"
        );
        assert_eq!(
            elapsed_ms, 2000,
            "After 2,000,000 cycles at 1MHz, should be 2000 ms"
        );
        assert_eq!(
            elapsed_s, 2,
            "After 2,000,000 cycles at 1MHz, should be 2 s"
        );
    }

    #[test]
    fn test_clock_peripheral_read_only() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // Run a few cycles to get non-zero values
        for _ in 0..10 {
            clock_cycle!(dut);
        }

        // Read current values
        let us_before = read_register(&mut dut, ELAPSED_US);
        let ms_before = read_register(&mut dut, ELAPSED_MS);
        let s_before = read_register(&mut dut, ELAPSED_S);

        // Attempt to write to each register (should be ignored)
        dut.addr = ELAPSED_US;
        dut.wdata = 0xDEADBEEF;
        dut.we = 1;
        dut.req = 1;
        dut.size = SIZE_WORD;
        dut.eval();
        clock_cycle!(dut);
        dut.we = 0;
        dut.req = 0;

        dut.addr = ELAPSED_MS;
        dut.wdata = 0xCAFEBABE;
        dut.we = 1;
        dut.req = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.we = 0;
        dut.req = 0;

        dut.addr = ELAPSED_S;
        dut.wdata = 0x12345678;
        dut.we = 1;
        dut.req = 1;
        dut.eval();
        clock_cycle!(dut);
        dut.we = 0;
        dut.req = 0;

        // Read values after write attempts
        // They should have incremented by the clock cycles, not been overwritten
        let us_after = read_register(&mut dut, ELAPSED_US);
        let ms_after = read_register(&mut dut, ELAPSED_MS);
        let s_after = read_register(&mut dut, ELAPSED_S);

        // Values should have incremented (3 extra cycles from write attempts)
        // At 1 MHz: 3 cycles = 3 us, not enough for 1 ms
        assert_eq!(
            us_after,
            us_before + 3,
            "ELAPSED_US should increment, not be overwritten"
        );
        assert_eq!(
            ms_after, ms_before,
            "ELAPSED_MS should be same (not enough cycles for 1ms)"
        );
        assert_eq!(
            s_after, s_before,
            "ELAPSED_S should be same (not enough cycles for 1s)"
        );

        // Specifically check they were NOT set to the attempted write values
        assert_ne!(us_after, 0xDEADBEEF, "ELAPSED_US should not be writable");
        assert_ne!(ms_after, 0xCAFEBABE, "ELAPSED_MS should not be writable");
        assert_ne!(s_after, 0x12345678, "ELAPSED_S should not be writable");
    }

    #[test]
    fn test_clock_peripheral_unmapped_register() {
        let runtime =
            create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
        let mut dut = runtime
            .create_model_simple::<ClockPeripheral>()
            .expect("Failed to create clock peripheral model");

        // Apply reset
        reset_clock_peripheral(&mut dut);

        // Try to read from an unmapped register offset (e.g., 0x0C)
        let unmapped = read_register(&mut dut, 0x0C);
        assert_eq!(unmapped, 0, "Unmapped register should return 0");

        // Try another unmapped offset
        let unmapped2 = read_register(&mut dut, 0x10);
        assert_eq!(unmapped2, 0, "Unmapped register should return 0");
    }
}

// ============================================================
// SYSTEM CONTROLLER TESTS (in separate module)
// ============================================================

mod system_controller {
    /// System Controller Peripheral RTL Tests
    ///
    /// Tests the system_controller module which manages CPU boot, reset, and system control.
    ///
    /// Register Map:
    ///   0x00 - STATUS (RO): bit 0 = cpu_booting, bit 1 = cpu_halted
    ///   0x04 - RESET  (WO): write 1 = system reset, write 2 = CPU reset
    ///   0x08 - BOOT   (WO): write boot address to complete CPU boot
    ///   0x0C - HALT   (RW): termination code + CPU halt request pulse
    ///
    /// Control outputs are one-cycle pulses on register writes.
    use riscv_core::{create_system_controller_runtime, SystemController};

    // Register offsets
    const REG_STATUS: u32 = 0x00;
    const REG_RESET: u32 = 0x04;
    const REG_BOOT: u32 = 0x08;
    const REG_HALT: u32 = 0x0C;

    // Reset control values
    const RESET_SYSTEM: u32 = 1;
    const RESET_CPU: u32 = 2;

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

    fn reset_dut(dut: &mut SystemController) {
        dut.rst_n = 0;
        dut.clk = 0;
        dut.req = 0;
        dut.we = 0;
        dut.addr = 0;
        dut.wdata = 0;
        dut.size = 0b10; // Word
        dut.cpu_halted = 0;
        dut.cpu_booting = 0;
        dut.eval();
        clock_cycle!(dut);
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();
    }

    fn read_register(dut: &mut SystemController, offset: u32) -> u32 {
        dut.addr = offset;
        dut.we = 0;
        dut.req = 1;
        dut.size = 0b10; // Word
        dut.eval();
        let result = dut.rdata;
        clock_cycle!(dut);
        dut.req = 0;
        dut.eval();
        result
    }

    fn write_register(dut: &mut SystemController, offset: u32, value: u32) {
        dut.addr = offset;
        dut.wdata = value;
        dut.we = 1;
        dut.req = 1;
        dut.size = 0b10; // Word
        dut.eval();
        clock_cycle!(dut);
        dut.req = 0;
        dut.we = 0;
        dut.eval();
    }

    // ============================================================
    // Basic Register Tests
    // ============================================================

    #[test]
    fn test_system_controller_ready_always_asserted() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Ready should always be 1 (single-cycle peripheral)
        for _ in 0..10 {
            assert_eq!(dut.ready, 1, "Ready should always be asserted");
            clock_cycle!(dut);
        }
    }

    #[test]
    fn test_system_controller_status_register_read() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // When cpu_booting=0, cpu_halted=0, STATUS should be 0
        dut.cpu_booting = 0;
        dut.cpu_halted = 0;
        let status = read_register(&mut dut, REG_STATUS);
        assert_eq!(
            status & 0x03,
            0,
            "STATUS should be 0 when nothing is active"
        );

        // When cpu_booting=1, bit 0 should be set
        dut.cpu_booting = 1;
        dut.cpu_halted = 0;
        let status = read_register(&mut dut, REG_STATUS);
        assert_eq!(
            status & 0x01,
            1,
            "STATUS bit 0 should reflect cpu_booting=1"
        );
        assert_eq!(status & 0x02, 0, "STATUS bit 1 should reflect cpu_halted=0");

        // When cpu_halted=1, bit 1 should be set
        dut.cpu_booting = 0;
        dut.cpu_halted = 1;
        let status = read_register(&mut dut, REG_STATUS);
        assert_eq!(
            status & 0x01,
            0,
            "STATUS bit 0 should reflect cpu_booting=0"
        );
        assert_eq!(status & 0x02, 2, "STATUS bit 1 should reflect cpu_halted=1");

        // When both are set
        dut.cpu_booting = 1;
        dut.cpu_halted = 1;
        let status = read_register(&mut dut, REG_STATUS);
        assert_eq!(status & 0x03, 3, "STATUS should have both bits set");
    }

    #[test]
    fn test_system_controller_halt_register_read_write() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        assert_eq!(
            read_register(&mut dut, REG_HALT),
            0,
            "HALT register should reset to zero"
        );

        let halt_code = 0x1234_ABCD;
        write_register(&mut dut, REG_HALT, halt_code);

        assert_eq!(
            read_register(&mut dut, REG_HALT),
            halt_code,
            "HALT register should return last written value"
        );
    }

    #[test]
    fn test_system_controller_halt_write_pulses_req_cpu_halt() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        write_register(&mut dut, REG_HALT, 0xCAFE_BABE);
        assert_eq!(
            dut.req_cpu_halt, 1,
            "req_cpu_halt should pulse high for the cycle after HALT write"
        );

        clock_cycle!(dut);
        assert_eq!(
            dut.req_cpu_halt, 0,
            "req_cpu_halt should deassert after the one-cycle pulse"
        );
    }

    // ============================================================
    // FSM State Machine Tests
    // ============================================================

    #[test]
    fn test_system_controller_initial_state_after_reset() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // After reset, cpu_rst_n should be deasserted (inactive high)
        assert_eq!(
            dut.cpu_rst_n, 1,
            "cpu_rst_n should be high (inactive) after reset"
        );

        // sys_rst should be 0 (no system reset)
        assert_eq!(dut.sys_rst, 0, "sys_rst should be low after reset");
    }

    #[test]
    fn test_system_controller_boot_sequence() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Simulate CPU being in boot state
        dut.cpu_booting = 1;
        dut.eval();

        // Write boot address to BOOT register
        let boot_addr: u32 = 0x8000_0000;
        write_register(&mut dut, REG_BOOT, boot_addr);

        // After write, the system controller should have stored the boot address
        assert_eq!(
            dut.cpu_boot_addr, boot_addr,
            "cpu_boot_addr should match written boot address"
        );

        // After one more clock, should transition to S_IDLE (through S_CPU_BOOT)
        clock_cycle!(dut);

        // In S_IDLE, cpu_rst_n should be 1 (CPU released from reset)
        assert_eq!(
            dut.cpu_rst_n, 1,
            "cpu_rst_n should be high (CPU released) after boot complete"
        );
    }

    #[test]
    fn test_system_controller_boot_requires_cpu_booting() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // cpu_booting is NOT set; BOOT write should still work.
        dut.cpu_booting = 0;
        dut.eval();
        write_register(&mut dut, REG_BOOT, 0x8000_0000);
        assert_eq!(dut.cpu_boot_addr, 0x8000_0000);
        assert_eq!(dut.cpu_boot, 1, "cpu_boot should pulse on BOOT write");
    }

    #[test]
    fn test_system_controller_boot_addr_output() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        dut.cpu_booting = 1;
        dut.eval();

        // Write a specific boot address
        let test_addr: u32 = 0xDEAD_BEEF;
        write_register(&mut dut, REG_BOOT, test_addr);

        // cpu_boot_addr should reflect the written value
        assert_eq!(
            dut.cpu_boot_addr, test_addr,
            "cpu_boot_addr should output the boot address"
        );
    }

    // ============================================================
    // Reset Control Tests
    // ============================================================

    #[test]
    fn test_system_controller_system_reset() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Trigger a system reset
        write_register(&mut dut, REG_RESET, RESET_SYSTEM);
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst should remain low in write cycle and pulse next cycle"
        );
        clock_cycle!(dut);
        assert_eq!(dut.sys_rst, 1, "sys_rst should pulse one cycle after write");
        clock_cycle!(dut);
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst pulse should deassert after one cycle"
        );
    }

    #[test]
    fn test_system_controller_cpu_reset() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Trigger CPU reset
        write_register(&mut dut, REG_RESET, RESET_CPU);
        assert_eq!(
            dut.cpu_rst_n, 0,
            "cpu_rst_n should pulse low on RESET_CPU write"
        );
        clock_cycle!(dut);

        assert_eq!(
            dut.cpu_rst_n, 1,
            "cpu_rst_n should return high after one-cycle reset pulse"
        );
    }

    // ============================================================
    // LED Output Tests
    // ============================================================

    #[test]
    fn test_system_controller_led_halted() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // When cpu_halted is high, all LED bits should be 1
        // sys_led is registered so needs a clock cycle to update
        dut.cpu_halted = 1;
        dut.cpu_booting = 0;
        dut.eval();
        clock_cycle!(dut);

        assert_eq!(
            dut.sys_led, 0xFF,
            "All LEDs should be on when CPU is halted"
        );
    }

    #[test]
    fn test_system_controller_led_booting() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // When cpu_booting is high (and not halted), only first LED bit should be on
        // sys_led is registered so needs a clock cycle to update
        dut.cpu_halted = 0;
        dut.cpu_booting = 1;
        dut.eval();
        clock_cycle!(dut);

        assert_eq!(
            dut.sys_led, 0x01,
            "Only first LED should be on when CPU is booting"
        );
    }

    #[test]
    fn test_system_controller_led_normal() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // When neither halted nor booting, all LEDs should be off
        // sys_led is registered so needs a clock cycle to update
        dut.cpu_halted = 0;
        dut.cpu_booting = 0;
        dut.eval();
        clock_cycle!(dut);

        assert_eq!(
            dut.sys_led, 0x00,
            "All LEDs should be off during normal operation"
        );
    }

    #[test]
    fn test_system_controller_led_halted_takes_priority() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // When both halted and booting, halted takes priority (all LEDs on)
        // sys_led is registered so needs a clock cycle to update
        dut.cpu_halted = 1;
        dut.cpu_booting = 1;
        dut.eval();
        clock_cycle!(dut);

        assert_eq!(
            dut.sys_led, 0xFF,
            "Halted should take priority - all LEDs on even when booting"
        );
    }

    // ============================================================
    // Edge Case Tests
    // ============================================================

    #[test]
    fn test_system_controller_write_to_status_ignored() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // STATUS is read-only, writing should have no effect
        dut.cpu_booting = 1;
        dut.cpu_halted = 0;
        dut.eval();

        let status_before = read_register(&mut dut, REG_STATUS);
        write_register(&mut dut, REG_STATUS, 0xFFFFFFFF);
        let status_after = read_register(&mut dut, REG_STATUS);

        // STATUS should still reflect the actual signal state
        assert_eq!(
            status_before & 0x03,
            status_after & 0x03,
            "Writing to STATUS should have no effect"
        );
    }

    #[test]
    fn test_system_controller_reset_clears_state() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Apply external reset
        dut.rst_n = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();
        clock_cycle!(dut);

        // Outputs should return to defaults after external reset
        assert_eq!(
            dut.cpu_rst_n, 1,
            "After external reset, cpu_rst_n should be high (inactive)"
        );
        assert_eq!(dut.sys_rst, 0, "sys_rst should be low after external reset");
    }

    #[test]
    fn test_system_controller_cpu_reset_then_reboot() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // CPU reset
        write_register(&mut dut, REG_RESET, RESET_CPU);
        assert_eq!(dut.cpu_rst_n, 0, "CPU reset should pulse cpu_rst_n low");
        clock_cycle!(dut);
        assert_eq!(dut.cpu_rst_n, 1, "cpu_rst_n should deassert after pulse");

        // Second boot with different address
        write_register(&mut dut, REG_BOOT, 0xA000_0000);

        assert_eq!(
            dut.cpu_boot_addr, 0xA000_0000,
            "Boot address should be updated on reboot"
        );
        assert_eq!(dut.cpu_rst_n, 1, "CPU should be released after reboot");
    }

    #[test]
    fn test_system_controller_invalid_reset_value_ignored() {
        let runtime =
            create_system_controller_runtime().expect("Failed to create system controller runtime");
        let mut dut = runtime
            .create_model_simple::<SystemController>()
            .expect("Failed to create system controller model");

        reset_dut(&mut dut);

        // Boot the CPU
        dut.cpu_booting = 1;
        dut.eval();
        write_register(&mut dut, REG_BOOT, 0x8000_0000);
        clock_cycle!(dut);
        assert_eq!(dut.cpu_rst_n, 1, "CPU should be in S_IDLE");

        // Write invalid value to RESET register (neither 1 nor 2)
        write_register(&mut dut, REG_RESET, 0x42);
        clock_cycle!(dut);

        // Should still be in S_IDLE
        assert_eq!(
            dut.cpu_rst_n, 1,
            "CPU should still be released - invalid reset value ignored"
        );
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst should not be asserted for invalid reset value"
        );
    }
}

// Bus Arbiter Tests
// Comprehensive testing of the bus_arbiter RTL module
//
// The bus arbiter implements fixed-priority arbitration between CPU and Host masters
// Priority: Host > CPU
//
// Features tested:
// - Basic single-master access (CPU only, Host only)
// - Priority resolution (Host gets priority over CPU)
// - Grant holding until transaction completes
// - Sequential transactions
// - Simultaneous request handling

use riscv_core::{create_bus_arbiter_runtime, BusArbiter};

// Clock cycle macro
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

/// Apply reset to the module
fn reset_module(dut: &mut BusArbiter) {
    dut.rst_n = 0;
    dut.cpu_addr = 0;
    dut.cpu_wdata = 0;
    dut.cpu_we = 0;
    dut.cpu_size = 0;
    dut.cpu_req = 0;
    dut.host_addr = 0;
    dut.host_wdata = 0;
    dut.host_we = 0;
    dut.host_size = 0;
    dut.host_req = 0;
    dut.bus_rdata = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

// ============================================================
// Reset State Tests
// ============================================================

#[test]
fn test_reset_state() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Verify outputs are in expected initial state
    assert_eq!(dut.cpu_ready, 0, "cpu_ready should be LOW after reset");
    assert_eq!(dut.host_ready, 0, "host_ready should be LOW after reset");
    assert_eq!(dut.bus_req, 0, "bus_req should be LOW after reset");
}

#[test]
fn test_idle_no_requests() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Run for many cycles without any requests
    for _ in 0..100 {
        assert_eq!(dut.bus_req, 0, "bus_req should stay LOW without requests");
        assert_eq!(
            dut.cpu_ready, 0,
            "cpu_ready should stay LOW without requests"
        );
        assert_eq!(
            dut.host_ready, 0,
            "host_ready should stay LOW without requests"
        );
        clock_cycle!(dut);
    }
}

// ============================================================
// CPU-Only Access Tests
// ============================================================

#[test]
fn test_cpu_only_read() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // CPU requests a read
    dut.cpu_addr = 0x50000000;
    dut.cpu_we = 0;
    dut.cpu_size = 0b10; // Word
    dut.cpu_req = 1;
    clock_cycle!(dut);

    // Verify arbiter forwards to bus
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
    assert_eq!(dut.bus_addr, 0x50000000, "bus_addr should match cpu_addr");
    assert_eq!(dut.bus_we, 0, "bus_we should be 0 for read");
    assert_eq!(dut.bus_size, 0b10, "bus_size should match cpu_size");

    // Simulate bus response
    dut.bus_rdata = 0xDEADBEEF;
    dut.bus_ready = 1;
    clock_cycle!(dut);

    // Verify CPU receives response
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be asserted");
    assert_eq!(
        dut.cpu_rdata, 0xDEADBEEF,
        "cpu_rdata should match bus_rdata"
    );

    // Deassert request and bus_ready
    dut.cpu_req = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);

    // Verify return to idle
    assert_eq!(dut.bus_req, 0, "bus_req should return to LOW");
    assert_eq!(dut.cpu_ready, 0, "cpu_ready should return to LOW");
}

#[test]
fn test_cpu_only_write() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // CPU requests a write
    dut.cpu_addr = 0x51000000;
    dut.cpu_wdata = 0x12345678;
    dut.cpu_we = 1;
    dut.cpu_size = 0b10; // Word
    dut.cpu_req = 1;
    clock_cycle!(dut);

    // Verify arbiter forwards to bus
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
    assert_eq!(dut.bus_addr, 0x51000000, "bus_addr should match");
    assert_eq!(dut.bus_wdata, 0x12345678, "bus_wdata should match");
    assert_eq!(dut.bus_we, 1, "bus_we should be 1 for write");

    // Simulate bus response
    dut.bus_ready = 1;
    clock_cycle!(dut);

    // Verify CPU receives response
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be asserted");

    // Deassert request
    dut.cpu_req = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);

    // Verify return to idle
    assert_eq!(dut.bus_req, 0, "bus_req should return to LOW");
}

// ============================================================
// Host-Only Access Tests
// ============================================================

#[test]
fn test_host_only_read() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host requests a read
    dut.host_addr = 0x50000000;
    dut.host_we = 0;
    dut.host_size = 0b10; // Word
    dut.host_req = 1;
    clock_cycle!(dut);

    // Verify arbiter forwards to bus
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
    assert_eq!(dut.bus_addr, 0x50000000, "bus_addr should match host_addr");
    assert_eq!(dut.bus_we, 0, "bus_we should be 0 for read");

    // Simulate bus response
    dut.bus_rdata = 0xCAFEBABE;
    dut.bus_ready = 1;
    clock_cycle!(dut);

    // Verify Host receives response
    assert_eq!(dut.host_ready, 1, "host_ready should be asserted");
    assert_eq!(
        dut.host_rdata, 0xCAFEBABE,
        "host_rdata should match bus_rdata"
    );

    // Deassert request
    dut.host_req = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);

    // Verify return to idle
    assert_eq!(dut.bus_req, 0, "bus_req should return to LOW");
}

#[test]
fn test_host_only_write() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host requests a write
    dut.host_addr = 0x50000000;
    dut.host_wdata = 0xAA;
    dut.host_we = 1;
    dut.host_size = 0b00; // Byte
    dut.host_req = 1;
    clock_cycle!(dut);

    // Verify arbiter forwards to bus
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
    assert_eq!(dut.bus_addr, 0x50000000, "bus_addr should match");
    assert_eq!(dut.bus_wdata, 0xAA, "bus_wdata should match");
    assert_eq!(dut.bus_we, 1, "bus_we should be 1 for write");

    // Simulate bus response
    dut.bus_ready = 1;
    clock_cycle!(dut);

    // Verify Host receives response
    assert_eq!(dut.host_ready, 1, "host_ready should be asserted");

    // Deassert request
    dut.host_req = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);

    // Verify return to idle
    assert_eq!(dut.bus_req, 0, "bus_req should return to LOW");
}

// ============================================================
// Priority Resolution Tests
// ============================================================

#[test]
fn test_host_priority_over_cpu() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Both CPU and Host request simultaneously
    dut.cpu_addr = 0x51000000;
    dut.cpu_wdata = 0x11111111;
    dut.cpu_we = 1;
    dut.cpu_req = 1;

    dut.host_addr = 0x50000000;
    dut.host_wdata = 0x22222222;
    dut.host_we = 1;
    dut.host_req = 1;

    clock_cycle!(dut);

    // Host should get priority (arbiter is registered, state transitions on clock)
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
    assert_eq!(
        dut.bus_addr, 0x50000000,
        "bus_addr should be Host address (priority)"
    );
    assert_eq!(dut.bus_wdata, 0x22222222, "bus_wdata should be Host data");

    // Complete Host transaction
    dut.bus_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.host_ready, 1, "host_ready should be asserted");
    assert_eq!(dut.cpu_ready, 0, "cpu_ready should NOT be asserted");

    // Host releases, bus_ready still high (this triggers state transition to CPU_GRANT)
    dut.host_req = 0;
    clock_cycle!(dut);
    dut.bus_ready = 0;

    // CPU should now have the bus (state is now CPU_GRANT)
    assert_eq!(dut.bus_req, 1, "bus_req should be asserted for CPU");
    assert_eq!(dut.bus_addr, 0x51000000, "bus_addr should be CPU address");
    assert_eq!(dut.bus_wdata, 0x11111111, "bus_wdata should be CPU data");

    // Complete CPU transaction
    dut.bus_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.cpu_ready, 1, "cpu_ready should be asserted");

    // CPU releases
    dut.cpu_req = 0;
    dut.bus_ready = 0;
    clock_cycle!(dut);

    // Verify return to idle
    assert_eq!(dut.bus_req, 0, "bus_req should return to LOW");
}

#[test]
fn test_host_preempts_cpu_waiting() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // CPU requests first
    dut.cpu_addr = 0x51000000;
    dut.cpu_we = 0;
    dut.cpu_req = 1;
    clock_cycle!(dut);

    // Verify CPU has the bus
    assert_eq!(dut.bus_addr, 0x51000000, "CPU should have bus initially");

    // CPU transaction in progress (bus not ready yet)
    // Host requests while CPU is waiting
    dut.host_addr = 0x50000000;
    dut.host_we = 0;
    dut.host_req = 1;
    clock_cycle!(dut);

    // CPU should still hold the bus (grant is held during transaction)
    // The arbiter grants the bus to CPU first, holds until ready
    assert_eq!(
        dut.bus_addr, 0x51000000,
        "CPU should still have bus (in progress)"
    );

    // Complete CPU transaction
    dut.bus_rdata = 0x12345678;
    dut.bus_ready = 1;
    clock_cycle!(dut);
    // CPU receives ready signal

    // CPU releases, but host is still requesting - arbiter transitions to HOST_GRANT
    dut.cpu_req = 0;
    clock_cycle!(dut);
    dut.bus_ready = 0;

    // Now Host should have the bus (state is now HOST_GRANT)
    assert_eq!(dut.bus_addr, 0x50000000, "Host should now have bus");

    // Complete Host transaction
    dut.bus_rdata = 0xABCDEF00;
    dut.bus_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.host_ready, 1, "host_ready should be asserted");
    assert_eq!(dut.host_rdata, 0xABCDEF00, "host_rdata should match");
}

// ============================================================
// Sequential Transaction Tests
// ============================================================

#[test]
fn test_sequential_cpu_transactions() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Multiple sequential CPU transactions
    for i in 0..5 {
        let addr = 0x50000000 + (i * 4);
        let data = 0x1000 + i;

        dut.cpu_addr = addr;
        dut.cpu_wdata = data;
        dut.cpu_we = 1;
        dut.cpu_req = 1;
        clock_cycle!(dut);

        assert_eq!(dut.bus_req, 1, "bus_req should be asserted");
        assert_eq!(dut.bus_addr, addr, "bus_addr should match");

        // Complete transaction
        dut.bus_ready = 1;
        clock_cycle!(dut);
        assert_eq!(dut.cpu_ready, 1, "cpu_ready should be asserted");

        // Release
        dut.cpu_req = 0;
        dut.bus_ready = 0;
        clock_cycle!(dut);
    }
}

#[test]
fn test_alternating_cpu_host_transactions() {
    let runtime = create_bus_arbiter_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<BusArbiter>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    for i in 0..3 {
        // CPU transaction
        dut.cpu_addr = 0x51000000 + (i * 4);
        dut.cpu_we = 0;
        dut.cpu_req = 1;
        clock_cycle!(dut);

        assert_eq!(dut.bus_addr, 0x51000000 + (i * 4), "CPU addr should match");

        dut.bus_rdata = 0xC0000000 + i;
        dut.bus_ready = 1;
        clock_cycle!(dut);
        assert_eq!(dut.cpu_ready, 1);

        // CPU releases - need to clock once more to return to IDLE
        dut.cpu_req = 0;
        clock_cycle!(dut);
        dut.bus_ready = 0;
        // Now in IDLE state

        // Host transaction
        dut.host_addr = 0x50000000 + (i * 4);
        dut.host_we = 1;
        dut.host_wdata = 0xAA + i;
        dut.host_req = 1;
        clock_cycle!(dut);
        // Now in HOST_GRANT state

        assert_eq!(dut.bus_addr, 0x50000000 + (i * 4), "Host addr should match");

        dut.bus_ready = 1;
        clock_cycle!(dut);
        assert_eq!(dut.host_ready, 1);

        // Host releases - need to clock once more to return to IDLE
        dut.host_req = 0;
        clock_cycle!(dut);
        dut.bus_ready = 0;
    }
}

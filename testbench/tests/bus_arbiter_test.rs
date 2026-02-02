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

    // CPU transaction still active (Host waiting)
    assert_eq!(dut.bus_addr, 0x4000, "CPU transaction still active");

    // Complete CPU transaction (bus_ready HIGH)
    // With host_req asserted, next_state will be HOST_GRANT
    respond_to_bus(&mut dut, 0xBBBBBBBB, 1);
    clock_cycle!(dut);

    // State transitioned to HOST_GRANT on this cycle
    // CPU does NOT see ready=1 because grant switched to Host
    assert_eq!(
        dut.cpu_ready, 0,
        "cpu_ready should be LOW (grant switched to Host)"
    );

    // Host should have the bus now (preemption)
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

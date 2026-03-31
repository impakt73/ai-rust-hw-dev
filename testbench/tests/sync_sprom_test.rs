use riscv_core::{create_sync_sprom_runtime, SyncSpromTestWrapper};

use riscv_core::AsDynamicVerilatedModel;
fn clock_cycle(dut: &mut SyncSpromTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
}

fn advance_read_latency(dut: &mut SyncSpromTestWrapper) {
    clock_cycle(dut);
    clock_cycle(dut);
}

#[test]
fn test_sync_sprom_reads_initialized_contents() {
    let runtime = create_sync_sprom_runtime().expect("Failed to create sync_sprom runtime");
    let mut dut = runtime
        .create_model_simple::<SyncSpromTestWrapper>()
        .expect("Failed to create sync_sprom model");

    dut.addr = 0;
    advance_read_latency(&mut dut);
    assert_eq!(
        dut.rdata, 0x1234_5678,
        "address 0 should return initialized ROM contents"
    );

    dut.addr = 3;
    advance_read_latency(&mut dut);
    assert_eq!(
        dut.rdata, 0x00C0_FFEE,
        "address 3 should return initialized ROM contents"
    );
}

#[test]
fn test_sync_sprom_read_pipeline_latency() {
    let runtime = create_sync_sprom_runtime().expect("Failed to create sync_sprom runtime");
    let mut dut = runtime
        .create_model_simple::<SyncSpromTestWrapper>()
        .expect("Failed to create sync_sprom model");

    dut.addr = 1;
    advance_read_latency(&mut dut);
    assert_eq!(
        dut.rdata, 0xDEAD_BEEF,
        "priming read should return address 1 data"
    );

    dut.addr = 6;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.rdata, 0xDEAD_BEEF,
        "output should retain prior value one cycle after an address change"
    );

    clock_cycle(&mut dut);
    assert_eq!(
        dut.rdata, 0x1357_9BDF,
        "output should update to the new address after the internal pipeline latency"
    );
}

#[test]
fn test_sync_sprom_repeated_reads_are_stable() {
    let runtime = create_sync_sprom_runtime().expect("Failed to create sync_sprom runtime");
    let mut dut = runtime
        .create_model_simple::<SyncSpromTestWrapper>()
        .expect("Failed to create sync_sprom model");

    dut.addr = 5;
    advance_read_latency(&mut dut);
    let first_read = dut.rdata;

    advance_read_latency(&mut dut);
    assert_eq!(
        first_read, dut.rdata,
        "re-reading the same address should produce the same ROM data"
    );
    assert_eq!(dut.rdata, 0x5A5A_5A5A, "address 5 should remain stable");
}

use riscv_core::{create_sync_sprom_runtime, SyncSpromTestWrapper};
use std::fs;
use std::path::{Path, PathBuf};

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

fn ensure_init_file_visible() {
    let relative_init_path = Path::new("rtl/common/wrappers/sync_sprom_test_init.hex");
    if relative_init_path.is_file() {
        return;
    }

    let source_init_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rtl/common/wrappers/sync_sprom_test_init.hex");
    let target_init_path = std::env::current_dir()
        .expect("Failed to read current directory")
        .join(relative_init_path);
    let target_dir = target_init_path
        .parent()
        .expect("ROM init file path should have a parent directory");

    fs::create_dir_all(target_dir).expect("Failed to create ROM init file directory");
    fs::copy(&source_init_path, &target_init_path).expect("Failed to stage ROM init file");
}

#[test]
fn test_sync_sprom_reads_initialized_contents() {
    let runtime = create_sync_sprom_runtime().expect("Failed to create sync_sprom runtime");
    ensure_init_file_visible();
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
    ensure_init_file_visible();
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
    ensure_init_file_visible();
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

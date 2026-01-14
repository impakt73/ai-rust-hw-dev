use rand::Rng;
use riscv_core::{create_regfile_runtime, RegFile};

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_regfile_runtime().expect("Failed to create RegFile runtime")
}

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

#[test]
fn test_regfile_write_read() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Write value 42 to register x1
    dut.we = 1;
    dut.rd_addr = 1;
    dut.rd_data = 42;
    clock_cycle!(dut);

    // Disable write
    dut.we = 0;
    dut.eval();

    // Read from x1
    dut.rs1_addr = 1;
    dut.eval();
    assert_eq!(
        dut.rs1_data, 42,
        "Register x1 should contain 42 after write"
    );

    // Write value 100 to register x2
    dut.we = 1;
    dut.rd_addr = 2;
    dut.rd_data = 100;
    clock_cycle!(dut);

    // Read from x2
    dut.we = 0;
    dut.rs2_addr = 2;
    dut.eval();
    assert_eq!(
        dut.rs2_data, 100,
        "Register x2 should contain 100 after write"
    );

    // Verify x1 still has its value
    dut.rs1_addr = 1;
    dut.eval();
    assert_eq!(dut.rs1_data, 42, "Register x1 should still contain 42");
}

#[test]
fn test_regfile_x0_hardwired() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Attempt to write to x0
    dut.we = 1;
    dut.rd_addr = 0;
    dut.rd_data = 0xDEADBEEF;
    clock_cycle!(dut);

    // Disable write
    dut.we = 0;
    dut.eval();

    // Read from x0
    dut.rs1_addr = 0;
    dut.eval();
    assert_eq!(dut.rs1_data, 0, "Register x0 must always be 0");

    dut.rs2_addr = 0;
    dut.eval();
    assert_eq!(dut.rs2_data, 0, "Register x0 must always be 0");
}

#[test]
fn test_regfile_simultaneous_read() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Write to multiple registers
    for i in 1..10 {
        dut.we = 1;
        dut.rd_addr = i;
        dut.rd_data = (i * 10) as u32;
        clock_cycle!(dut);
    }

    // Disable write
    dut.we = 0;
    dut.eval();

    // Test simultaneous reads
    dut.rs1_addr = 3;
    dut.rs2_addr = 7;
    dut.eval();
    assert_eq!(dut.rs1_data, 30, "Register x3 should contain 30");
    assert_eq!(dut.rs2_data, 70, "Register x7 should contain 70");
}

#[test]
fn test_regfile_write_enable() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Write value to x5
    dut.we = 1;
    dut.rd_addr = 5;
    dut.rd_data = 123;
    clock_cycle!(dut);

    // Read to verify write
    dut.we = 0;
    dut.rs1_addr = 5;
    dut.eval();
    assert_eq!(dut.rs1_data, 123);

    // Attempt to write without write enable
    dut.we = 0;
    dut.rd_addr = 5;
    dut.rd_data = 456;
    clock_cycle!(dut);

    // Verify value didn't change
    dut.rs1_addr = 5;
    dut.eval();
    assert_eq!(
        dut.rs1_data, 123,
        "Register should not change when write enable is low"
    );
}

#[test]
fn test_regfile_all_registers() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();
    let mut rng = rand::thread_rng();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Write random values to all registers (except x0)
    let mut expected_values = [0u32; 32];
    #[allow(clippy::needless_range_loop)]
    for i in 1..32 {
        let value: u32 = rng.gen();
        expected_values[i] = value;

        dut.we = 1;
        dut.rd_addr = i as u8;
        dut.rd_data = value;
        clock_cycle!(dut);
    }

    // Disable write
    dut.we = 0;
    dut.eval();

    // Verify all registers
    #[allow(clippy::needless_range_loop)]
    for i in 0..32 {
        dut.rs1_addr = i as u8;
        dut.eval();
        assert_eq!(
            dut.rs1_data, expected_values[i],
            "Register x{} should contain {}",
            i, expected_values[i]
        );
    }
}

#[test]
fn test_regfile_overwrite() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<RegFile>().unwrap();

    // Initialize
    dut.clk = 0;
    dut.we = 0;
    dut.eval();

    // Write initial value to x10
    dut.we = 1;
    dut.rd_addr = 10;
    dut.rd_data = 111;
    clock_cycle!(dut);

    // Verify initial write
    dut.we = 0;
    dut.rs1_addr = 10;
    dut.eval();
    assert_eq!(dut.rs1_data, 111);

    // Overwrite x10
    dut.we = 1;
    dut.rd_addr = 10;
    dut.rd_data = 222;
    clock_cycle!(dut);

    // Verify overwrite
    dut.we = 0;
    dut.rs1_addr = 10;
    dut.eval();
    assert_eq!(dut.rs1_data, 222, "Register x10 should be overwritten");
}

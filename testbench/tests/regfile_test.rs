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

// BRAM-based register file has 1-cycle read latency.
// After setting the read address, we need to clock once to get the data.
macro_rules! read_cycle {
    ($dut:expr) => {
        clock_cycle!($dut);
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

    // Read from x1 (BRAM has 1-cycle read latency)
    dut.rs1_addr = 1;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(
        dut.rs1_data, 42,
        "Register x1 should contain 42 after write"
    );

    // Write value 100 to register x2
    dut.we = 1;
    dut.rd_addr = 2;
    dut.rd_data = 100;
    clock_cycle!(dut);

    // Read from x2 (BRAM has 1-cycle read latency)
    dut.we = 0;
    dut.rs2_addr = 2;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(
        dut.rs2_data, 100,
        "Register x2 should contain 100 after write"
    );

    // Verify x1 still has its value
    dut.rs1_addr = 1;
    dut.eval();
    read_cycle!(dut);
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

    // NOTE: x0 write gating is now handled in the CPU module, not the regfile.
    // The regfile's BRAM is initialized to 0, so x0 starts as 0.
    // In this standalone test, we verify that x0 is initially 0 (BRAM initialized).
    // The CPU will prevent writes to x0 using the reg_write_x0_gate signal.

    // Verify x0 is initially 0 (BRAM initialization)
    dut.rs1_addr = 0;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(dut.rs1_data, 0, "Register x0 must be initialized to 0");

    dut.rs2_addr = 0;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(dut.rs2_data, 0, "Register x0 must be initialized to 0");
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

    // Test simultaneous reads (BRAM has 1-cycle read latency)
    dut.rs1_addr = 3;
    dut.rs2_addr = 7;
    dut.eval();
    read_cycle!(dut);
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

    // Read to verify write (BRAM has 1-cycle read latency)
    dut.we = 0;
    dut.rs1_addr = 5;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(dut.rs1_data, 123);

    // Attempt to write without write enable
    dut.we = 0;
    dut.rd_addr = 5;
    dut.rd_data = 456;
    clock_cycle!(dut);

    // Verify value didn't change (BRAM has 1-cycle read latency)
    dut.rs1_addr = 5;
    dut.eval();
    read_cycle!(dut);
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

    // Verify all registers (BRAM has 1-cycle read latency)
    #[allow(clippy::needless_range_loop)]
    for i in 0..32 {
        dut.rs1_addr = i as u8;
        dut.eval();
        read_cycle!(dut);
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

    // Verify initial write (BRAM has 1-cycle read latency)
    dut.we = 0;
    dut.rs1_addr = 10;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(dut.rs1_data, 111);

    // Overwrite x10
    dut.we = 1;
    dut.rd_addr = 10;
    dut.rd_data = 222;
    clock_cycle!(dut);

    // Verify overwrite (BRAM has 1-cycle read latency)
    dut.we = 0;
    dut.rs1_addr = 10;
    dut.eval();
    read_cycle!(dut);
    assert_eq!(dut.rs1_data, 222, "Register x10 should be overwritten");
}

// ============================================================
// FP_REGFILE TESTS (in separate module)
// ============================================================

mod fp_regfile {
    use rand::Rng;
    use riscv_core::{create_fp_regfile_runtime, FpRegFile};

    fn create_runtime() -> riscv_core::VerilatorRuntime {
        create_fp_regfile_runtime().expect("Failed to create FpRegFile runtime")
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
    fn test_fp_regfile_write_read() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset the FP register file
        dut.clk = 0;
        dut.rst_n = 0; // Assert reset
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1; // Deassert reset
        dut.eval();

        // Write IEEE 754 value 1.0 (0x3F800000) to register f1
        dut.we = 1;
        dut.rd_addr = 1;
        dut.rd_data = 0x3F800000; // 1.0 in IEEE 754 single precision
        clock_cycle!(dut);

        // Disable write
        dut.we = 0;
        dut.eval();

        // Read from f1
        dut.rs1_addr = 1;
        dut.eval();
        assert_eq!(
            dut.rs1_data, 0x3F800000,
            "Register f1 should contain 1.0 (0x3F800000) after write"
        );

        // Write IEEE 754 value 2.0 (0x40000000) to register f2
        dut.we = 1;
        dut.rd_addr = 2;
        dut.rd_data = 0x40000000; // 2.0 in IEEE 754 single precision
        clock_cycle!(dut);

        // Read from f2
        dut.we = 0;
        dut.rs2_addr = 2;
        dut.eval();
        assert_eq!(
            dut.rs2_data, 0x40000000,
            "Register f2 should contain 2.0 (0x40000000) after write"
        );

        // Verify f1 still has its value
        dut.rs1_addr = 1;
        dut.eval();
        assert_eq!(
            dut.rs1_data, 0x3F800000,
            "Register f1 should still contain 1.0"
        );
    }

    #[test]
    fn test_fp_regfile_f0_writable() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Unlike integer x0, FP f0 is writable
        dut.we = 1;
        dut.rd_addr = 0;
        dut.rd_data = 0x40490FDB; // Pi (approximately 3.14159)
        clock_cycle!(dut);

        // Disable write
        dut.we = 0;
        dut.eval();

        // Read from f0 - should contain written value
        dut.rs1_addr = 0;
        dut.eval();
        assert_eq!(
            dut.rs1_data, 0x40490FDB,
            "Register f0 should be writable (not hardwired to 0 like x0)"
        );
    }

    #[test]
    fn test_fp_regfile_three_port_read() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Write to three different registers
        let test_values = [
            (3, 0x3F800000), // f3 = 1.0
            (5, 0x40000000), // f5 = 2.0
            (7, 0x40400000), // f7 = 3.0
        ];

        for (addr, value) in &test_values {
            dut.we = 1;
            dut.rd_addr = *addr;
            dut.rd_data = *value;
            clock_cycle!(dut);
        }

        // Disable write
        dut.we = 0;
        dut.eval();

        // Test simultaneous 3-port reads (for FMADD instructions)
        dut.rs1_addr = 3;
        dut.rs2_addr = 5;
        dut.rs3_addr = 7;
        dut.eval();

        assert_eq!(dut.rs1_data, 0x3F800000, "Register f3 should contain 1.0");
        assert_eq!(dut.rs2_data, 0x40000000, "Register f5 should contain 2.0");
        assert_eq!(dut.rs3_data, 0x40400000, "Register f7 should contain 3.0");
    }

    #[test]
    fn test_fp_regfile_write_enable() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Write value to f10
        dut.we = 1;
        dut.rd_addr = 10;
        dut.rd_data = 0x41200000; // 10.0
        clock_cycle!(dut);

        // Read to verify write
        dut.we = 0;
        dut.rs1_addr = 10;
        dut.eval();
        assert_eq!(dut.rs1_data, 0x41200000);

        // Attempt to write without write enable
        dut.we = 0;
        dut.rd_addr = 10;
        dut.rd_data = 0x41A00000; // 20.0
        clock_cycle!(dut);

        // Verify value didn't change
        dut.rs1_addr = 10;
        dut.eval();
        assert_eq!(
            dut.rs1_data, 0x41200000,
            "Register should not change when write enable is low"
        );
    }

    #[test]
    fn test_fp_regfile_all_registers() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();
        let mut rng = rand::thread_rng();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Write random values to all 32 FP registers (including f0)
        let mut expected_values = [0u32; 32];
        #[allow(clippy::needless_range_loop)]
        for i in 0..32 {
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
                "Register f{} should contain 0x{:08X}",
                i, expected_values[i]
            );
        }
    }

    #[test]
    fn test_fp_regfile_reset() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Write non-zero values to several registers
        for i in 1..10 {
            dut.we = 1;
            dut.rd_addr = i;
            dut.rd_data = 0x40000000 + ((i as u32) * 0x100000); // Different values
            clock_cycle!(dut);
        }

        // Assert reset again
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Verify all registers are reset to +0.0 (0x00000000)
        #[allow(clippy::needless_range_loop)]
        for i in 0..32 {
            dut.rs1_addr = i as u8;
            dut.eval();
            assert_eq!(
                dut.rs1_data, 0x00000000,
                "Register f{} should be reset to +0.0 (0x00000000)",
                i
            );
        }
    }

    #[test]
    fn test_fp_regfile_overwrite() {
        let runtime = create_runtime();
        let mut dut = runtime.create_model_simple::<FpRegFile>().unwrap();

        // Reset
        dut.clk = 0;
        dut.rst_n = 0;
        dut.we = 0;
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        // Write initial value to f15
        dut.we = 1;
        dut.rd_addr = 15;
        dut.rd_data = 0x3F800000; // 1.0
        clock_cycle!(dut);

        // Verify initial write
        dut.we = 0;
        dut.rs1_addr = 15;
        dut.eval();
        assert_eq!(dut.rs1_data, 0x3F800000);

        // Overwrite f15
        dut.we = 1;
        dut.rd_addr = 15;
        dut.rd_data = 0x7F800000; // +infinity
        clock_cycle!(dut);

        // Verify overwrite
        dut.we = 0;
        dut.rs1_addr = 15;
        dut.eval();
        assert_eq!(
            dut.rs1_data, 0x7F800000,
            "Register f15 should be overwritten to +infinity"
        );
    }
}

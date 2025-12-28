use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
use marlin::verilog::prelude::*;
use rand::Rng;

// ALU Operation Encodings (must match the RTL)
const ALU_ADD: u32 = 0b0000;
const ALU_SUB: u32 = 0b0001;
const ALU_AND: u32 = 0b0010;
const ALU_OR: u32 = 0b0011;
const ALU_XOR: u32 = 0b0100;
const ALU_SLL: u32 = 0b0101;
const ALU_SRL: u32 = 0b0110;
const ALU_SRA: u32 = 0b0111;
const ALU_SLT: u32 = 0b1000;
const ALU_SLTU: u32 = 0b1001;

#[verilog(src = "../rtl/alu.sv", name = "alu")]
pub struct Alu;

fn calculate_expected(a: u32, b: u32, alu_op: u32) -> u32 {
    match alu_op {
        ALU_ADD => a.wrapping_add(b),
        ALU_SUB => a.wrapping_sub(b),
        ALU_AND => a & b,
        ALU_OR => a | b,
        ALU_XOR => a ^ b,
        ALU_SLL => a << (b & 0x1F),
        ALU_SRL => a >> (b & 0x1F),
        ALU_SRA => ((a as i32) >> (b & 0x1F)) as u32,
        ALU_SLT => {
            if (a as i32) < (b as i32) {
                1
            } else {
                0
            }
        }
        ALU_SLTU => {
            if a < b {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

#[test]
fn test_alu_add() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    for _ in 0..100 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen();
        let expected = a.wrapping_add(b);

        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_ADD as u8;
        dut.eval();

        assert_eq!(
            dut.result, expected,
            "ADD failed: {} + {} = {} (expected {})",
            a, b, dut.result, expected
        );

        let expected_zero = if expected == 0 { 1 } else { 0 };
        assert_eq!(dut.zero, expected_zero);
    }
}

#[test]
fn test_alu_sub() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    for _ in 0..100 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen();
        let expected = a.wrapping_sub(b);

        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_SUB as u8;
        dut.eval();

        assert_eq!(
            dut.result, expected,
            "SUB failed: {} - {} = {} (expected {})",
            a, b, dut.result, expected
        );
    }
}

#[test]
fn test_alu_logic_ops() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    for _ in 0..50 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen();

        // Test AND
        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_AND as u8;
        dut.eval();
        assert_eq!(dut.result, a & b, "AND failed");

        // Test OR
        dut.alu_op = ALU_OR as u8;
        dut.eval();
        assert_eq!(dut.result, a | b, "OR failed");

        // Test XOR
        dut.alu_op = ALU_XOR as u8;
        dut.eval();
        assert_eq!(dut.result, a ^ b, "XOR failed");
    }
}

#[test]
fn test_alu_shift_ops() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    for _ in 0..50 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen_range(0..32);

        // Test SLL (Shift Left Logical)
        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_SLL as u8;
        dut.eval();
        assert_eq!(dut.result, a << b, "SLL failed");

        // Test SRL (Shift Right Logical)
        dut.alu_op = ALU_SRL as u8;
        dut.eval();
        assert_eq!(dut.result, a >> b, "SRL failed");

        // Test SRA (Shift Right Arithmetic)
        dut.alu_op = ALU_SRA as u8;
        dut.eval();
        let expected_sra = ((a as i32) >> b) as u32;
        assert_eq!(dut.result, expected_sra, "SRA failed");
    }
}

#[test]
fn test_alu_compare_ops() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test SLT (Set Less Than - signed)
    let test_cases_slt = vec![
        (10i32, 20i32, 1u32),
        (20i32, 10i32, 0u32),
        (-10i32, 10i32, 1u32),
        (10i32, -10i32, 0u32),
        (-20i32, -10i32, 1u32),
    ];

    for (a, b, expected) in test_cases_slt {
        dut.a = a as u32;
        dut.b = b as u32;
        dut.alu_op = ALU_SLT as u8;
        dut.eval();
        assert_eq!(
            dut.result, expected,
            "SLT failed: {} < {} should be {}",
            a, b, expected
        );
    }

    // Test SLTU (Set Less Than Unsigned)
    let test_cases_sltu = vec![
        (10u32, 20u32, 1u32),
        (20u32, 10u32, 0u32),
        (0xFFFFFFFFu32, 1u32, 0u32), // max unsigned > 1
        (1u32, 0xFFFFFFFFu32, 1u32), // 1 < max unsigned
    ];

    for (a, b, expected) in test_cases_sltu {
        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_SLTU as u8;
        dut.eval();
        assert_eq!(
            dut.result, expected,
            "SLTU failed: {} < {} should be {}",
            a, b, expected
        );
    }
}

#[test]
fn test_alu_zero_flag() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test zero flag with ADD resulting in 0
    dut.a = 0;
    dut.b = 0;
    dut.alu_op = ALU_ADD as u8;
    dut.eval();
    assert_eq!(dut.result, 0);
    assert_eq!(dut.zero, 1, "Zero flag should be set");

    // Test zero flag with non-zero result
    dut.a = 5;
    dut.b = 3;
    dut.alu_op = ALU_ADD as u8;
    dut.eval();
    assert_eq!(dut.result, 8);
    assert_eq!(dut.zero, 0, "Zero flag should not be set");

    // Test zero flag with SUB resulting in 0
    dut.a = 100;
    dut.b = 100;
    dut.alu_op = ALU_SUB as u8;
    dut.eval();
    assert_eq!(dut.result, 0);
    assert_eq!(dut.zero, 1, "Zero flag should be set");
}

#[test]
fn test_alu_all_operations() {
    let runtime = VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap();

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    for _ in 0..100 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen();
        let alu_op = rng.gen_range(0..=9); // 0-9 for all valid operations

        let expected = calculate_expected(a, b, alu_op);

        dut.a = a;
        dut.b = b;
        dut.alu_op = alu_op as u8;
        dut.eval();

        assert_eq!(
            dut.result, expected,
            "Operation {} failed: a={}, b={}, result={}, expected={}",
            alu_op, a, b, dut.result, expected
        );
    }
}

use rand::Rng;
use riscv_core::{create_alu_runtime, Alu};

// ALU Operation Encodings (must match the RTL)
const ALU_ADD: u32 = 0b00000;
const ALU_SUB: u32 = 0b00001;
const ALU_AND: u32 = 0b00010;
const ALU_OR: u32 = 0b00011;
const ALU_XOR: u32 = 0b00100;
const ALU_SLL: u32 = 0b00101;
const ALU_SRL: u32 = 0b00110;
const ALU_SRA: u32 = 0b00111;
const ALU_SLT: u32 = 0b01000;
const ALU_SLTU: u32 = 0b01001;

// M Extension Operation Encodings
const ALU_MUL: u32 = 0b01010;
const ALU_MULH: u32 = 0b01011;
const ALU_MULHSU: u32 = 0b01100;
const ALU_MULHU: u32 = 0b01101;
const ALU_DIV: u32 = 0b01110;
const ALU_DIVU: u32 = 0b01111;
const ALU_REM: u32 = 0b10000;
const ALU_REMU: u32 = 0b10001;

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

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

// ============================================================================
// M Extension Tests - Multiplication
// ============================================================================

#[test]
fn test_alu_mul() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();
    let mut rng = rand::thread_rng();

    // Test basic multiplication
    for _ in 0..50 {
        let a: u32 = rng.gen();
        let b: u32 = rng.gen();
        let expected = a.wrapping_mul(b);

        dut.a = a;
        dut.b = b;
        dut.alu_op = ALU_MUL as u8;
        dut.eval();

        assert_eq!(
            dut.result, expected,
            "MUL failed: {} × {} = {} (expected {})",
            a, b, dut.result, expected
        );
    }

    // Test edge cases
    dut.a = 0;
    dut.b = 0xFFFFFFFF;
    dut.alu_op = ALU_MUL as u8;
    dut.eval();
    assert_eq!(dut.result, 0, "0 × anything = 0");

    dut.a = 1;
    dut.b = 0xFFFFFFFF;
    dut.alu_op = ALU_MUL as u8;
    dut.eval();
    assert_eq!(dut.result, 0xFFFFFFFF, "1 × x = x");
}

#[test]
fn test_alu_mulh() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test signed × signed, upper 32 bits
    // Positive × Positive
    dut.a = 0x00010000; // 65536
    dut.b = 0x00010000; // 65536
    dut.alu_op = ALU_MULH as u8;
    dut.eval();
    // 65536 × 65536 = 4294967296 = 0x0000000100000000
    assert_eq!(dut.result, 0x00000001, "MULH: 65536 × 65536 upper = 1");

    // Test with larger values
    dut.a = 0x7FFFFFFF; // max positive i32
    dut.b = 2;
    dut.alu_op = ALU_MULH as u8;
    dut.eval();
    // 2147483647 × 2 = 4294967294 (as i64), upper 32 = 0
    assert_eq!(dut.result, 0, "MULH: max_positive × 2 upper = 0");

    // Negative × Negative = Positive
    dut.a = 0xFFFFFFFF; // -1
    dut.b = 0xFFFFFFFF; // -1
    dut.alu_op = ALU_MULH as u8;
    dut.eval();
    // -1 × -1 = 1, upper 32 bits = 0
    assert_eq!(dut.result, 0, "MULH: -1 × -1 upper = 0");

    // Positive × Negative
    dut.a = 0x7FFFFFFF; // max positive
    dut.b = 0xFFFFFFFF; // -1
    dut.alu_op = ALU_MULH as u8;
    dut.eval();
    // 2147483647 × -1 = -2147483647, as i64 = 0xFFFFFFFF80000001, upper = 0xFFFFFFFF
    assert_eq!(dut.result, 0xFFFFFFFF, "MULH: positive × negative");
}

#[test]
fn test_alu_mulhsu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test signed × unsigned, upper 32 bits
    // Negative signed × positive unsigned
    dut.a = 0xFFFFFFFF; // -1 (signed)
    dut.b = 0x00000002; // 2 (unsigned)
    dut.alu_op = ALU_MULHSU as u8;
    dut.eval();
    // -1 (sign-extended to 64-bit: 0xFFFFFFFFFFFFFFFF) × 2 (zero-extended: 0x0000000000000002)
    // = 0xFFFFFFFFFFFFFFFE (which is -2 in signed 64-bit)
    // Upper 32 bits: 0xFFFFFFFF
    assert_eq!(dut.result, 0xFFFFFFFF, "MULHSU: -1 × 2 upper");

    // Positive signed × large unsigned
    dut.a = 0x00000002; // 2 (signed)
    dut.b = 0xFFFFFFFF; // large unsigned
    dut.alu_op = ALU_MULHSU as u8;
    dut.eval();
    // 2 × 4294967295 = 8589934590, upper = 1
    assert_eq!(dut.result, 1, "MULHSU: 2 × max_unsigned upper");
}

#[test]
fn test_alu_mulhu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test unsigned × unsigned, upper 32 bits
    dut.a = 0xFFFFFFFF;
    dut.b = 0xFFFFFFFF;
    dut.alu_op = ALU_MULHU as u8;
    dut.eval();
    // 4294967295 × 4294967295 = 18446744065119617025
    // = 0xFFFFFFFE00000001, upper = 0xFFFFFFFE
    assert_eq!(dut.result, 0xFFFFFFFE, "MULHU: max × max upper");

    dut.a = 0x00010000;
    dut.b = 0x00010000;
    dut.alu_op = ALU_MULHU as u8;
    dut.eval();
    // 65536 × 65536 = 4294967296 = 0x100000000, upper = 1
    assert_eq!(dut.result, 1, "MULHU: 65536 × 65536 upper = 1");

    dut.a = 0x80000000;
    dut.b = 2;
    dut.alu_op = ALU_MULHU as u8;
    dut.eval();
    // 2147483648 × 2 = 4294967296, upper = 1
    assert_eq!(dut.result, 1, "MULHU: 2^31 × 2 upper");
}

// ============================================================================
// M Extension Tests - Division
// ============================================================================

#[test]
fn test_alu_div() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal signed division
    dut.a = 20;
    dut.b = 3;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, 6, "DIV: 20 ÷ 3 = 6");

    // Negative dividend
    dut.a = (-20i32) as u32;
    dut.b = 3;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, (-6i32) as u32, "DIV: -20 ÷ 3 = -6");

    // Negative divisor
    dut.a = 20;
    dut.b = (-3i32) as u32;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, (-6i32) as u32, "DIV: 20 ÷ -3 = -6");

    // Both negative
    dut.a = (-20i32) as u32;
    dut.b = (-3i32) as u32;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, 6, "DIV: -20 ÷ -3 = 6");

    // Division by zero - should return all 1's
    dut.a = 100;
    dut.b = 0;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, 0xFFFFFFFF, "DIV: division by zero = 0xFFFFFFFF");

    // Overflow case: -2^31 ÷ -1 = -2^31
    dut.a = 0x80000000;
    dut.b = 0xFFFFFFFF;
    dut.alu_op = ALU_DIV as u8;
    dut.eval();
    assert_eq!(dut.result, 0x80000000, "DIV: overflow case -2^31 ÷ -1");
}

#[test]
fn test_alu_divu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned division
    dut.a = 20;
    dut.b = 3;
    dut.alu_op = ALU_DIVU as u8;
    dut.eval();
    assert_eq!(dut.result, 6, "DIVU: 20 ÷ 3 = 6");

    // Large unsigned values
    dut.a = 0xFFFFFFFF;
    dut.b = 2;
    dut.alu_op = ALU_DIVU as u8;
    dut.eval();
    assert_eq!(dut.result, 0x7FFFFFFF, "DIVU: max_u32 ÷ 2");

    // Division by zero
    dut.a = 100;
    dut.b = 0;
    dut.alu_op = ALU_DIVU as u8;
    dut.eval();
    assert_eq!(
        dut.result, 0xFFFFFFFF,
        "DIVU: division by zero = 0xFFFFFFFF"
    );
}

// ============================================================================
// M Extension Tests - Remainder
// ============================================================================

#[test]
fn test_alu_rem() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal signed remainder
    dut.a = 20;
    dut.b = 3;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, 2, "REM: 20 % 3 = 2");

    // Negative dividend
    dut.a = (-20i32) as u32;
    dut.b = 3;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, (-2i32) as u32, "REM: -20 % 3 = -2");

    // Negative divisor
    dut.a = 20;
    dut.b = (-3i32) as u32;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, 2, "REM: 20 % -3 = 2");

    // Both negative
    dut.a = (-20i32) as u32;
    dut.b = (-3i32) as u32;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, (-2i32) as u32, "REM: -20 % -3 = -2");

    // Modulo by zero - should return dividend
    dut.a = 100;
    dut.b = 0;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, 100, "REM: modulo by zero = dividend");

    // Overflow case: -2^31 % -1 = 0
    dut.a = 0x80000000;
    dut.b = 0xFFFFFFFF;
    dut.alu_op = ALU_REM as u8;
    dut.eval();
    assert_eq!(dut.result, 0, "REM: overflow case -2^31 % -1 = 0");
}

#[test]
fn test_alu_remu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned remainder
    dut.a = 20;
    dut.b = 3;
    dut.alu_op = ALU_REMU as u8;
    dut.eval();
    assert_eq!(dut.result, 2, "REMU: 20 % 3 = 2");

    // Large unsigned values
    dut.a = 0xFFFFFFFF;
    dut.b = 10;
    dut.alu_op = ALU_REMU as u8;
    dut.eval();
    assert_eq!(dut.result, 5, "REMU: max_u32 % 10 = 5");

    // Modulo by zero - should return dividend
    dut.a = 100;
    dut.b = 0;
    dut.alu_op = ALU_REMU as u8;
    dut.eval();
    assert_eq!(dut.result, 100, "REMU: modulo by zero = dividend");
}

// ============================================================================
// M Extension Edge Cases
// ============================================================================

#[test]
fn test_alu_m_extension_edge_cases() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test all M operations with zero
    let m_ops = [
        ALU_MUL, ALU_MULH, ALU_MULHSU, ALU_MULHU, ALU_DIV, ALU_DIVU, ALU_REM, ALU_REMU,
    ];

    for &op in &m_ops {
        dut.a = 0;
        dut.b = 12345;
        dut.alu_op = op as u8;
        dut.eval();

        match op {
            ALU_MUL | ALU_MULH | ALU_MULHSU | ALU_MULHU => {
                assert_eq!(
                    dut.result, 0,
                    "M-ext op {} with zero operand should be 0",
                    op
                );
            }
            ALU_DIV | ALU_DIVU => {
                assert_eq!(
                    dut.result, 0,
                    "M-ext div op {} with zero dividend should be 0",
                    op
                );
            }
            ALU_REM | ALU_REMU => {
                assert_eq!(
                    dut.result, 0,
                    "M-ext rem op {} with zero dividend should be 0",
                    op
                );
            }
            _ => {}
        }
    }

    // Test all M operations with one
    for &op in &m_ops {
        dut.a = 0x12345678;
        dut.b = 1;
        dut.alu_op = op as u8;
        dut.eval();

        match op {
            ALU_MUL => assert_eq!(dut.result, 0x12345678, "x × 1 = x"),
            ALU_DIV | ALU_DIVU => assert_eq!(dut.result, 0x12345678, "x ÷ 1 = x"),
            ALU_REM | ALU_REMU => assert_eq!(dut.result, 0, "x % 1 = 0"),
            _ => {}
        }
    }
}

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

// A Extension Operation Encodings
const ALU_MIN: u32 = 0b10010;
const ALU_MAX: u32 = 0b10011;
const ALU_MINU: u32 = 0b10100;
const ALU_MAXU: u32 = 0b10101;

// Clock cycle macro for ALU tests
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

// Helper function for multi-cycle ALU operations (division)
// Sets up inputs, pulses alu_start, and waits for alu_ready
fn execute_alu_operation(dut: &mut Alu, a: u32, b: u32, alu_op: u8) {
    // Set inputs
    dut.a = a;
    dut.b = b;
    dut.alu_op = alu_op;

    // Reset state
    dut.rst_n = 0;
    dut.alu_start = 0;
    clock_cycle!(dut);

    // Release reset
    dut.rst_n = 1;
    clock_cycle!(dut);

    // Pulse alu_start for one cycle
    dut.alu_start = 1;
    clock_cycle!(dut);
    dut.alu_start = 0;

    // Wait for alu_ready (max 100 cycles for safety)
    for _ in 0..100 {
        dut.eval();
        if dut.alu_ready == 1 {
            break;
        }
        clock_cycle!(dut);
    }

    // Final eval to get result
    dut.eval();
}

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

        execute_alu_operation(&mut dut, a, b, ALU_ADD as u8);

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

        execute_alu_operation(&mut dut, a, b, ALU_SUB as u8);

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
        execute_alu_operation(&mut dut, a, b, ALU_AND as u8);
        assert_eq!(dut.result, a & b, "AND failed");

        // Test OR
        execute_alu_operation(&mut dut, a, b, ALU_OR as u8);
        assert_eq!(dut.result, a | b, "OR failed");

        // Test XOR
        execute_alu_operation(&mut dut, a, b, ALU_XOR as u8);
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
        execute_alu_operation(&mut dut, a, b, ALU_SLL as u8);
        assert_eq!(dut.result, a << b, "SLL failed");

        // Test SRL (Shift Right Logical)
        execute_alu_operation(&mut dut, a, b, ALU_SRL as u8);
        assert_eq!(dut.result, a >> b, "SRL failed");

        // Test SRA (Shift Right Arithmetic)
        execute_alu_operation(&mut dut, a, b, ALU_SRA as u8);
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
        execute_alu_operation(&mut dut, a as u32, b as u32, ALU_SLT as u8);
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
        execute_alu_operation(&mut dut, a, b, ALU_SLTU as u8);
        assert_eq!(
            dut.result, expected,
            "SLTU failed: {} < {} should be {}",
            a, b, expected
        );
    }
}

#[test]
fn test_alu_minmax_ops() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    let signed_cases = vec![
        (0xFFFF_FFF6u32, 10u32),          // -10, 10
        (10u32, 0xFFFF_FFF6u32),          // 10, -10
        (0xFFFF_FFECu32, 0xFFFF_FFF6u32), // -20, -10
        (25u32, 25u32),
    ];

    for (a, b) in signed_cases {
        execute_alu_operation(&mut dut, a, b, ALU_MIN as u8);
        assert_eq!(
            dut.result,
            std::cmp::min(a as i32, b as i32) as u32,
            "MIN failed"
        );

        execute_alu_operation(&mut dut, a, b, ALU_MAX as u8);
        assert_eq!(
            dut.result,
            std::cmp::max(a as i32, b as i32) as u32,
            "MAX failed"
        );
    }

    let unsigned_cases = vec![
        (0u32, u32::MAX),
        (u32::MAX, 1u32),
        (123u32, 456u32),
        (999u32, 999u32),
    ];

    for (a, b) in unsigned_cases {
        execute_alu_operation(&mut dut, a, b, ALU_MINU as u8);
        assert_eq!(dut.result, std::cmp::min(a, b), "MINU failed");

        execute_alu_operation(&mut dut, a, b, ALU_MAXU as u8);
        assert_eq!(dut.result, std::cmp::max(a, b), "MAXU failed");
    }
}

#[test]
fn test_alu_zero_flag() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test zero flag with ADD resulting in 0
    execute_alu_operation(&mut dut, 0, 0, ALU_ADD as u8);
    assert_eq!(dut.result, 0);
    assert_eq!(dut.zero, 1, "Zero flag should be set");

    // Test zero flag with non-zero result
    execute_alu_operation(&mut dut, 5, 3, ALU_ADD as u8);
    assert_eq!(dut.result, 8);
    assert_eq!(dut.zero, 0, "Zero flag should not be set");

    // Test zero flag with SUB resulting in 0
    execute_alu_operation(&mut dut, 100, 100, ALU_SUB as u8);
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
        let alu_op = rng.gen_range(0..=9); // 0-9 for RV32I base operations (M extension tested separately)

        let expected = calculate_expected(a, b, alu_op);

        execute_alu_operation(&mut dut, a, b, alu_op as u8);

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

        execute_alu_operation(&mut dut, a, b, ALU_MUL as u8);

        assert_eq!(
            dut.result, expected,
            "MUL failed: {} × {} = {} (expected {})",
            a, b, dut.result, expected
        );
    }

    // Test edge cases
    execute_alu_operation(&mut dut, 0, 0xFFFFFFFF, ALU_MUL as u8);
    assert_eq!(dut.result, 0, "0 × anything = 0");

    execute_alu_operation(&mut dut, 1, 0xFFFFFFFF, ALU_MUL as u8);
    assert_eq!(dut.result, 0xFFFFFFFF, "1 × x = x");
}

#[test]
fn test_alu_mulh() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test signed × signed, upper 32 bits
    // Positive × Positive
    execute_alu_operation(&mut dut, 0x00010000, 0x00010000, ALU_MULH as u8); // 65536 × 65536
                                                                             // 65536 × 65536 = 4294967296 = 0x0000000100000000
    assert_eq!(dut.result, 0x00000001, "MULH: 65536 × 65536 upper = 1");

    // Test with larger values
    execute_alu_operation(&mut dut, 0x7FFFFFFF, 2, ALU_MULH as u8); // max positive i32 × 2
                                                                    // 2147483647 × 2 = 4294967294 (as i64), upper 32 = 0
    assert_eq!(dut.result, 0, "MULH: max_positive × 2 upper = 0");

    // Negative × Negative = Positive
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 0xFFFFFFFF, ALU_MULH as u8); // -1 × -1
                                                                             // -1 × -1 = 1, upper 32 bits = 0
    assert_eq!(dut.result, 0, "MULH: -1 × -1 upper = 0");

    // Positive × Negative
    execute_alu_operation(&mut dut, 0x7FFFFFFF, 0xFFFFFFFF, ALU_MULH as u8); // max positive × -1
                                                                             // 2147483647 × -1 = -2147483647, as i64 = 0xFFFFFFFF80000001, upper = 0xFFFFFFFF
    assert_eq!(dut.result, 0xFFFFFFFF, "MULH: positive × negative");
}

#[test]
fn test_alu_mulhsu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test signed × unsigned, upper 32 bits
    // Negative signed × positive unsigned
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 0x00000002, ALU_MULHSU as u8); // -1 × 2
                                                                               // -1 (sign-extended to 64-bit: 0xFFFFFFFFFFFFFFFF) × 2 (zero-extended: 0x0000000000000002)
                                                                               // = 0xFFFFFFFFFFFFFFFE (which is -2 in signed 64-bit)
                                                                               // Upper 32 bits: 0xFFFFFFFF
    assert_eq!(dut.result, 0xFFFFFFFF, "MULHSU: -1 × 2 upper");

    // Positive signed × large unsigned
    execute_alu_operation(&mut dut, 0x00000002, 0xFFFFFFFF, ALU_MULHSU as u8); // 2 × max_unsigned
                                                                               // 2 × 4294967295 = 8589934590, upper = 1
    assert_eq!(dut.result, 1, "MULHSU: 2 × max_unsigned upper");
}

#[test]
fn test_alu_mulhu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test unsigned × unsigned, upper 32 bits
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 0xFFFFFFFF, ALU_MULHU as u8);
    // 4294967295 × 4294967295 = 18446744065119617025
    // = 0xFFFFFFFE00000001, upper = 0xFFFFFFFE
    assert_eq!(dut.result, 0xFFFFFFFE, "MULHU: max × max upper");

    execute_alu_operation(&mut dut, 0x00010000, 0x00010000, ALU_MULHU as u8);
    // 65536 × 65536 = 4294967296 = 0x100000000, upper = 1
    assert_eq!(dut.result, 1, "MULHU: 65536 × 65536 upper = 1");

    execute_alu_operation(&mut dut, 0x80000000, 2, ALU_MULHU as u8);
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
    execute_alu_operation(&mut dut, 20, 3, ALU_DIV as u8);
    assert_eq!(dut.result, 6, "DIV: 20 ÷ 3 = 6");

    // Negative dividend
    execute_alu_operation(&mut dut, (-20i32) as u32, 3, ALU_DIV as u8);
    assert_eq!(dut.result, (-6i32) as u32, "DIV: -20 ÷ 3 = -6");

    // Negative divisor
    execute_alu_operation(&mut dut, 20, (-3i32) as u32, ALU_DIV as u8);
    assert_eq!(dut.result, (-6i32) as u32, "DIV: 20 ÷ -3 = -6");

    // Both negative
    execute_alu_operation(&mut dut, (-20i32) as u32, (-3i32) as u32, ALU_DIV as u8);
    assert_eq!(dut.result, 6, "DIV: -20 ÷ -3 = 6");

    // Division by zero - should return all 1's
    execute_alu_operation(&mut dut, 100, 0, ALU_DIV as u8);
    assert_eq!(dut.result, 0xFFFFFFFF, "DIV: division by zero = 0xFFFFFFFF");

    // Overflow case: -2^31 ÷ -1 = -2^31
    execute_alu_operation(&mut dut, 0x80000000, 0xFFFFFFFF, ALU_DIV as u8);
    assert_eq!(dut.result, 0x80000000, "DIV: overflow case -2^31 ÷ -1");
}

#[test]
fn test_alu_divu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned division
    execute_alu_operation(&mut dut, 20, 3, ALU_DIVU as u8);
    assert_eq!(dut.result, 6, "DIVU: 20 ÷ 3 = 6");

    // Large unsigned values
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 2, ALU_DIVU as u8);
    assert_eq!(dut.result, 0x7FFFFFFF, "DIVU: max_u32 ÷ 2");

    // Division by zero
    execute_alu_operation(&mut dut, 100, 0, ALU_DIVU as u8);
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
    execute_alu_operation(&mut dut, 20, 3, ALU_REM as u8);
    assert_eq!(dut.result, 2, "REM: 20 % 3 = 2");

    // Negative dividend
    execute_alu_operation(&mut dut, (-20i32) as u32, 3, ALU_REM as u8);
    assert_eq!(dut.result, (-2i32) as u32, "REM: -20 % 3 = -2");

    // Negative divisor
    execute_alu_operation(&mut dut, 20, (-3i32) as u32, ALU_REM as u8);
    assert_eq!(dut.result, 2, "REM: 20 % -3 = 2");

    // Both negative
    execute_alu_operation(&mut dut, (-20i32) as u32, (-3i32) as u32, ALU_REM as u8);
    assert_eq!(dut.result, (-2i32) as u32, "REM: -20 % -3 = -2");

    // Modulo by zero - should return dividend
    execute_alu_operation(&mut dut, 100, 0, ALU_REM as u8);
    assert_eq!(dut.result, 100, "REM: modulo by zero = dividend");

    // Overflow case: -2^31 % -1 = 0
    execute_alu_operation(&mut dut, 0x80000000, 0xFFFFFFFF, ALU_REM as u8);
    assert_eq!(dut.result, 0, "REM: overflow case -2^31 % -1 = 0");
}

#[test]
fn test_alu_remu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned remainder
    execute_alu_operation(&mut dut, 20, 3, ALU_REMU as u8);
    assert_eq!(dut.result, 2, "REMU: 20 % 3 = 2");

    // Large unsigned values
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 10, ALU_REMU as u8);
    assert_eq!(dut.result, 5, "REMU: max_u32 % 10 = 5");

    // Modulo by zero - should return dividend
    execute_alu_operation(&mut dut, 100, 0, ALU_REMU as u8);
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
        match op {
            ALU_MUL | ALU_MULH | ALU_MULHSU | ALU_MULHU => {
                // Multiplication is now multi-cycle, use helper
                execute_alu_operation(&mut dut, 0, 12345, op as u8);
                assert_eq!(
                    dut.result, 0,
                    "M-ext op {} with zero operand should be 0",
                    op
                );
            }
            ALU_DIV | ALU_DIVU | ALU_REM | ALU_REMU => {
                // Division/Remainder: multi-cycle, use helper
                execute_alu_operation(&mut dut, 0, 12345, op as u8);
                assert_eq!(
                    dut.result, 0,
                    "M-ext div/rem op {} with zero dividend should be 0",
                    op
                );
            }
            _ => {}
        }
    }

    // Test all M operations with one
    for &op in &m_ops {
        match op {
            ALU_MUL => {
                // Multiplication is now multi-cycle, use helper
                execute_alu_operation(&mut dut, 0x12345678, 1, op as u8);
                assert_eq!(dut.result, 0x12345678, "x × 1 = x");
            }
            ALU_DIV | ALU_DIVU => {
                execute_alu_operation(&mut dut, 0x12345678, 1, op as u8);
                assert_eq!(dut.result, 0x12345678, "x ÷ 1 = x");
            }
            ALU_REM | ALU_REMU => {
                execute_alu_operation(&mut dut, 0x12345678, 1, op as u8);
                assert_eq!(dut.result, 0, "x % 1 = 0");
            }
            _ => {}
        }
    }
}

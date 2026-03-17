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

// A-Extension Operation Encodings
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

// Helper function for ALU operations.
// Sets up inputs, pulses in_valid, and waits for out_valid.
fn execute_alu_operation(dut: &mut Alu, a: u32, b: u32, alu_op: u8) {
    // Set inputs
    dut.a = a;
    dut.b = b;
    dut.alu_op = alu_op;

    // Reset state
    dut.rst = 1;
    dut.in_valid = 0;
    clock_cycle!(dut);

    // Release reset
    dut.rst = 0;
    clock_cycle!(dut);

    assert_eq!(
        dut.in_ready, 1,
        "ALU should accept a new request after reset"
    );

    // Pulse in_valid for one cycle
    dut.in_valid = 1;
    clock_cycle!(dut);
    dut.in_valid = 0;

    // Wait for out_valid (max 100 cycles for safety)
    let mut saw_out_valid = false;
    for _ in 0..100 {
        dut.eval();
        if dut.out_valid == 1 {
            saw_out_valid = true;
            break;
        }
        clock_cycle!(dut);
    }

    assert!(
        saw_out_valid,
        "ALU operation timed out waiting for out_valid (op={alu_op:#04x}, a={a:#010x}, b={b:#010x})"
    );

    // Final eval to get out_data.
    dut.eval();
}

fn reset_alu(dut: &mut Alu) {
    dut.rst = 1;
    dut.in_valid = 0;
    clock_cycle!(dut);
    dut.rst = 0;
    clock_cycle!(dut);
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
            dut.out_data, expected,
            "ADD failed: {} + {} = {} (expected {})",
            a, b, dut.out_data, expected
        );
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
            dut.out_data, expected,
            "SUB failed: {} - {} = {} (expected {})",
            a, b, dut.out_data, expected
        );
    }
}

#[test]
fn test_alu_operations_latch_inputs_after_handshake() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    for (a, b, alu_op, expected) in [
        (5_u32, 3_u32, ALU_ADD as u8, 8_u32),
        (0xFFFF_FFFBu32, 3_u32, ALU_MIN as u8, 0xFFFF_FFFBu32),
        (7_u32, 6_u32, ALU_MUL as u8, 42_u32),
        (100_u32, 9_u32, ALU_DIVU as u8, 11_u32),
    ] {
        reset_alu(&mut dut);
        assert_eq!(dut.in_ready, 1, "ALU should accept a request after reset");

        dut.a = a;
        dut.b = b;
        dut.alu_op = alu_op;
        dut.in_valid = 1;
        clock_cycle!(dut);
        dut.in_valid = 0;

        // Change inputs immediately after the request handshake; the result should still
        // reflect the accepted request, not these later values.
        dut.a = 3;
        dut.b = 2;
        dut.alu_op = ALU_ADD as u8;

        let mut saw_out_valid = false;
        for _ in 0..100 {
            dut.eval();
            if dut.out_valid == 1 {
                saw_out_valid = true;
                break;
            }
            clock_cycle!(dut);
        }

        assert!(saw_out_valid, "Timed out waiting for latched ALU result");
        assert_eq!(
            dut.out_data, expected,
            "ALU result should use latched inputs"
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
        assert_eq!(dut.out_data, a & b, "AND failed");

        // Test OR
        execute_alu_operation(&mut dut, a, b, ALU_OR as u8);
        assert_eq!(dut.out_data, a | b, "OR failed");

        // Test XOR
        execute_alu_operation(&mut dut, a, b, ALU_XOR as u8);
        assert_eq!(dut.out_data, a ^ b, "XOR failed");
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
        assert_eq!(dut.out_data, a << b, "SLL failed");

        // Test SRL (Shift Right Logical)
        execute_alu_operation(&mut dut, a, b, ALU_SRL as u8);
        assert_eq!(dut.out_data, a >> b, "SRL failed");

        // Test SRA (Shift Right Arithmetic)
        execute_alu_operation(&mut dut, a, b, ALU_SRA as u8);
        let expected_sra = ((a as i32) >> b) as u32;
        assert_eq!(dut.out_data, expected_sra, "SRA failed");
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
            dut.out_data, expected,
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
            dut.out_data, expected,
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
        (0x8000_0000u32, 0u32),           // i32::MIN, 0
        (0xFFFF_FFF6u32, 10u32),          // -10, 10
        (10u32, 0xFFFF_FFF6u32),          // 10, -10
        (0xFFFF_FFECu32, 0xFFFF_FFF6u32), // -20, -10
        (25u32, 25u32),
    ];

    for (a, b) in signed_cases {
        execute_alu_operation(&mut dut, a, b, ALU_MIN as u8);
        assert_eq!(
            dut.out_data,
            std::cmp::min(a as i32, b as i32) as u32,
            "MIN failed"
        );

        execute_alu_operation(&mut dut, a, b, ALU_MAX as u8);
        assert_eq!(
            dut.out_data,
            std::cmp::max(a as i32, b as i32) as u32,
            "MAX failed"
        );
    }

    let unsigned_cases = vec![
        (0u32, u32::MAX),
        (u32::MAX, 1u32),
        (u32::MAX, u32::MAX),
        (123u32, 456u32),
        (999u32, 999u32),
    ];

    for (a, b) in unsigned_cases {
        execute_alu_operation(&mut dut, a, b, ALU_MINU as u8);
        assert_eq!(dut.out_data, std::cmp::min(a, b), "MINU failed");

        execute_alu_operation(&mut dut, a, b, ALU_MAXU as u8);
        assert_eq!(dut.out_data, std::cmp::max(a, b), "MAXU failed");
    }
}

#[test]
fn test_alu_minmax_result_is_registered() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    dut.rst = 1;
    dut.in_valid = 0;
    clock_cycle!(dut);

    dut.rst = 0;
    dut.a = 0xFFFF_FFFBu32; // -5
    dut.b = 3u32;
    dut.alu_op = ALU_MIN as u8;

    dut.in_valid = 1;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MIN should spend the first cycle comparing"
    );

    clock_cycle!(dut);

    dut.in_valid = 0;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MIN should spend the second cycle registering the compare result"
    );
    assert_eq!(
        dut.in_ready, 0,
        "ALU should hold off new requests while MIN is pending"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MIN should spend the third cycle selecting the comparison mode"
    );
    assert_eq!(
        dut.in_ready, 0,
        "ALU should remain busy until the MIN comparison selection stage completes"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MIN result should still be pending while the selected operands are staged"
    );
    assert_eq!(
        dut.in_ready, 0,
        "ALU should remain busy until the MIN operand result stage completes"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MIN result should still be pending while the min/max candidates are registered"
    );
    assert_eq!(
        dut.in_ready, 0,
        "ALU should remain busy until the MIN output selection stage completes"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 1,
        "MIN should be valid after all four registered min/max stages complete"
    );
    assert_eq!(
        dut.out_data, 0xFFFF_FFFBu32,
        "MIN should select the smaller operand"
    );
    assert_eq!(
        dut.in_ready, 1,
        "ALU should be ready for another request after registering the MIN result"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 1,
        "MIN result should remain valid until upstream logic consumes it"
    );
    assert_eq!(dut.out_data, 0xFFFF_FFFBu32);

    dut.a = 10u32;
    dut.b = 3u32;
    dut.alu_op = ALU_MAXU as u8;
    dut.in_valid = 1;
    dut.eval();
    assert_eq!(
        dut.in_ready, 1,
        "ALU should be ready to accept the next request"
    );

    clock_cycle!(dut);

    dut.in_valid = 0;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MAXU should spend the second cycle registering the compare result"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MAXU should spend the third cycle selecting the comparison mode"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MAXU result should still be pending while the selected operands are staged"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "MAXU result should still be pending while the min/max candidates are registered"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 1,
        "MAXU should be valid after all four registered min/max stages complete"
    );
    assert_eq!(dut.out_data, 10u32, "MAXU should select the larger operand");
}

#[test]
fn test_alu_single_cycle_result_is_registered() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");

    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    dut.rst = 1;
    dut.in_valid = 0;
    clock_cycle!(dut);

    dut.rst = 0;
    dut.a = 5;
    dut.b = 3;
    dut.alu_op = ALU_ADD as u8;
    dut.eval();

    assert_eq!(dut.in_ready, 1, "ADD should be accepted immediately");
    assert_eq!(
        dut.out_valid, 0,
        "Output must not be valid before the request"
    );

    dut.in_valid = 1;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "Output must stay registered during the request cycle"
    );

    clock_cycle!(dut);

    dut.in_valid = 0;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "ADD result should still be pending on the first cycle after the request edge"
    );
    assert_eq!(
        dut.in_ready, 0,
        "ALU should not accept a new request while ADD is pending"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.out_valid, 1,
        "ADD result should become valid once the pending request reaches the response stage"
    );
    assert_eq!(
        dut.out_data, 8,
        "ADD result should be registered internally"
    );
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
            dut.out_data, expected,
            "Operation {} failed: a={}, b={}, out_data={}, expected={}",
            alu_op, a, b, dut.out_data, expected
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
            dut.out_data, expected,
            "MUL failed: {} × {} = {} (expected {})",
            a, b, dut.out_data, expected
        );
    }

    // Test edge cases
    execute_alu_operation(&mut dut, 0, 0xFFFFFFFF, ALU_MUL as u8);
    assert_eq!(dut.out_data, 0, "0 × anything = 0");

    execute_alu_operation(&mut dut, 1, 0xFFFFFFFF, ALU_MUL as u8);
    assert_eq!(dut.out_data, 0xFFFFFFFF, "1 × x = x");
}

#[test]
fn test_alu_mulh() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test signed × signed, upper 32 bits
    // Positive × Positive
    execute_alu_operation(&mut dut, 0x00010000, 0x00010000, ALU_MULH as u8); // 65536 × 65536
                                                                             // 65536 × 65536 = 4294967296 = 0x0000000100000000
    assert_eq!(dut.out_data, 0x00000001, "MULH: 65536 × 65536 upper = 1");

    // Test with larger values
    execute_alu_operation(&mut dut, 0x7FFFFFFF, 2, ALU_MULH as u8); // max positive i32 × 2
                                                                    // 2147483647 × 2 = 4294967294 (as i64), upper 32 = 0
    assert_eq!(dut.out_data, 0, "MULH: max_positive × 2 upper = 0");

    // Negative × Negative = Positive
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 0xFFFFFFFF, ALU_MULH as u8); // -1 × -1
                                                                             // -1 × -1 = 1, upper 32 bits = 0
    assert_eq!(dut.out_data, 0, "MULH: -1 × -1 upper = 0");

    // Positive × Negative
    execute_alu_operation(&mut dut, 0x7FFFFFFF, 0xFFFFFFFF, ALU_MULH as u8); // max positive × -1
                                                                             // 2147483647 × -1 = -2147483647, as i64 = 0xFFFFFFFF80000001, upper = 0xFFFFFFFF
    assert_eq!(dut.out_data, 0xFFFFFFFF, "MULH: positive × negative");
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
    assert_eq!(dut.out_data, 0xFFFFFFFF, "MULHSU: -1 × 2 upper");

    // Positive signed × large unsigned
    execute_alu_operation(&mut dut, 0x00000002, 0xFFFFFFFF, ALU_MULHSU as u8); // 2 × max_unsigned
                                                                               // 2 × 4294967295 = 8589934590, upper = 1
    assert_eq!(dut.out_data, 1, "MULHSU: 2 × max_unsigned upper");
}

#[test]
fn test_alu_mulhu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Test unsigned × unsigned, upper 32 bits
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 0xFFFFFFFF, ALU_MULHU as u8);
    // 4294967295 × 4294967295 = 18446744065119617025
    // = 0xFFFFFFFE00000001, upper = 0xFFFFFFFE
    assert_eq!(dut.out_data, 0xFFFFFFFE, "MULHU: max × max upper");

    execute_alu_operation(&mut dut, 0x00010000, 0x00010000, ALU_MULHU as u8);
    // 65536 × 65536 = 4294967296 = 0x100000000, upper = 1
    assert_eq!(dut.out_data, 1, "MULHU: 65536 × 65536 upper = 1");

    execute_alu_operation(&mut dut, 0x80000000, 2, ALU_MULHU as u8);
    // 2147483648 × 2 = 4294967296, upper = 1
    assert_eq!(dut.out_data, 1, "MULHU: 2^31 × 2 upper");
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
    assert_eq!(dut.out_data, 6, "DIV: 20 ÷ 3 = 6");

    // Negative dividend
    execute_alu_operation(&mut dut, (-20i32) as u32, 3, ALU_DIV as u8);
    assert_eq!(dut.out_data, (-6i32) as u32, "DIV: -20 ÷ 3 = -6");

    // Negative divisor
    execute_alu_operation(&mut dut, 20, (-3i32) as u32, ALU_DIV as u8);
    assert_eq!(dut.out_data, (-6i32) as u32, "DIV: 20 ÷ -3 = -6");

    // Both negative
    execute_alu_operation(&mut dut, (-20i32) as u32, (-3i32) as u32, ALU_DIV as u8);
    assert_eq!(dut.out_data, 6, "DIV: -20 ÷ -3 = 6");

    // Division by zero - should return all 1's
    execute_alu_operation(&mut dut, 100, 0, ALU_DIV as u8);
    assert_eq!(
        dut.out_data, 0xFFFFFFFF,
        "DIV: division by zero = 0xFFFFFFFF"
    );

    // Overflow case: -2^31 ÷ -1 = -2^31
    execute_alu_operation(&mut dut, 0x80000000, 0xFFFFFFFF, ALU_DIV as u8);
    assert_eq!(dut.out_data, 0x80000000, "DIV: overflow case -2^31 ÷ -1");
}

#[test]
fn test_alu_divu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned division
    execute_alu_operation(&mut dut, 20, 3, ALU_DIVU as u8);
    assert_eq!(dut.out_data, 6, "DIVU: 20 ÷ 3 = 6");

    // Large unsigned values
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 2, ALU_DIVU as u8);
    assert_eq!(dut.out_data, 0x7FFFFFFF, "DIVU: max_u32 ÷ 2");

    // Division by zero
    execute_alu_operation(&mut dut, 100, 0, ALU_DIVU as u8);
    assert_eq!(
        dut.out_data, 0xFFFFFFFF,
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
    assert_eq!(dut.out_data, 2, "REM: 20 % 3 = 2");

    // Negative dividend
    execute_alu_operation(&mut dut, (-20i32) as u32, 3, ALU_REM as u8);
    assert_eq!(dut.out_data, (-2i32) as u32, "REM: -20 % 3 = -2");

    // Negative divisor
    execute_alu_operation(&mut dut, 20, (-3i32) as u32, ALU_REM as u8);
    assert_eq!(dut.out_data, 2, "REM: 20 % -3 = 2");

    // Both negative
    execute_alu_operation(&mut dut, (-20i32) as u32, (-3i32) as u32, ALU_REM as u8);
    assert_eq!(dut.out_data, (-2i32) as u32, "REM: -20 % -3 = -2");

    // Modulo by zero - should return dividend
    execute_alu_operation(&mut dut, 100, 0, ALU_REM as u8);
    assert_eq!(dut.out_data, 100, "REM: modulo by zero = dividend");

    // Overflow case: -2^31 % -1 = 0
    execute_alu_operation(&mut dut, 0x80000000, 0xFFFFFFFF, ALU_REM as u8);
    assert_eq!(dut.out_data, 0, "REM: overflow case -2^31 % -1 = 0");
}

#[test]
fn test_alu_remu() {
    let runtime = create_alu_runtime().expect("Failed to create ALU runtime");
    let mut dut = runtime.create_model_simple::<Alu>().unwrap();

    // Normal unsigned remainder
    execute_alu_operation(&mut dut, 20, 3, ALU_REMU as u8);
    assert_eq!(dut.out_data, 2, "REMU: 20 % 3 = 2");

    // Large unsigned values
    execute_alu_operation(&mut dut, 0xFFFFFFFF, 10, ALU_REMU as u8);
    assert_eq!(dut.out_data, 5, "REMU: max_u32 % 10 = 5");

    // Modulo by zero - should return dividend
    execute_alu_operation(&mut dut, 100, 0, ALU_REMU as u8);
    assert_eq!(dut.out_data, 100, "REMU: modulo by zero = dividend");
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
                    dut.out_data, 0,
                    "M-ext op {} with zero operand should be 0",
                    op
                );
            }
            ALU_DIV | ALU_DIVU | ALU_REM | ALU_REMU => {
                // Division/Remainder: multi-cycle, use helper
                execute_alu_operation(&mut dut, 0, 12345, op as u8);
                assert_eq!(
                    dut.out_data, 0,
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
                assert_eq!(dut.out_data, 0x12345678, "x × 1 = x");
            }
            ALU_DIV | ALU_DIVU => {
                execute_alu_operation(&mut dut, 0x12345678, 1, op as u8);
                assert_eq!(dut.out_data, 0x12345678, "x ÷ 1 = x");
            }
            ALU_REM | ALU_REMU => {
                execute_alu_operation(&mut dut, 0x12345678, 1, op as u8);
                assert_eq!(dut.out_data, 0, "x % 1 = 0");
            }
            _ => {}
        }
    }
}

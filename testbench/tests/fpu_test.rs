use riscv_core::AsDynamicVerilatedModel;
use riscv_core::Fpu;
use testbench::with_fpu_model;
// FPU Operation Encodings (must match rtl/fpu.sv)
const FPU_ADD: u8 = 0b00000;
const FPU_SUB: u8 = 0b00001;
const FPU_MUL: u8 = 0b00010;
const FPU_DIV: u8 = 0b00011;
const FPU_SQRT: u8 = 0b00100;
const FPU_MIN: u8 = 0b00101;
const FPU_MAX: u8 = 0b00110;
const FPU_MADD: u8 = 0b00111;
const FPU_MSUB: u8 = 0b01000;
const FPU_NMSUB: u8 = 0b01001;
const FPU_NMADD: u8 = 0b01010;
const FPU_SGNJ: u8 = 0b01011;
const FPU_SGNJN: u8 = 0b01100;
const FPU_SGNJX: u8 = 0b01101;
const FPU_CVTWS: u8 = 0b01110;
const FPU_CVTWUS: u8 = 0b01111;
const FPU_CVTSW: u8 = 0b10000;
const FPU_CVTSWU: u8 = 0b10001;
const FPU_FEQ: u8 = 0b10010;
const FPU_FLT: u8 = 0b10011;
const FPU_FLE: u8 = 0b10100;
const FPU_FCLASS: u8 = 0b10101;
const FPU_MVXW: u8 = 0b10110;
const FPU_MVWX: u8 = 0b10111;

// IEEE 754 test constants
const POS_ZERO: u32 = 0x00000000;
const NEG_ZERO: u32 = 0x80000000;
const ONE: u32 = 0x3F800000; // 1.0
const TWO: u32 = 0x40000000; // 2.0
const THREE: u32 = 0x40400000; // 3.0
const FOUR: u32 = 0x40800000; // 4.0
const NEG_ONE: u32 = 0xBF800000; // -1.0
const POS_INF: u32 = 0x7F800000;
const NEG_INF: u32 = 0xFF800000;
const QNAN: u32 = 0x7FC00000;

// Clock cycle macro for FPU tests
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

// Helper function for multi-cycle FPU operations
// Sets up inputs, pulses fpu_start, and waits for fpu_ready
// All parameters are passed to the hardware; unused parameters should be set to 0
fn execute_fpu_operation(
    dut: &mut Fpu,
    fs1: u32,
    fs2: u32,
    fs3: u32,
    int_src: u32,
    fpu_op: u8,
    rm: u8,
) {
    // Set inputs
    dut.fs1 = fs1;
    dut.fs2 = fs2;
    dut.fs3 = fs3;
    dut.int_src = int_src;
    dut.fpu_op = fpu_op;
    dut.rm = rm;

    // Reset state
    dut.rst = 1;
    dut.fpu_start = 0;
    clock_cycle!(dut);

    // Release reset
    dut.rst = 0;
    clock_cycle!(dut);

    // Pulse fpu_start for one cycle
    dut.fpu_start = 1;
    clock_cycle!(dut);
    dut.fpu_start = 0;

    // Wait for fpu_ready (max 60 cycles - 48-bit div needs ~50 cycles)
    for _ in 0..60 {
        dut.eval();
        if dut.fpu_ready == 1 {
            break;
        }
        clock_cycle!(dut);
    }

    // Final eval to get result
    dut.eval();
}

// ========== Arithmetic Tests ==========

#[test]
fn test_fpu_add_basic() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 + 2.0 = 3.0
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_ADD, 0);
        assert_eq!(dut.fp_result, THREE, "1.0 + 2.0 should equal 3.0");
    });
}

#[test]
fn test_fpu_add_negative() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 + (-1.0) = 0.0
        execute_fpu_operation(&mut dut, ONE, NEG_ONE, 0, 0, FPU_ADD, 0);
        assert_eq!(dut.fp_result, POS_ZERO, "1.0 + (-1.0) should equal 0.0");
    });
}

#[test]
fn test_fpu_sub_basic() {
    with_fpu_model(|mut dut| {
        // Test: 3.0 - 1.0 = 2.0
        execute_fpu_operation(&mut dut, THREE, ONE, 0, 0, FPU_SUB, 0);
        assert_eq!(dut.fp_result, TWO, "3.0 - 1.0 should equal 2.0");
    });
}

#[test]
fn test_fpu_mul_basic() {
    with_fpu_model(|mut dut| {
        // Test: 2.0 * 2.0 = 4.0
        execute_fpu_operation(&mut dut, TWO, TWO, 0, 0, FPU_MUL, 0);
        assert_eq!(dut.fp_result, FOUR, "2.0 * 2.0 should equal 4.0");
    });
}

// ========== Sign Injection Tests ==========

#[test]
fn test_fpu_sign_injection() {
    with_fpu_model(|mut dut| {
        // FSGNJ: copy sign of fs2 to fs1 -> -1.0
        execute_fpu_operation(&mut dut, ONE, NEG_ONE, 0, 0, FPU_SGNJ, 0);
        assert_eq!(dut.fp_result, NEG_ONE, "fsgnj(1.0, -1.0) should equal -1.0");

        // FSGNJN: copy inverted sign of fs2 to fs1 -> 1.0
        execute_fpu_operation(&mut dut, ONE, NEG_ONE, 0, 0, FPU_SGNJN, 0);
        assert_eq!(dut.fp_result, ONE, "fsgnjn(1.0, -1.0) should equal 1.0");

        // FSGNJX: XOR signs -> -1.0
        execute_fpu_operation(&mut dut, ONE, NEG_ONE, 0, 0, FPU_SGNJX, 0);
        assert_eq!(
            dut.fp_result, NEG_ONE,
            "fsgnjx(1.0, -1.0) should equal -1.0"
        );
    });
}

// ========== Comparison Tests ==========

#[test]
fn test_fpu_feq() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 == 1.0 -> true (1)
        execute_fpu_operation(&mut dut, ONE, ONE, 0, 0, FPU_FEQ, 0);
        assert_eq!(dut.int_result, 1, "1.0 == 1.0 should be true");

        // Test: 1.0 == 2.0 -> false (0)
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_FEQ, 0);
        assert_eq!(dut.int_result, 0, "1.0 == 2.0 should be false");
    });
}

#[test]
fn test_fpu_flt() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 < 2.0 -> true (1)
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_FLT, 0);
        assert_eq!(dut.int_result, 1, "1.0 < 2.0 should be true");

        // Test: 2.0 < 1.0 -> false (0)
        execute_fpu_operation(&mut dut, TWO, ONE, 0, 0, FPU_FLT, 0);
        assert_eq!(dut.int_result, 0, "2.0 < 1.0 should be false");
    });
}

#[test]
fn test_fpu_fle() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 <= 2.0 -> true (1)
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_FLE, 0);
        assert_eq!(dut.int_result, 1, "1.0 <= 2.0 should be true");

        // Test: 1.0 <= 1.0 -> true (1)
        execute_fpu_operation(&mut dut, ONE, ONE, 0, 0, FPU_FLE, 0);
        assert_eq!(dut.int_result, 1, "1.0 <= 1.0 should be true");

        // Test: 2.0 <= 1.0 -> false (0)
        execute_fpu_operation(&mut dut, TWO, ONE, 0, 0, FPU_FLE, 0);
        assert_eq!(dut.int_result, 0, "2.0 <= 1.0 should be false");
    });
}

// ========== MIN/MAX Tests ==========

#[test]
fn test_fpu_min_max() {
    with_fpu_model(|mut dut| {
        // MIN: min(1.0, 2.0) = 1.0
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_MIN, 0);
        assert_eq!(dut.fp_result, ONE, "min(1.0, 2.0) should equal 1.0");

        // MAX: max(1.0, 2.0) = 2.0
        execute_fpu_operation(&mut dut, ONE, TWO, 0, 0, FPU_MAX, 0);
        assert_eq!(dut.fp_result, TWO, "max(1.0, 2.0) should equal 2.0");
    });
}

#[test]
fn test_fpu_min_max_signed_zero() {
    with_fpu_model(|mut dut| {
        // MIN: min(+0.0, -0.0) = -0.0
        execute_fpu_operation(&mut dut, POS_ZERO, NEG_ZERO, 0, 0, FPU_MIN, 0);
        assert_eq!(dut.fp_result, NEG_ZERO, "min(+0.0, -0.0) should equal -0.0");

        // MAX: max(+0.0, -0.0) = +0.0
        execute_fpu_operation(&mut dut, POS_ZERO, NEG_ZERO, 0, 0, FPU_MAX, 0);
        assert_eq!(dut.fp_result, POS_ZERO, "max(+0.0, -0.0) should equal +0.0");
    });
}

// ========== Conversion Tests ==========

#[test]
fn test_fpu_fcvt_w_s() {
    with_fpu_model(|mut dut| {
        // Test: float 3.0 -> int 3
        execute_fpu_operation(&mut dut, THREE, 0, 0, 0, FPU_CVTWS, 0);
        assert_eq!(dut.int_result, 3, "fcvt.w.s(3.0) should equal 3");

        // Test: float -1.0 -> int -1
        execute_fpu_operation(&mut dut, NEG_ONE, 0, 0, 0, FPU_CVTWS, 0);
        assert_eq!(dut.int_result as i32, -1, "fcvt.w.s(-1.0) should equal -1");
    });
}

#[test]
fn test_fpu_fcvt_wu_s() {
    with_fpu_model(|mut dut| {
        // Test: float 3.0 -> unsigned int 3
        execute_fpu_operation(&mut dut, THREE, 0, 0, 0, FPU_CVTWUS, 0);
        assert_eq!(dut.int_result, 3, "fcvt.wu.s(3.0) should equal 3");

        // Test: float -1.0 -> unsigned int 0 (saturates with NV flag)
        execute_fpu_operation(&mut dut, NEG_ONE, 0, 0, 0, FPU_CVTWUS, 0);
        assert_eq!(dut.int_result, 0, "fcvt.wu.s(-1.0) should saturate to 0");
        assert_eq!(dut.fflags & 0b10000, 0b10000, "NV flag should be set");
    });
}

#[test]
fn test_fpu_fcvt_s_w() {
    with_fpu_model(|mut dut| {
        // Test: int 3 -> float 3.0
        execute_fpu_operation(&mut dut, 0, 0, 0, 3, FPU_CVTSW, 0);
        assert_eq!(dut.fp_result, THREE, "fcvt.s.w(3) should equal 3.0");

        // Test: int -1 -> float -1.0
        execute_fpu_operation(&mut dut, 0, 0, 0, (-1i32) as u32, FPU_CVTSW, 0);
        assert_eq!(dut.fp_result, NEG_ONE, "fcvt.s.w(-1) should equal -1.0");
    });
}

#[test]
fn test_fpu_fcvt_s_wu() {
    with_fpu_model(|mut dut| {
        // Test: unsigned int 3 -> float 3.0
        execute_fpu_operation(&mut dut, 0, 0, 0, 3, FPU_CVTSWU, 0);
        assert_eq!(dut.fp_result, THREE, "fcvt.s.wu(3) should equal 3.0");

        // Test: unsigned int 100 -> float 100.0
        execute_fpu_operation(&mut dut, 0, 0, 0, 100, FPU_CVTSWU, 0);
        let hundred: u32 = 0x42C80000; // 100.0
        assert_eq!(dut.fp_result, hundred, "fcvt.s.wu(100) should equal 100.0");
    });
}

// ========== Move and Classification Tests ==========

#[test]
fn test_fpu_fmv_x_w() {
    with_fpu_model(|mut dut| {
        execute_fpu_operation(&mut dut, ONE, 0, 0, 0, FPU_MVXW, 0);
        assert_eq!(dut.int_result, ONE, "fmv.x.w should copy bits unchanged");
    });
}

#[test]
fn test_fpu_fmv_w_x() {
    with_fpu_model(|mut dut| {
        execute_fpu_operation(&mut dut, 0, 0, 0, ONE, FPU_MVWX, 0);
        assert_eq!(dut.fp_result, ONE, "fmv.w.x should copy bits unchanged");
    });
}

#[test]
fn test_fpu_fclass() {
    with_fpu_model(|mut dut| {
        // Test: fclass(-inf) = bit 0
        execute_fpu_operation(&mut dut, NEG_INF, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000001, "fclass(-inf) should be 0x001");

        // Test: fclass(-1.0) = bit 1 (negative normal)
        execute_fpu_operation(&mut dut, NEG_ONE, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000002, "fclass(-1.0) should be 0x002");

        // Test: fclass(-0.0) = bit 3
        execute_fpu_operation(&mut dut, NEG_ZERO, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000008, "fclass(-0.0) should be 0x008");

        // Test: fclass(+0.0) = bit 4
        execute_fpu_operation(&mut dut, POS_ZERO, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000010, "fclass(+0.0) should be 0x010");

        // Test: fclass(1.0) = bit 6 (positive normal)
        execute_fpu_operation(&mut dut, ONE, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000040, "fclass(1.0) should be 0x040");

        // Test: fclass(+inf) = bit 7
        execute_fpu_operation(&mut dut, POS_INF, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000080, "fclass(+inf) should be 0x080");

        // Test: fclass(QNaN) = bit 9
        execute_fpu_operation(&mut dut, QNAN, 0, 0, 0, FPU_FCLASS, 0);
        assert_eq!(dut.int_result, 0x00000200, "fclass(QNaN) should be 0x200");
    });
}

// ========== Division Tests ==========

// FP32 division test - using multi-cycle hardware divider
#[test]
fn test_fpu_div_basic() {
    with_fpu_model(|mut dut| {
        // Test: 4.0 / 2.0 = 2.0
        execute_fpu_operation(&mut dut, FOUR, TWO, 0, 0, FPU_DIV, 0);
        assert_eq!(dut.fp_result, TWO, "4.0 / 2.0 should equal 2.0");
    });
}

#[test]
fn test_fpu_div_by_zero() {
    with_fpu_model(|mut dut| {
        // Test: 1.0 / 0.0 = +inf with DZ flag
        execute_fpu_operation(&mut dut, ONE, POS_ZERO, 0, 0, FPU_DIV, 0);
        assert_eq!(dut.fp_result, POS_INF, "1.0 / 0.0 should equal +inf");
        assert_eq!(dut.fflags & 0b01000, 0b01000, "DZ flag should be set");
    });
}

// ========== Square Root Tests ==========

#[test]
fn test_fpu_sqrt_basic() {
    with_fpu_model(|mut dut| {
        // Test: sqrt(4.0) = 2.0
        execute_fpu_operation(&mut dut, FOUR, 0, 0, 0, FPU_SQRT, 0);
        assert_eq!(dut.fp_result, TWO, "sqrt(4.0) should equal 2.0");
    });
}

#[test]
fn test_fpu_sqrt_negative() {
    with_fpu_model(|mut dut| {
        // Test: sqrt(-1.0) = NaN with NV flag
        execute_fpu_operation(&mut dut, NEG_ONE, 0, 0, 0, FPU_SQRT, 0);
        assert_eq!(dut.fp_result, QNAN, "sqrt(-1.0) should equal NaN");
        assert_eq!(dut.fflags & 0b10000, 0b10000, "NV flag should be set");
    });
}

// ========== Fused Multiply-Add Tests ==========

#[test]
fn test_fpu_fmadd() {
    with_fpu_model(|mut dut| {
        // Test: FMADD 2.0 * 3.0 + 1.0 = 7.0
        let seven: u32 = 0x40E00000; // 7.0
        execute_fpu_operation(&mut dut, TWO, THREE, ONE, 0, FPU_MADD, 0);
        assert_eq!(dut.fp_result, seven, "2.0 * 3.0 + 1.0 should equal 7.0");
    });
}

#[test]
fn test_fpu_fmsub() {
    with_fpu_model(|mut dut| {
        // Test: FMSUB 2.0 * 3.0 - 1.0 = 5.0
        let five: u32 = 0x40A00000; // 5.0
        execute_fpu_operation(&mut dut, TWO, THREE, ONE, 0, FPU_MSUB, 0);
        assert_eq!(dut.fp_result, five, "2.0 * 3.0 - 1.0 should equal 5.0");
    });
}

#[test]
fn test_fpu_fnmsub() {
    with_fpu_model(|mut dut| {
        // Test: FNMSUB -(2.0 * 3.0) + 1.0 = -5.0
        let neg_five: u32 = 0xC0A00000; // -5.0
        execute_fpu_operation(&mut dut, TWO, THREE, ONE, 0, FPU_NMSUB, 0);
        assert_eq!(
            dut.fp_result, neg_five,
            "-(2.0 * 3.0) + 1.0 should equal -5.0"
        );
    });
}

#[test]
fn test_fpu_fnmadd() {
    with_fpu_model(|mut dut| {
        // Test: FNMADD -(2.0 * 3.0) - 1.0 = -7.0
        let neg_seven: u32 = 0xC0E00000; // -7.0
        execute_fpu_operation(&mut dut, TWO, THREE, ONE, 0, FPU_NMADD, 0);
        assert_eq!(
            dut.fp_result, neg_seven,
            "-(2.0 * 3.0) - 1.0 should equal -7.0"
        );
    });
}

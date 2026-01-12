use riscv_core::{create_fpu_runtime, Fpu};

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

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_fpu_runtime().expect("Failed to create FPU runtime")
}

// ========== Arithmetic Tests ==========

#[test]
fn test_fpu_add_basic() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 1.0 + 2.0 = 3.0
    dut.fs1 = ONE;
    dut.fs2 = TWO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_ADD;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, THREE, "1.0 + 2.0 should equal 3.0");
}

#[test]
fn test_fpu_add_negative() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 1.0 + (-1.0) = 0.0
    dut.fs1 = ONE;
    dut.fs2 = NEG_ONE;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_ADD;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, POS_ZERO, "1.0 + (-1.0) should equal 0.0");
}

#[test]
fn test_fpu_sub_basic() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 3.0 - 1.0 = 2.0
    dut.fs1 = THREE;
    dut.fs2 = ONE;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_SUB;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, TWO, "3.0 - 1.0 should equal 2.0");
}

#[test]
fn test_fpu_mul_basic() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 2.0 * 2.0 = 4.0
    dut.fs1 = TWO;
    dut.fs2 = TWO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_MUL;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, FOUR, "2.0 * 2.0 should equal 4.0");
}

// ========== Sign Injection Tests ==========

#[test]
fn test_fpu_sign_injection() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = ONE; // 1.0 (positive)
    dut.fs2 = NEG_ONE; // -1.0 (negative sign)
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.rm = 0;

    // FSGNJ: copy sign of fs2 to fs1 -> -1.0
    dut.fpu_op = FPU_SGNJ;
    dut.eval();
    assert_eq!(dut.fp_result, NEG_ONE, "fsgnj(1.0, -1.0) should equal -1.0");

    // FSGNJN: copy inverted sign of fs2 to fs1 -> 1.0
    dut.fpu_op = FPU_SGNJN;
    dut.eval();
    assert_eq!(dut.fp_result, ONE, "fsgnjn(1.0, -1.0) should equal 1.0");

    // FSGNJX: XOR signs -> -1.0
    dut.fpu_op = FPU_SGNJX;
    dut.eval();
    assert_eq!(
        dut.fp_result, NEG_ONE,
        "fsgnjx(1.0, -1.0) should equal -1.0"
    );
}

// ========== Comparison Tests ==========

#[test]
fn test_fpu_feq() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_FEQ;
    dut.rm = 0;

    // Test: 1.0 == 1.0 -> true (1)
    dut.fs1 = ONE;
    dut.fs2 = ONE;
    dut.eval();
    assert_eq!(dut.int_result, 1, "1.0 == 1.0 should be true");

    // Test: 1.0 == 2.0 -> false (0)
    dut.fs2 = TWO;
    dut.eval();
    assert_eq!(dut.int_result, 0, "1.0 == 2.0 should be false");
}

#[test]
fn test_fpu_flt() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_FLT;
    dut.rm = 0;

    // Test: 1.0 < 2.0 -> true (1)
    dut.fs1 = ONE;
    dut.fs2 = TWO;
    dut.eval();
    assert_eq!(dut.int_result, 1, "1.0 < 2.0 should be true");

    // Test: 2.0 < 1.0 -> false (0)
    dut.fs1 = TWO;
    dut.fs2 = ONE;
    dut.eval();
    assert_eq!(dut.int_result, 0, "2.0 < 1.0 should be false");
}

#[test]
fn test_fpu_fle() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_FLE;
    dut.rm = 0;

    // Test: 1.0 <= 2.0 -> true (1)
    dut.fs1 = ONE;
    dut.fs2 = TWO;
    dut.eval();
    assert_eq!(dut.int_result, 1, "1.0 <= 2.0 should be true");

    // Test: 1.0 <= 1.0 -> true (1)
    dut.fs2 = ONE;
    dut.eval();
    assert_eq!(dut.int_result, 1, "1.0 <= 1.0 should be true");

    // Test: 2.0 <= 1.0 -> false (0)
    dut.fs1 = TWO;
    dut.fs2 = ONE;
    dut.eval();
    assert_eq!(dut.int_result, 0, "2.0 <= 1.0 should be false");
}

// ========== MIN/MAX Tests ==========

#[test]
fn test_fpu_min_max() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = ONE;
    dut.fs2 = TWO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.rm = 0;

    // MIN: min(1.0, 2.0) = 1.0
    dut.fpu_op = FPU_MIN;
    dut.eval();
    assert_eq!(dut.fp_result, ONE, "min(1.0, 2.0) should equal 1.0");

    // MAX: max(1.0, 2.0) = 2.0
    dut.fpu_op = FPU_MAX;
    dut.eval();
    assert_eq!(dut.fp_result, TWO, "max(1.0, 2.0) should equal 2.0");
}

#[test]
fn test_fpu_min_max_signed_zero() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = POS_ZERO;
    dut.fs2 = NEG_ZERO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.rm = 0;

    // MIN: min(+0.0, -0.0) = -0.0
    dut.fpu_op = FPU_MIN;
    dut.eval();
    assert_eq!(dut.fp_result, NEG_ZERO, "min(+0.0, -0.0) should equal -0.0");

    // MAX: max(+0.0, -0.0) = +0.0
    dut.fpu_op = FPU_MAX;
    dut.eval();
    assert_eq!(dut.fp_result, POS_ZERO, "max(+0.0, -0.0) should equal +0.0");
}

// ========== Conversion Tests ==========

#[test]
fn test_fpu_fcvt_w_s() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_CVTWS;
    dut.rm = 0;

    // Test: float 3.0 -> int 3
    dut.fs1 = THREE;
    dut.eval();
    assert_eq!(dut.int_result, 3, "fcvt.w.s(3.0) should equal 3");

    // Test: float -1.0 -> int -1
    dut.fs1 = NEG_ONE;
    dut.eval();
    assert_eq!(dut.int_result as i32, -1, "fcvt.w.s(-1.0) should equal -1");
}

#[test]
fn test_fpu_fcvt_wu_s() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_CVTWUS;
    dut.rm = 0;

    // Test: float 3.0 -> unsigned int 3
    dut.fs1 = THREE;
    dut.eval();
    assert_eq!(dut.int_result, 3, "fcvt.wu.s(3.0) should equal 3");

    // Test: float -1.0 -> unsigned int 0 (saturates with NV flag)
    dut.fs1 = NEG_ONE;
    dut.eval();
    assert_eq!(dut.int_result, 0, "fcvt.wu.s(-1.0) should saturate to 0");
    assert_eq!(dut.fflags & 0b10000, 0b10000, "NV flag should be set");
}

#[test]
fn test_fpu_fcvt_s_w() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = 0;
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.fpu_op = FPU_CVTSW;
    dut.rm = 0;

    // Test: int 3 -> float 3.0
    dut.int_src = 3;
    dut.eval();
    assert_eq!(dut.fp_result, THREE, "fcvt.s.w(3) should equal 3.0");

    // Test: int -1 -> float -1.0
    dut.int_src = (-1i32) as u32;
    dut.eval();
    assert_eq!(dut.fp_result, NEG_ONE, "fcvt.s.w(-1) should equal -1.0");
}

#[test]
fn test_fpu_fcvt_s_wu() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = 0;
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.fpu_op = FPU_CVTSWU;
    dut.rm = 0;

    // Test: unsigned int 3 -> float 3.0
    dut.int_src = 3;
    dut.eval();
    assert_eq!(dut.fp_result, THREE, "fcvt.s.wu(3) should equal 3.0");

    // Test: unsigned int 100 -> float 100.0
    dut.int_src = 100;
    dut.eval();
    let hundred: u32 = 0x42C80000; // 100.0
    assert_eq!(dut.fp_result, hundred, "fcvt.s.wu(100) should equal 100.0");
}

// ========== Move and Classification Tests ==========

#[test]
fn test_fpu_fmv_x_w() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = ONE; // 0x3F800000
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_MVXW;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.int_result, ONE, "fmv.x.w should copy bits unchanged");
}

#[test]
fn test_fpu_fmv_w_x() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs1 = 0;
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = ONE; // 0x3F800000
    dut.fpu_op = FPU_MVWX;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, ONE, "fmv.w.x should copy bits unchanged");
}

#[test]
fn test_fpu_fclass() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_FCLASS;
    dut.rm = 0;

    // Test: fclass(-inf) = bit 0
    dut.fs1 = NEG_INF;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000001, "fclass(-inf) should be 0x001");

    // Test: fclass(-1.0) = bit 1 (negative normal)
    dut.fs1 = NEG_ONE;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000002, "fclass(-1.0) should be 0x002");

    // Test: fclass(-0.0) = bit 3
    dut.fs1 = NEG_ZERO;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000008, "fclass(-0.0) should be 0x008");

    // Test: fclass(+0.0) = bit 4
    dut.fs1 = POS_ZERO;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000010, "fclass(+0.0) should be 0x010");

    // Test: fclass(1.0) = bit 6 (positive normal)
    dut.fs1 = ONE;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000040, "fclass(1.0) should be 0x040");

    // Test: fclass(+inf) = bit 7
    dut.fs1 = POS_INF;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000080, "fclass(+inf) should be 0x080");

    // Test: fclass(QNaN) = bit 9
    dut.fs1 = QNAN;
    dut.eval();
    assert_eq!(dut.int_result, 0x00000200, "fclass(QNaN) should be 0x200");
}

// ========== Division Tests ==========

#[test]
fn test_fpu_div_basic() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 4.0 / 2.0 = 2.0
    dut.fs1 = FOUR;
    dut.fs2 = TWO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_DIV;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, TWO, "4.0 / 2.0 should equal 2.0");
}

#[test]
fn test_fpu_div_by_zero() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: 1.0 / 0.0 = +inf with DZ flag
    dut.fs1 = ONE;
    dut.fs2 = POS_ZERO;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_DIV;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, POS_INF, "1.0 / 0.0 should equal +inf");
    assert_eq!(dut.fflags & 0b01000, 0b01000, "DZ flag should be set");
}

// ========== Square Root Tests ==========

#[test]
fn test_fpu_sqrt_basic() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: sqrt(4.0) = 2.0
    dut.fs1 = FOUR;
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_SQRT;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, TWO, "sqrt(4.0) should equal 2.0");
}

#[test]
fn test_fpu_sqrt_negative() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: sqrt(-1.0) = NaN with NV flag
    dut.fs1 = NEG_ONE;
    dut.fs2 = 0;
    dut.fs3 = 0;
    dut.int_src = 0;
    dut.fpu_op = FPU_SQRT;
    dut.rm = 0;
    dut.eval();

    assert_eq!(dut.fp_result, QNAN, "sqrt(-1.0) should equal NaN");
    assert_eq!(dut.fflags & 0b10000, 0b10000, "NV flag should be set");
}

// ========== Fused Multiply-Add Tests ==========

#[test]
fn test_fpu_fmadd() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: FMADD 2.0 * 3.0 + 1.0 = 7.0
    dut.fs1 = TWO;
    dut.fs2 = THREE;
    dut.fs3 = ONE;
    dut.int_src = 0;
    dut.fpu_op = FPU_MADD;
    dut.rm = 0;
    dut.eval();

    let seven: u32 = 0x40E00000; // 7.0
    assert_eq!(dut.fp_result, seven, "2.0 * 3.0 + 1.0 should equal 7.0");
}

#[test]
fn test_fpu_fmsub() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: FMSUB 2.0 * 3.0 - 1.0 = 5.0
    dut.fs1 = TWO;
    dut.fs2 = THREE;
    dut.fs3 = ONE;
    dut.int_src = 0;
    dut.fpu_op = FPU_MSUB;
    dut.rm = 0;
    dut.eval();

    let five: u32 = 0x40A00000; // 5.0
    assert_eq!(dut.fp_result, five, "2.0 * 3.0 - 1.0 should equal 5.0");
}

#[test]
fn test_fpu_fnmsub() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: FNMSUB -(2.0 * 3.0) + 1.0 = -5.0
    dut.fs1 = TWO;
    dut.fs2 = THREE;
    dut.fs3 = ONE;
    dut.int_src = 0;
    dut.fpu_op = FPU_NMSUB;
    dut.rm = 0;
    dut.eval();

    let neg_five: u32 = 0xC0A00000; // -5.0
    assert_eq!(
        dut.fp_result, neg_five,
        "-(2.0 * 3.0) + 1.0 should equal -5.0"
    );
}

#[test]
fn test_fpu_fnmadd() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Fpu>().unwrap();

    // Test: FNMADD -(2.0 * 3.0) - 1.0 = -7.0
    dut.fs1 = TWO;
    dut.fs2 = THREE;
    dut.fs3 = ONE;
    dut.int_src = 0;
    dut.fpu_op = FPU_NMADD;
    dut.rm = 0;
    dut.eval();

    let neg_seven: u32 = 0xC0E00000; // -7.0
    assert_eq!(
        dut.fp_result, neg_seven,
        "-(2.0 * 3.0) - 1.0 should equal -7.0"
    );
}

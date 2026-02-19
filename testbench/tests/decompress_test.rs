// Tests for RV32C Instruction Decompressor
// Validates all 27 compressed instructions decompress correctly

use riscv_core::instruction::*;
use riscv_core::{create_decompress_runtime, Decompress};

// Helper function to test a single decompression
fn test_decompress(
    insn_16: u16,
    expected_insn_32: u32,
    should_be_compressed: bool,
    should_be_valid: bool,
) {
    let runtime = create_decompress_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<Decompress>()
        .expect("Failed to create model");

    dut.insn_16 = insn_16;
    // For compressed instructions, upper 16 bits don't matter
    // For non-compressed (32-bit marker), pass the expected value
    dut.insn_32_in = if should_be_compressed {
        insn_16 as u32
    } else {
        expected_insn_32
    };
    dut.eval();

    assert_eq!(
        dut.is_compressed,
        if should_be_compressed { 1 } else { 0 },
        "is_compressed mismatch for insn_16=0x{:04x}",
        insn_16
    );
    assert_eq!(
        dut.is_valid,
        if should_be_valid { 1 } else { 0 },
        "is_valid mismatch for insn_16=0x{:04x}",
        insn_16
    );
    if should_be_valid {
        assert_eq!(
            dut.insn_32, expected_insn_32,
            "Decompression mismatch for insn_16=0x{:04x}: expected 0x{:08x}, got 0x{:08x}",
            insn_16, expected_insn_32, dut.insn_32
        );
    } else {
        // For illegal instructions, decompressor may leave insn_32 as a default value (NOP).
        // Only verify is_valid is cleared (done above); don't assert on insn_32.
    }
}

// ============================================================
// Quadrant 0 Tests: C.ADDI4SPN, C.LW, C.SW
// ============================================================

#[test]
fn test_c_addi4spn_basic() {
    // C.ADDI4SPN x8, x2, 64
    // Expands to: ADDI x8, x2, 64
    let insn_16: u16 = c_addi4spn(8, 2, 64);
    let expected = addi(8, 2, 64);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi4spn_max() {
    // C.ADDI4SPN x15, x2, 1020
    // Expands to: ADDI x15, x2, 1020
    let insn_16: u16 = c_addi4spn(15, 2, 1020);
    let expected = addi(15, 2, 1020);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi4spn_illegal_zero() {
    // C.ADDI4SPN with nzuimm=0 is illegal
    let insn_16: u16 = c_addi4spn(8, 2, 0);
    test_decompress(insn_16, 0, true, false); // is_valid should be 0
}

#[test]
fn test_c_lw() {
    // C.LW x10, 4(x9)
    // Expands to: LW x10, 4(x9)
    let insn_16: u16 = c_lw(10, 9, 4);
    let expected = lw(10, 9, 4);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_sw() {
    // C.SW x11, 8(x8)
    // Expands to: SW x11, 8(x8)
    let insn_16: u16 = c_sw(8, 11, 8);
    let expected = sw(8, 11, 8);
    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Quadrant 1 Tests: Arithmetic, branches, jumps
// ============================================================

#[test]
fn test_c_nop() {
    // C.NOP
    // Expands to: ADDI x0, x0, 0
    let insn_16: u16 = c_nop();
    let expected = addi(0, 0, 0);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi() {
    // C.ADDI x10, 5
    // Expands to: ADDI x10, x10, 5
    let insn_16: u16 = c_addi(10, 5);
    let expected = addi(10, 10, 5);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi_negative() {
    // C.ADDI x10, -1
    // Expands to: ADDI x10, x10, -1
    let insn_16: u16 = c_addi(10, -1);
    let expected = addi(10, 10, -1);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jal() {
    // C.JAL offset (RV32 only, expands to JAL x1, offset)
    // offset encoding in C.JAL is complex, so let's test a simple case
    // For a simple forward jump of +4 bytes
    let insn_16: u16 = c_jal(4);
    let expected = jal(1, 4);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_li() {
    // C.LI x10, 5
    // Expands to: ADDI x10, x0, 5
    let insn_16: u16 = c_li(10, 5);
    let expected = addi(10, 0, 5);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_li_negative() {
    // C.LI x11, -1
    // Expands to: ADDI x11, x0, -1
    let insn_16: u16 = c_li(11, -1);
    let expected = addi(11, 0, -1);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi16sp() {
    // C.ADDI16SP x2, 16
    // Expands to: ADDI x2, x2, 16
    let insn_16: u16 = c_addi16sp(16);
    let expected = addi(2, 2, 16);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi16sp_illegal_zero() {
    // C.ADDI16SP with nzimm=0 is illegal
    let insn_16: u16 = c_addi16sp(0);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_lui() {
    // C.LUI x10, 1
    // Expands to: LUI x10, 1
    let insn_16: u16 = c_lui(10, 1);
    let expected = lui(10, 0x1000);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_lui_illegal_rd_zero() {
    // C.LUI with rd=0 is illegal
    let insn_16: u16 = c_lui(0, 1);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_srli() {
    // C.SRLI x8, 1
    // Expands to: SRLI x8, x8, 1
    let insn_16: u16 = c_srli(8, 1);
    let expected = srli(8, 8, 1);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_srai() {
    // C.SRAI x9, 2
    // Expands to: SRAI x9, x9, 2
    let insn_16: u16 = c_srai(9, 2);
    let expected = srai(9, 9, 2);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_andi() {
    // C.ANDI x10, 15
    // Expands to: ANDI x10, x10, 15
    let insn_16: u16 = c_andi(10, 15);
    let expected = andi(10, 10, 15);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_sub() {
    // C.SUB x8, x9
    // Expands to: SUB x8, x8, x9
    let insn_16: u16 = c_sub(8, 9);
    let expected = sub(8, 8, 9);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_xor() {
    // C.XOR x10, x11
    // Expands to: XOR x10, x10, x11
    let insn_16: u16 = c_xor(10, 11);
    let expected = xor(10, 10, 11);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_or() {
    // C.OR x12, x13
    // Expands to: OR x12, x12, x13
    let insn_16: u16 = c_or(12, 13);
    let expected = or(12, 12, 13);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_and() {
    // C.AND x14, x15
    // Expands to: AND x14, x14, x15
    let insn_16: u16 = c_and(14, 15);
    let expected = and(14, 14, 15);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_j() {
    // C.J offset
    // Expands to: JAL x0, offset
    let insn_16: u16 = c_j(4);
    let expected = jal(0, 4);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_beqz() {
    // C.BEQZ x8, offset
    // Expands to: BEQ x8, x0, 0
    let insn_16: u16 = c_beqz(8, 0);
    let expected = beq(8, 0, 0);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_bnez() {
    // C.BNEZ x9, offset
    // Expands to: BNE x9, x0, 0
    let insn_16: u16 = c_bnez(9, 0);
    let expected = bne(9, 0, 0);
    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Quadrant 2 Tests: Shifts, loads/stores, jumps
// ============================================================

#[test]
fn test_c_slli() {
    // C.SLLI x10, 1
    // Expands to: SLLI x10, x10, 1
    let insn_16: u16 = c_slli(10, 1);
    let expected = slli(10, 10, 1);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_slli_illegal_zero_shamt() {
    // C.SLLI with shamt=0 is illegal
    let insn_16: u16 = c_slli(10, 0);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_lwsp() {
    // C.LWSP x10, 4(x2)
    // Expands to: LW x10, 4(x2)
    let insn_16: u16 = c_lwsp(10, 4);
    let expected = lw(10, 2, 4);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_lwsp_illegal_rd_zero() {
    // C.LWSP with rd=0 is reserved/illegal
    let insn_16: u16 = c_lwsp(0, 4);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_jr() {
    // C.JR x10
    // Expands to: JALR x0, 0(x10)
    let insn_16: u16 = c_jr(10);
    let expected = jalr(0, 10, 0);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jr_illegal_rs1_zero() {
    // C.JR with rs1=0 is illegal
    let insn_16: u16 = c_jr(0);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_mv() {
    // C.MV x10, x11
    // Expands to: ADD x10, x0, x11
    let insn_16: u16 = c_mv(10, 11);
    let expected = add(10, 0, 11);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_mv_illegal_rd_zero() {
    // C.MV with rd=0 is illegal
    let insn_16: u16 = c_mv(0, 11);
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_ebreak() {
    // C.EBREAK
    // Expands to: EBREAK
    let insn_16: u16 = c_ebreak();
    let expected = ebreak();
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jalr() {
    // C.JALR x10
    // Expands to: JALR x1, 0(x10)
    let insn_16: u16 = c_jalr(10);
    let expected = jalr(1, 10, 0);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_add() {
    // C.ADD x10, x11
    // Expands to: ADD x10, x10, x11
    let insn_16: u16 = c_add(10, 11);
    let expected = add(10, 10, 11);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_swsp() {
    // C.SWSP x11, 8(x2)
    // Expands to: SW x11, 8(x2)
    let insn_16: u16 = c_swsp(11, 8);
    let expected = sw(2, 11, 8);

    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Edge Cases and Invalid Instructions
// ============================================================

#[test]
fn test_32bit_marker() {
    // Test that instructions with bits[1:0]=11 are marked as non-compressed
    let insn_16: u16 = 0b0000_0000_0000_0011; // bits[1:0]=11
                                              // Should pass through lower 16 bits, is_compressed=0
    let expected: u32 = 0x00000003;
    test_decompress(insn_16, expected, false, true);
}

#[test]
fn test_all_zeros_illegal() {
    // All zeros should be illegal
    let insn_16: u16 = 0x0000;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_quadrant_0_reserved() {
    // Reserved encodings in quadrant 0 should be invalid
    // Set bits[15:13]=001 and bits[1:0]=00 to get quadrant 0 funct3=001
    let insn_16: u16 = 0x2000; // funct3=001 is reserved in quadrant 0

    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_quadrant_2_reserved() {
    // Reserved encodings in quadrant 2 should be invalid
    let insn_16: u16 = 0b0010_0000_0000_0010; // funct3=001 is reserved in quadrant 2
    test_decompress(insn_16, 0, true, false);
}

// ============================================================
// RV32FC Compressed Floating-Point Instructions
// ============================================================

#[test]
fn test_c_flw_basic() {
    // C.FLW f10, 8(x9)   [f10 is compressed reg 10-8=2, x9 is compressed reg 9-8=1]
    let insn_16: u16 = c_flw(10, 9, 8);
    let expected = flw(10, 9, 8);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_flw_max_offset() {
    // C.FLW f8, 124(x8)
    let insn_16: u16 = c_flw(8, 8, 124);
    let expected = flw(8, 8, 124);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_fsw_basic() {
    // C.FSW x9, f10, 8
    let insn_16: u16 = c_fsw(9, 10, 8);
    let expected = fsw(9, 10, 8);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_fsw_max_offset() {
    // C.FSW x8, f15, 124
    let insn_16: u16 = c_fsw(8, 15, 124);
    let expected = fsw(8, 15, 124);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_flwsp_basic() {
    // C.FLWSP f14, 8(x2)
    let insn_16: u16 = c_flwsp(14, 8);
    let expected = flw(14, 2, 8);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_flwsp_known_encoding() {
    // Verify against actual binary seen from the Rust compiler
    // flw fa5, 12(sp) = 0x67b2
    let insn_16: u16 = c_flwsp(15, 12);
    assert_eq!(insn_16, 0x67b2, "Encoding mismatch for flw fa5, 12(sp)");
    let expected = flw(15, 2, 12);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_flwsp_known_encoding2() {
    // flw fa4, 8(sp) = 0x6722
    let insn_16: u16 = c_flwsp(14, 8);
    assert_eq!(insn_16, 0x6722, "Encoding mismatch for flw fa4, 8(sp)");
    let expected = flw(14, 2, 8);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_fswsp_basic() {
    // C.FSWSP f11, 8(x2)
    let insn_16: u16 = c_fswsp(11, 8);
    let expected = fsw(2, 11, 8);
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_fswsp_known_encoding() {
    // Verify against actual binary seen from the Rust compiler
    // fsw fa5, 8(sp) = 0xe43e
    let insn_16: u16 = c_fswsp(15, 8);
    assert_eq!(insn_16, 0xe43e, "Encoding mismatch for fsw fa5, 8(sp)");
    let expected = fsw(2, 15, 8);
    test_decompress(insn_16, expected, true, true);
}

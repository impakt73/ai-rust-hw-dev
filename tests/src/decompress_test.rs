// Tests for RV32C Instruction Decompressor
// Validates all 27 compressed instructions decompress correctly

use riscv_core::{create_decompress_runtime, Decompress};

// Helper function to test a single decompression
fn test_decompress(insn_16: u16, expected_insn_32: u32, should_be_compressed: bool, should_be_valid: bool) {
    let runtime = create_decompress_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().expect("Failed to create model");
    
    dut.insn_16 = insn_16;
    dut.eval();
    
    assert_eq!(dut.is_compressed, if should_be_compressed { 1 } else { 0 }, 
               "is_compressed mismatch for insn_16=0x{:04x}", insn_16);
    assert_eq!(dut.is_valid, if should_be_valid { 1 } else { 0 }, 
               "is_valid mismatch for insn_16=0x{:04x}", insn_16);
    assert_eq!(dut.insn_32, expected_insn_32, 
               "Decompression mismatch for insn_16=0x{:04x}: expected 0x{:08x}, got 0x{:08x}", 
               insn_16, expected_insn_32, dut.insn_32);
}

// ============================================================
// Quadrant 0 Tests: C.ADDI4SPN, C.LW, C.SW
// ============================================================

#[test]
fn test_c_addi4spn_basic() {
    // C.ADDI4SPN x8, x2, 64
    // Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
    // nzuimm = 64 = 0b0001000000 = {insn[10:7]=0001, insn[12:11]=00, insn[5]=0, insn[6]=0, 2'b00}
    let insn_16: u16 = 0b000_00_0100_000_00;  // funct3=000, nzuimm bits, rd'=000(x8), op=00
    // ADDI x8, x2, 64
    let expected: u32 = (64 << 20) | (2 << 15) | (0 << 12) | (8 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi4spn_max() {
    // C.ADDI4SPN with maximum immediate (1020)
    // nzuimm = 1020 = 0b1111111100
    let insn_16: u16 = 0b000_11_1111_111_00;
    let expected: u32 = (1020 << 20) | (2 << 15) | (0 << 12) | (15 << 7) | 0b0010011; // rd'=111=x15
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi4spn_illegal_zero() {
    // C.ADDI4SPN with nzuimm=0 is illegal
    let insn_16: u16 = 0b000_00_0000_000_00;
    test_decompress(insn_16, 0, true, false);  // is_valid should be 0
}

#[test]
fn test_c_lw() {
    // C.LW x10, 4(x9)
    // Format: 010 uimm[5:3] rs1' uimm[2|6] rd' 00
    // uimm = 4 = {insn[5]=0, insn[12:10]=001, insn[6]=0, 2'b00}
    let insn_16: u16 = 0b010_001_001_00_010_00;  // funct3=010, uimm, rs1'=001(x9), rd'=010(x10), op=00
    // LW x10, 4(x9)
    let expected: u32 = (4 << 20) | (9 << 15) | (2 << 12) | (10 << 7) | 0b0000011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_sw() {
    // C.SW x11, 8(x8)
    // uimm = 8 = {insn[5]=0, insn[12:10]=010, insn[6]=0, 2'b00}
    let insn_16: u16 = 0b110_010_000_00_011_00;  // funct3=110, rs1'=000(x8), rs2'=011(x11)
    // SW x11, 8(x8)
    let expected: u32 = (0 << 25) | (11 << 20) | (8 << 15) | (2 << 12) | (8 << 7) | 0b0100011;
    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Quadrant 1 Tests: Arithmetic, branches, jumps
// ============================================================

#[test]
fn test_c_nop() {
    // C.NOP (rd=0, imm=0)
    let insn_16: u16 = 0b000_0_00000_00000_01;
    // ADDI x0, x0, 0
    let expected: u32 = 0x00000013;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi() {
    // C.ADDI x10, 5
    // imm = 5 = {insn[12]=0, insn[6:2]=00101}
    let insn_16: u16 = 0b000_0_01010_00101_01;  // funct3=000, rd=x10, imm=5, op=01
    // ADDI x10, x10, 5
    let expected: u32 = (5 << 20) | (10 << 15) | (0 << 12) | (10 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi_negative() {
    // C.ADDI x10, -1
    // imm = -1 (6-bit signed) = 0b111111
    let insn_16: u16 = 0b000_1_01010_11111_01;
    // ADDI x10, x10, -1 (sign-extended to 32 bits)
    let expected: u32 = (0xFFFFFFFF_u32 << 20) | (10 << 15) | (0 << 12) | (10 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jal() {
    // C.JAL offset (RV32 only, expands to JAL x1, offset)
    // offset encoding in C.JAL is complex, so let's test a simple case
    // For a simple forward jump of +4 bytes
    let insn_16: u16 = 0b001_0_0_0_1_0_0_0_010_01;
    // JAL x1, 4 (encoded as immediate)
    let expected: u32 = 0x004000ef;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_li() {
    // C.LI x10, 5
    // Expands to ADDI x10, x0, 5
    let insn_16: u16 = 0b010_0_01010_00101_01;
    let expected: u32 = (5 << 20) | (0 << 15) | (0 << 12) | (10 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_li_negative() {
    // C.LI x11, -1
    let insn_16: u16 = 0b010_1_01011_11111_01;
    let expected: u32 = (0xFFFFFFFF_u32 << 20) | (0 << 15) | (0 << 12) | (11 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi16sp() {
    // C.ADDI16SP x2, 16
    // nzimm = 16 = {insn[12]=0, insn[4:3]=01, insn[5]=0, insn[2]=0, insn[6]=0, 4'b0000}
    let insn_16: u16 = 0b011_0_00010_01000_01;  // rd=x2, nzimm encodes to 16
    // ADDI x2, x2, 16
    let expected: u32 = (16 << 20) | (2 << 15) | (0 << 12) | (2 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_addi16sp_illegal_zero() {
    // C.ADDI16SP with nzimm=0 is illegal
    let insn_16: u16 = 0b011_0_00010_00000_01;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_lui() {
    // C.LUI x10, 1
    // nzimm = 1 = {insn[12]=0, insn[6:2]=00001}
    let insn_16: u16 = 0b011_0_01010_00001_01;
    // LUI x10, 1
    let expected: u32 = (1 << 12) | (10 << 7) | 0b0110111;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_lui_illegal_rd_zero() {
    // C.LUI with rd=0 is illegal
    let insn_16: u16 = 0b011_0_00000_00001_01;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_srli() {
    // C.SRLI x8, 1
    let insn_16: u16 = 0b100_0_00_000_00001_01;  // funct3=100, funct2=00, rs1'/rd'=000(x8), shamt=1
    // SRLI x8, x8, 1
    let expected: u32 = (0 << 25) | (1 << 20) | (8 << 15) | (5 << 12) | (8 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_srai() {
    // C.SRAI x9, 2
    let insn_16: u16 = 0b100_0_01_001_00010_01;  // funct2=01, rs1'=001(x9), shamt=2
    // SRAI x9, x9, 2
    let expected: u32 = (0b0100000 << 25) | (2 << 20) | (9 << 15) | (5 << 12) | (9 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_andi() {
    // C.ANDI x10, 15
    let insn_16: u16 = 0b100_0_10_010_01111_01;  // funct2=10, rs1'=010(x10), imm=15
    // ANDI x10, x10, 15
    let expected: u32 = (15 << 20) | (10 << 15) | (7 << 12) | (10 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_sub() {
    // C.SUB x8, x9
    let insn_16: u16 = 0b100_0_11_000_00_001_01;  // funct2=11, bit12=0, funct2_ca=00, rs1'=000, rs2'=001
    // SUB x8, x8, x9
    let expected: u32 = (0b0100000 << 25) | (9 << 20) | (8 << 15) | (0 << 12) | (8 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_xor() {
    // C.XOR x10, x11
    let insn_16: u16 = 0b100_0_11_010_01_011_01;  // rs1'=010(x10), rs2'=011(x11), funct2_ca=01
    // XOR x10, x10, x11
    let expected: u32 = (0 << 25) | (11 << 20) | (10 << 15) | (4 << 12) | (10 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_or() {
    // C.OR x12, x13
    let insn_16: u16 = 0b100_0_11_100_10_101_01;  // rs1'=100(x12), rs2'=101(x13), funct2_ca=10
    // OR x12, x12, x13
    let expected: u32 = (0 << 25) | (13 << 20) | (12 << 15) | (6 << 12) | (12 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_and() {
    // C.AND x14, x15
    let insn_16: u16 = 0b100_0_11_110_11_111_01;  // rs1'=110(x14), rs2'=111(x15), funct2_ca=11
    // AND x14, x14, x15
    let expected: u32 = (0 << 25) | (15 << 20) | (14 << 15) | (7 << 12) | (14 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_j() {
    // C.J offset
    // Expands to JAL x0, offset
    let insn_16: u16 = 0b101_0_0_0_1_0_0_0_010_01;  // Similar to C.JAL but rd=x0
    let expected: u32 = 0x0040006f;  // JAL x0, 4
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_beqz() {
    // C.BEQZ x8, offset
    // Expands to BEQ x8, x0, offset
    let insn_16: u16 = 0b110_0_00_000_00_000_01;  // rs1'=000(x8), offset bits
    // BEQ x8, x0, 0
    let expected: u32 = (0 << 25) | (0 << 20) | (8 << 15) | (0 << 12) | (0 << 7) | 0b1100011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_bnez() {
    // C.BNEZ x9, offset
    // Expands to BNE x9, x0, offset
    let insn_16: u16 = 0b111_0_00_001_00_000_01;  // rs1'=001(x9)
    // BNE x9, x0, 0
    let expected: u32 = (0 << 25) | (0 << 20) | (9 << 15) | (1 << 12) | (0 << 7) | 0b1100011;
    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Quadrant 2 Tests: Shifts, loads/stores, jumps
// ============================================================

#[test]
fn test_c_slli() {
    // C.SLLI x10, 1
    let insn_16: u16 = 0b000_0_01010_00001_10;  // rd=x10, shamt=1
    // SLLI x10, x10, 1
    let expected: u32 = (0 << 25) | (1 << 20) | (10 << 15) | (1 << 12) | (10 << 7) | 0b0010011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_slli_illegal_zero_shamt() {
    // C.SLLI with shamt=0 is illegal
    let insn_16: u16 = 0b000_0_01010_00000_10;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_lwsp() {
    // C.LWSP x10, 4(x2)
    // uimm = 4 = {insn[3:2]=01, insn[12]=0, insn[6:4]=000, 2'b00}
    let insn_16: u16 = 0b010_0_01010_01000_10;  // rd=x10, uimm encodes to 4
    // LW x10, 4(x2)
    let expected: u32 = (4 << 20) | (2 << 15) | (2 << 12) | (10 << 7) | 0b0000011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_lwsp_illegal_rd_zero() {
    // C.LWSP with rd=0 is reserved/illegal
    let insn_16: u16 = 0b010_0_00000_01000_10;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_jr() {
    // C.JR x10
    // Expands to JALR x0, 0(x10)
    let insn_16: u16 = 0b100_0_01010_00000_10;  // bit12=0, rs1=x10, rs2=0
    // JALR x0, 0(x10)
    let expected: u32 = (0 << 20) | (10 << 15) | (0 << 12) | (0 << 7) | 0b1100111;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jr_illegal_rs1_zero() {
    // C.JR with rs1=0 is illegal
    let insn_16: u16 = 0b100_0_00000_00000_10;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_mv() {
    // C.MV x10, x11
    // Expands to ADD x10, x0, x11
    let insn_16: u16 = 0b100_0_01010_01011_10;  // bit12=0, rd=x10, rs2=x11
    // ADD x10, x0, x11
    let expected: u32 = (0 << 25) | (11 << 20) | (0 << 15) | (0 << 12) | (10 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_mv_illegal_rd_zero() {
    // C.MV with rd=0 is illegal
    let insn_16: u16 = 0b100_0_00000_01011_10;
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_c_ebreak() {
    // C.EBREAK
    let insn_16: u16 = 0b100_1_00000_00000_10;  // bit12=1, rd=0, rs2=0
    // EBREAK
    let expected: u32 = 0x00100073;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_jalr() {
    // C.JALR x10
    // Expands to JALR x1, 0(x10)
    let insn_16: u16 = 0b100_1_01010_00000_10;  // bit12=1, rs1=x10, rs2=0
    // JALR x1, 0(x10)
    let expected: u32 = (0 << 20) | (10 << 15) | (0 << 12) | (1 << 7) | 0b1100111;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_add() {
    // C.ADD x10, x11
    // Expands to ADD x10, x10, x11
    let insn_16: u16 = 0b100_1_01010_01011_10;  // bit12=1, rd=x10, rs2=x11
    // ADD x10, x10, x11
    let expected: u32 = (0 << 25) | (11 << 20) | (10 << 15) | (0 << 12) | (10 << 7) | 0b0110011;
    test_decompress(insn_16, expected, true, true);
}

#[test]
fn test_c_swsp() {
    // C.SWSP x11, 8(x2)
    // uimm = 8 = {insn[8:7]=10, insn[12:9]=0000, 2'b00}
    let insn_16: u16 = 0b110_0000_10_01011_10;  // rs2=x11, uimm encodes to 8
    // SW x11, 8(x2)
    let expected: u32 = (0 << 25) | (11 << 20) | (2 << 15) | (2 << 12) | (8 << 7) | 0b0100011;
    test_decompress(insn_16, expected, true, true);
}

// ============================================================
// Edge Cases and Invalid Instructions
// ============================================================

#[test]
fn test_32bit_marker() {
    // Test that instructions with bits[1:0]=11 are marked as non-compressed
    let insn_16: u16 = 0b0000_0000_0000_0011;  // bits[1:0]=11
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
    let insn_16: u16 = 0b001_00000_000_00;  // funct3=001 is reserved in quadrant 0
    test_decompress(insn_16, 0, true, false);
}

#[test]
fn test_quadrant_2_reserved() {
    // Reserved encodings in quadrant 2 should be invalid
    let insn_16: u16 = 0b001_0_00000_00000_10;  // funct3=001 is reserved in quadrant 2
    test_decompress(insn_16, 0, true, false);
}

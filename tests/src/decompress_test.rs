use riscv_core::{create_decompress_runtime, Decompress};

// Helper functions to encode standard RISC-V 32-bit instructions
fn encode_i_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_r_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, rs2: u32, funct7: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_u_type(opcode: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFFF000) | (rd << 7) | opcode
}

// ========== Quadrant 0 Tests ==========

#[test]
fn test_decompress_c_addi4spn() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI4SPN: addi rd', x2, nzuimm
    // Test with nzuimm = 64, rd' = x8 (000 in compressed encoding)
    // Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
    // nzuimm = 64 = 0b0001000000
    // bits: insn[12:11]=nzuimm[5:4]=00, insn[10:7]=nzuimm[9:6]=0001, insn[6]=nzuimm[3]=0, insn[5]=nzuimm[2]=0
    let nzuimm = 64_u32;
    let insn_16: u16 = (0b000 << 13) | 
                       (((nzuimm >> 4) & 0x3) as u16) << 11 |  // nzuimm[5:4]
                       (((nzuimm >> 6) & 0xF) as u16) << 7 |   // nzuimm[9:6]
                       (((nzuimm >> 3) & 0x1) as u16) << 6 |   // nzuimm[3]
                       (((nzuimm >> 2) & 0x1) as u16) << 5 |   // nzuimm[2]
                       (0b000 << 2) | 0b00;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x8, x2, 64
    let expected = encode_i_type(0b0010011, 8, 0b000, 2, 64);
    
    assert_eq!(dut.insn_32, expected, "C.ADDI4SPN decompression failed");
    assert_eq!(dut.is_compressed, 1, "Should be marked as compressed");
    assert_eq!(dut.is_valid, 1, "Should be valid");
}

#[test]
fn test_decompress_c_addi4spn_invalid_zero() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI4SPN with nzuimm = 0 is illegal
    let insn_16: u16 = (0b000 << 13) | (0b000 << 2) | 0b00;
    
    dut.insn_16 = insn_16;
    dut.eval();

    assert_eq!(dut.is_compressed, 1, "Should be marked as compressed");
    assert_eq!(dut.is_valid, 0, "Should be invalid (nzuimm == 0)");
}

#[test]
fn test_decompress_c_lw() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LW: lw rd', offset(rs1')
    // Test with offset = 4, rd' = x10 (010), rs1' = x9 (001)
    // Format: 010 offset[5:3] rs1' offset[2|6] rd' 00
    // offset = 4 = 0b0000100
    // insn[12:10]=offset[5:3]=000, insn[6]=offset[2]=1, insn[5]=offset[6]=0
    // Note: offset[2|6] notation means insn[6:5] = {offset[2], offset[6]}
    let offset = 4_u32;
    let insn_16: u16 = (0b010 << 13) |
                       (((offset >> 3) & 0x7) as u16) << 10 |  // offset[5:3]
                       (0b001 << 7) |  // rs1'
                       (((offset >> 2) & 0x1) as u16) << 6 |   // offset[2]
                       (((offset >> 6) & 0x1) as u16) << 5 |   // offset[6]
                       (0b010 << 2) | 0b00;  // rd'
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lw x10, 4(x9)
    let expected = encode_i_type(0b0000011, 10, 0b010, 9, 4);
    
    assert_eq!(dut.insn_32, expected, "C.LW decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_sw() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SW: sw rs2', offset(rs1')
    // Test with offset = 8, rs2' = x11 (011), rs1' = x8 (000)
    // Format: 110 offset[5:3] rs1' offset[2|6] rs2' 00
    // Note: offset[2|6] notation means insn[6:5] = {offset[2], offset[6]}
    let offset = 8_u32;
    let insn_16: u16 = (0b110 << 13) |
                       (((offset >> 3) & 0x7) as u16) << 10 |  // offset[5:3]
                       (0b000 << 7) |  // rs1'
                       (((offset >> 2) & 0x1) as u16) << 6 |   // offset[2]
                       (((offset >> 6) & 0x1) as u16) << 5 |   // offset[6]
                       (0b011 << 2) | 0b00;  // rs2'
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sw x11, 8(x8)
    // For S-type, we need special encoding
    let imm = 8_u32;
    let expected = ((imm >> 5) << 25) | (11 << 20) | (8 << 15) | (0b010 << 12) | ((imm & 0x1F) << 7) | 0b0100011;
    
    assert_eq!(dut.insn_32, expected, "C.SW decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

// ========== Quadrant 1 Tests ==========

#[test]
fn test_decompress_c_nop() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.NOP: addi x0, x0, 0
    // Format: 000 0 00000 0 01
    let insn_16: u16 = 0b000_0_00000_0_01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x0, x0, 0
    let expected = 0x00000013_u32;
    
    assert_eq!(dut.insn_32, expected, "C.NOP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI: addi rd, rd, nzimm
    // Test with rd = x10, imm = 5
    // Format: 000 imm[5] rd imm[4:0] 01
    let insn_16: u16 = (0b000 << 13) | (0b0 << 12) | (10 << 7) | (5 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x10, x10, 5
    let expected = encode_i_type(0b0010011, 10, 0b000, 10, 5);
    
    assert_eq!(dut.insn_32, expected, "C.ADDI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi_negative() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI with negative immediate (-1)
    // imm[5:0] = 0b111111 (sign-extended to -1)
    let insn_16: u16 = (0b000 << 13) | (0b1 << 12) | (10 << 7) | (0b11111 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x10, x10, -1
    let expected = encode_i_type(0b0010011, 10, 0b000, 10, -1);
    
    assert_eq!(dut.insn_32, expected, "C.ADDI negative decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_jal() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JAL: jal x1, offset (RV32C only)
    // Test with small offset (simplified pattern)
    // Format: 001 offset[11|4|9:8|10|6|7|3:1|5] 01
    // Let's use offset = 8 (0b00000000001000)
    let insn_16: u16 = (0b001 << 13) | (0b0 << 12) | (0b0 << 11) | (0b01 << 9) | 
                       (0b0 << 8) | (0b0 << 7) | (0b0 << 6) | (0b000 << 3) | 
                       (0b0 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jal x1, offset
    // Just verify it's a valid JAL instruction to x1
    assert_eq!(dut.insn_32 & 0x7F, 0b1101111, "Should be JAL opcode");
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 1, "Should target x1");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_li() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LI: addi rd, x0, imm
    // Test with rd = x10, imm = 5 (positive, fits in 6 bits)
    // Format: 010 imm[5] rd imm[4:0] 01
    let imm = 5_i32;
    let insn_16: u16 = (0b010 << 13) |
                       ((((imm >> 5) & 0x1) as u16) << 12) |  // imm[5] (sign bit for 6-bit imm)
                       (10 << 7) |  // rd
                       (((imm & 0x1F) as u16) << 2) |  // imm[4:0]
                       0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x10, x0, 5
    let expected = encode_i_type(0b0010011, 10, 0b000, 0, 5);
    
    assert_eq!(dut.insn_32, expected, "C.LI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi16sp() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI16SP: addi x2, x2, nzimm (must be multiple of 16, non-zero, sign-extended 10-bit)
    // Format: 011 nzimm[9] 2 nzimm[4|6|8:7|5] 01
    // Test with nzimm = 16 (0b0000010000)
    // insn[12]=nzimm[9]=0, insn[6]=nzimm[4]=1, insn[5]=nzimm[6]=0, insn[4:3]=nzimm[8:7]=00, insn[2]=nzimm[5]=0
    let nzimm = 16_u32;
    let insn_16: u16 = (0b011 << 13) |
                       ((((nzimm >> 9) & 0x1) as u16) << 12) |  // nzimm[9]
                       (2 << 7) |  // rd = x2
                       ((((nzimm >> 4) & 0x1) as u16) << 6) |   // nzimm[4]
                       ((((nzimm >> 6) & 0x1) as u16) << 5) |   // nzimm[6]
                       ((((nzimm >> 7) & 0x3) as u16) << 3) |   // nzimm[8:7]
                       ((((nzimm >> 5) & 0x1) as u16) << 2) |   // nzimm[5]
                       0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x2, x2, 16
    let expected = encode_i_type(0b0010011, 2, 0b000, 2, 16);
    
    assert_eq!(dut.insn_32, expected, "C.ADDI16SP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_lui() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LUI: lui rd, nzimm (rd != 0, 2)
    // Format: 011 nzimm[17] rd nzimm[16:12] 01
    // Test with rd = x3, nzimm[17:12] = 1
    let insn_16: u16 = (0b011 << 13) | (0b0 << 12) | (3 << 7) | (1 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lui x3, 1
    let expected = encode_u_type(0b0110111, 3, 1 << 12);
    
    assert_eq!(dut.insn_32, expected, "C.LUI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_srli() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SRLI: srli rd', rd', shamt
    // Format: 100 0 00 rs1'/rd' shamt 01
    // Test with rd' = x8 (000), shamt = 5
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b00 << 10) | (0b000 << 7) | (5 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: srli x8, x8, 5
    let expected = encode_i_type(0b0010011, 8, 0b101, 8, 5);
    
    assert_eq!(dut.insn_32, expected, "C.SRLI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_srai() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SRAI: srai rd', rd', shamt
    // Format: 100 0 01 rs1'/rd' shamt 01
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b01 << 10) | (0b001 << 7) | (3 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: srai x9, x9, 3
    let expected = encode_r_type(0b0010011, 9, 0b101, 9, 3, 0b0100000);
    
    assert_eq!(dut.insn_32, expected, "C.SRAI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_andi() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ANDI: andi rd', rd', imm
    // Format: 100 imm[5] 10 rs1'/rd' imm[4:0] 01
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b10 << 10) | (0b010 << 7) | (0b01111 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: andi x10, x10, 15
    let expected = encode_i_type(0b0010011, 10, 0b111, 10, 15);
    
    assert_eq!(dut.insn_32, expected, "C.ANDI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_sub() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SUB: sub rd', rd', rs2'
    // Format: 100 0 11 rd'/rs1' 00 rs2' 01
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b11 << 10) | (0b001 << 7) | (0b00 << 5) | (0b010 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sub x9, x9, x10
    let expected = encode_r_type(0b0110011, 9, 0b000, 9, 10, 0b0100000);
    
    assert_eq!(dut.insn_32, expected, "C.SUB decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_xor() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.XOR: xor rd', rd', rs2'
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b11 << 10) | (0b000 << 7) | (0b01 << 5) | (0b001 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: xor x8, x8, x9
    let expected = encode_r_type(0b0110011, 8, 0b100, 8, 9, 0b0000000);
    
    assert_eq!(dut.insn_32, expected, "C.XOR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_or() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.OR: or rd', rd', rs2'
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b11 << 10) | (0b011 << 7) | (0b10 << 5) | (0b100 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: or x11, x11, x12
    let expected = encode_r_type(0b0110011, 11, 0b110, 11, 12, 0b0000000);
    
    assert_eq!(dut.insn_32, expected, "C.OR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_and() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.AND: and rd', rd', rs2'
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (0b11 << 10) | (0b010 << 7) | (0b11 << 5) | (0b011 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: and x10, x10, x11
    let expected = encode_r_type(0b0110011, 10, 0b111, 10, 11, 0b0000000);
    
    assert_eq!(dut.insn_32, expected, "C.AND decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_j() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.J: jal x0, offset
    let insn_16: u16 = (0b101 << 13) | (0b0 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jal x0, offset (check opcode and rd)
    assert_eq!(dut.insn_32 & 0x7F, 0b1101111, "Should be JAL opcode");
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 0, "Should target x0");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_beqz() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.BEQZ: beq rs1', x0, offset
    let insn_16: u16 = (0b110 << 13) | (0b001 << 7) | (0b00000 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: beq x9, x0, offset
    assert_eq!(dut.insn_32 & 0x7F, 0b1100011, "Should be BRANCH opcode");
    assert_eq!((dut.insn_32 >> 12) & 0x7, 0b000, "Should be BEQ funct3");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 9, "Should use rs1 = x9");
    assert_eq!((dut.insn_32 >> 20) & 0x1F, 0, "Should use rs2 = x0");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_bnez() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.BNEZ: bne rs1', x0, offset
    let insn_16: u16 = (0b111 << 13) | (0b010 << 7) | (0b00000 << 2) | 0b01;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: bne x10, x0, offset
    assert_eq!(dut.insn_32 & 0x7F, 0b1100011, "Should be BRANCH opcode");
    assert_eq!((dut.insn_32 >> 12) & 0x7, 0b001, "Should be BNE funct3");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 10, "Should use rs1 = x10");
    assert_eq!((dut.insn_32 >> 20) & 0x1F, 0, "Should use rs2 = x0");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

// ========== Quadrant 2 Tests ==========

#[test]
fn test_decompress_c_slli() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SLLI: slli rd, rd, shamt
    let insn_16: u16 = (0b000 << 13) | (0b0 << 12) | (10 << 7) | (4 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: slli x10, x10, 4
    let expected = encode_i_type(0b0010011, 10, 0b001, 10, 4);
    
    assert_eq!(dut.insn_32, expected, "C.SLLI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_lwsp() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LWSP: lw rd, offset(x2)
    // Format: 010 offset[5] rd offset[4:2|7:6] 10
    let insn_16: u16 = (0b010 << 13) | (0b0 << 12) | (10 << 7) | (0b00001 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lw x10, offset(x2)
    assert_eq!(dut.insn_32 & 0x7F, 0b0000011, "Should be LOAD opcode");
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 10, "Should target x10");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 2, "Should use x2 as base");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_jr() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JR: jalr x0, 0(rs1)
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (10 << 7) | (0 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jalr x0, 0(x10)
    assert_eq!(dut.insn_32 & 0x7F, 0b1100111, "Should be JALR opcode");
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 0, "Should target x0");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 10, "Should use x10 as base");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_mv() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.MV: add rd, x0, rs2
    let insn_16: u16 = (0b100 << 13) | (0b0 << 12) | (10 << 7) | (11 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: add x10, x0, x11
    let expected = encode_r_type(0b0110011, 10, 0b000, 0, 11, 0b0000000);
    
    assert_eq!(dut.insn_32, expected, "C.MV decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_ebreak() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.EBREAK
    let insn_16: u16 = (0b100 << 13) | (0b1 << 12) | (0 << 7) | (0 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: ebreak (0x00100073)
    assert_eq!(dut.insn_32, 0x00100073, "C.EBREAK decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_jalr() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JALR: jalr x1, 0(rs1)
    let insn_16: u16 = (0b100 << 13) | (0b1 << 12) | (10 << 7) | (0 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jalr x1, 0(x10)
    assert_eq!(dut.insn_32 & 0x7F, 0b1100111, "Should be JALR opcode");
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 1, "Should target x1");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 10, "Should use x10 as base");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_add() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADD: add rd, rd, rs2
    let insn_16: u16 = (0b100 << 13) | (0b1 << 12) | (10 << 7) | (11 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: add x10, x10, x11
    let expected = encode_r_type(0b0110011, 10, 0b000, 10, 11, 0b0000000);
    
    assert_eq!(dut.insn_32, expected, "C.ADD decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_swsp() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SWSP: sw rs2, offset(x2)
    let insn_16: u16 = (0b110 << 13) | (0b0001 << 9) | (10 << 2) | 0b10;
    
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sw x10, offset(x2)
    assert_eq!(dut.insn_32 & 0x7F, 0b0100011, "Should be STORE opcode");
    assert_eq!((dut.insn_32 >> 20) & 0x1F, 10, "Should store x10");
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 2, "Should use x2 as base");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

// ========== Edge Cases ==========

#[test]
fn test_decompress_all_zeros() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // All zeros is illegal
    dut.insn_16 = 0x0000;
    dut.eval();

    assert_eq!(dut.is_valid, 0, "All zeros should be invalid");
}

#[test]
fn test_decompress_standard_instruction() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // Standard 32-bit instruction (bits [1:0] == 2'b11)
    // This is just the lower 16 bits; full instruction needs fetch from memory
    dut.insn_16 = 0x0013;  // Lower half of NOP (addi x0, x0, 0)
    dut.eval();

    assert_eq!(dut.is_compressed, 0, "Should not be marked as compressed");
    assert_eq!(dut.is_valid, 0, "Should be invalid (incomplete 32-bit instruction)");
}

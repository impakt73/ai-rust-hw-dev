//! RISC-V instruction encoding utilities
//!
//! This module provides functions for encoding RISC-V instructions from their
//! constituent parts into 32-bit instruction words. These utilities are useful
//! for test code generation and instruction sequence creation.

/// Encode an I-type instruction
///
/// Format: imm[11:0] | rs1 | funct3 | rd | opcode
pub fn encode_i_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

/// Encode an R-type instruction
///
/// Format: funct7 | rs2 | rs1 | funct3 | rd | opcode
pub fn encode_r_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, rs2: u32, funct7: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

/// Encode a U-type instruction
///
/// Format: imm[31:12] | rd | opcode
pub fn encode_u_type(opcode: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFFF000) | (rd << 7) | opcode
}

/// Encode a B-type instruction
///
/// Format: imm[12|10:5] | rs2 | rs1 | funct3 | imm[4:1|11] | opcode
pub fn encode_b_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = imm as u32;
    let imm_12 = (imm_u >> 12) & 0x1;
    let imm_10_5 = (imm_u >> 5) & 0x3F;
    let imm_4_1 = (imm_u >> 1) & 0xF;
    let imm_11 = (imm_u >> 11) & 0x1;
    (imm_12 << 31)
        | (imm_10_5 << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (funct3 << 12)
        | (imm_4_1 << 8)
        | (imm_11 << 7)
        | opcode
}

/// Encode an S-type instruction
///
/// Format: imm[11:5] | rs2 | rs1 | funct3 | imm[4:0] | opcode
pub fn encode_s_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    let imm_11_5 = (imm_u >> 5) & 0x7F;
    let imm_4_0 = imm_u & 0x1F;
    (imm_11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm_4_0 << 7) | opcode
}

// ============================================================================
// RV32I Base Instruction Set
// ============================================================================

// Arithmetic Instructions

/// ADDI: Add Immediate
pub fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b000, rs1, imm)
}

/// ADD: Add
pub fn add(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b000, rs1, rs2, 0b0000000)
}

/// SUB: Subtract
pub fn sub(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b000, rs1, rs2, 0b0100000)
}

// Logic Instructions

/// AND: Bitwise AND
pub fn and(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b111, rs1, rs2, 0b0000000)
}

/// ANDI: Bitwise AND Immediate
pub fn andi(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b111, rs1, imm)
}

/// OR: Bitwise OR
pub fn or(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b110, rs1, rs2, 0b0000000)
}

/// ORI: Bitwise OR Immediate
pub fn ori(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b110, rs1, imm)
}

/// XOR: Bitwise XOR
pub fn xor(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b100, rs1, rs2, 0b0000000)
}

/// XORI: Bitwise XOR Immediate
pub fn xori(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b100, rs1, imm)
}

// Shift Instructions

/// SLL: Shift Left Logical
pub fn sll(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b001, rs1, rs2, 0b0000000)
}

/// SLLI: Shift Left Logical Immediate
pub fn slli(rd: u32, rs1: u32, shamt: u32) -> u32 {
    encode_i_type(0b0010011, rd, 0b001, rs1, shamt as i32)
}

/// SRL: Shift Right Logical
pub fn srl(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b101, rs1, rs2, 0b0000000)
}

/// SRLI: Shift Right Logical Immediate
pub fn srli(rd: u32, rs1: u32, shamt: u32) -> u32 {
    encode_i_type(0b0010011, rd, 0b101, rs1, shamt as i32)
}

/// SRA: Shift Right Arithmetic
pub fn sra(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b101, rs1, rs2, 0b0100000)
}

/// SRAI: Shift Right Arithmetic Immediate
pub fn srai(rd: u32, rs1: u32, shamt: u32) -> u32 {
    encode_i_type(0b0010011, rd, 0b101, rs1, (shamt as i32) | 0x400)
}

// Comparison Instructions

/// SLT: Set Less Than
pub fn slt(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b010, rs1, rs2, 0b0000000)
}

/// SLTI: Set Less Than Immediate
pub fn slti(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b010, rs1, imm)
}

/// SLTU: Set Less Than Unsigned
pub fn sltu(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b011, rs1, rs2, 0b0000000)
}

/// SLTIU: Set Less Than Immediate Unsigned
pub fn sltiu(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b011, rs1, imm)
}

// Upper Immediate Instructions

/// LUI: Load Upper Immediate
pub fn lui(rd: u32, imm: u32) -> u32 {
    encode_u_type(0b0110111, rd, imm)
}

/// AUIPC: Add Upper Immediate to PC
pub fn auipc(rd: u32, imm: u32) -> u32 {
    encode_u_type(0b0010111, rd, imm)
}

// Branch Instructions

/// BEQ: Branch if Equal
pub fn beq(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b000, rs1, rs2, imm)
}

/// BNE: Branch if Not Equal
pub fn bne(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b001, rs1, rs2, imm)
}

/// BLT: Branch if Less Than
pub fn blt(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b100, rs1, rs2, imm)
}

/// BGE: Branch if Greater or Equal
pub fn bge(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b101, rs1, rs2, imm)
}

/// BLTU: Branch if Less Than Unsigned
pub fn bltu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b110, rs1, rs2, imm)
}

/// BGEU: Branch if Greater or Equal Unsigned
pub fn bgeu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b111, rs1, rs2, imm)
}

// Jump Instructions

/// JAL: Jump and Link
pub fn jal(rd: u32, imm: i32) -> u32 {
    let imm_u = imm as u32;
    let imm_20 = (imm_u >> 20) & 0x1;
    let imm_10_1 = (imm_u >> 1) & 0x3FF;
    let imm_11 = (imm_u >> 11) & 0x1;
    let imm_19_12 = (imm_u >> 12) & 0xFF;
    (imm_20 << 31) | (imm_19_12 << 12) | (imm_11 << 20) | (imm_10_1 << 21) | (rd << 7) | 0b1101111
}

/// JALR: Jump and Link Register
pub fn jalr(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b1100111, rd, 0b000, rs1, imm)
}

// Load Instructions

/// LW: Load Word
pub fn lw(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b010, rs1, imm)
}

/// LH: Load Halfword
pub fn lh(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b001, rs1, imm)
}

/// LB: Load Byte
pub fn lb(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b000, rs1, imm)
}

/// LHU: Load Halfword Unsigned
pub fn lhu(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b101, rs1, imm)
}

/// LBU: Load Byte Unsigned
pub fn lbu(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b100, rs1, imm)
}

// Store Instructions

/// SW: Store Word
pub fn sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_s_type(0b0100011, 0b010, rs1, rs2, imm)
}

/// SH: Store Halfword
pub fn sh(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_s_type(0b0100011, 0b001, rs1, rs2, imm)
}

/// SB: Store Byte
pub fn sb(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_s_type(0b0100011, 0b000, rs1, rs2, imm)
}

// Memory Ordering Instructions

/// FENCE: Fence Memory and I/O
pub fn fence() -> u32 {
    // FENCE: opcode=0001111, rd=0, funct3=0, rs1=0, fm=0, pred=0b1111, succ=0b1111
    0b0000_1111_1111_0000_0000_0000_0000_1111
}

// System Instructions

/// ECALL: Environment Call
pub fn ecall() -> u32 {
    encode_i_type(0b1110011, 0, 0b000, 0, 0)
}

/// EBREAK: Environment Break
pub fn ebreak() -> u32 {
    encode_i_type(0b1110011, 0, 0b000, 0, 1)
}

// ============================================================================
// Zicsr Extension - CSR Instructions
// ============================================================================

/// CSRRW: CSR Read/Write
pub fn csrrw(rd: u32, rs1: u32, csr: u32) -> u32 {
    encode_i_type(0b1110011, rd, 0b001, rs1, csr as i32)
}

/// CSRRS: CSR Read and Set
pub fn csrrs(rd: u32, rs1: u32, csr: u32) -> u32 {
    encode_i_type(0b1110011, rd, 0b010, rs1, csr as i32)
}

/// CSRRC: CSR Read and Clear
pub fn csrrc(rd: u32, rs1: u32, csr: u32) -> u32 {
    encode_i_type(0b1110011, rd, 0b011, rs1, csr as i32)
}

/// CSRRWI: CSR Read/Write Immediate
pub fn csrrwi(rd: u32, imm: u32, csr: u32) -> u32 {
    // For immediate CSR instructions, rs1 field holds the immediate value (zimm)
    encode_i_type(0b1110011, rd, 0b101, imm, csr as i32)
}

/// CSRRSI: CSR Read and Set Immediate
pub fn csrrsi(rd: u32, imm: u32, csr: u32) -> u32 {
    encode_i_type(0b1110011, rd, 0b110, imm, csr as i32)
}

/// CSRRCI: CSR Read and Clear Immediate
pub fn csrrci(rd: u32, imm: u32, csr: u32) -> u32 {
    encode_i_type(0b1110011, rd, 0b111, imm, csr as i32)
}

// ============================================================================
// M Extension - Multiply/Divide Instructions
// ============================================================================

/// MUL: Multiply (lower 32 bits)
pub fn mul(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b000, rs1, rs2, 0b0000001)
}

/// MULH: Multiply High Signed×Signed
pub fn mulh(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b001, rs1, rs2, 0b0000001)
}

/// MULHSU: Multiply High Signed×Unsigned
pub fn mulhsu(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b010, rs1, rs2, 0b0000001)
}

/// MULHU: Multiply High Unsigned×Unsigned
pub fn mulhu(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b011, rs1, rs2, 0b0000001)
}

/// DIV: Divide Signed
pub fn div(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b100, rs1, rs2, 0b0000001)
}

/// DIVU: Divide Unsigned
pub fn divu(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b101, rs1, rs2, 0b0000001)
}

/// REM: Remainder Signed
pub fn rem(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b110, rs1, rs2, 0b0000001)
}

/// REMU: Remainder Unsigned
pub fn remu(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b111, rs1, rs2, 0b0000001)
}

// ============================================================================
// Compressed (RV32C) Instruction Encoders
// These helpers construct 16-bit compressed instructions used by tests.
// Implementations mirror the decompressor bit mappings in `rtl/decompress.sv`.

/// C.ADDI4SPN: addi rd', x2, nzuimm
pub fn c_addi4spn(rd: u32, rs1: u32, nzuimm: u32) -> u16 {
    // Allow encoding of zero/illegal nzuimm for tests; caller may pass illegal values intentionally
    assert!(rs1 == 2, "C.ADDI4SPN uses sp (x2)");
    assert!((8..=15).contains(&rd), "rd must be x8-x15");
    assert!(
        nzuimm.is_multiple_of(4) && nzuimm <= 1020,
        "nzuimm must be multiple of 4 and <=1020"
    );

    let n = nzuimm;
    let mut insn: u16 = 0;
    insn |= (((n >> 6) & 0xF) as u16) << 7; // insn[10:7]
    insn |= (((n >> 4) & 0x3) as u16) << 11; // insn[12:11]
    insn |= (((n >> 3) & 0x1) as u16) << 5; // insn[5]
    insn |= (((n >> 2) & 0x1) as u16) << 6; // insn[6]
    insn |= (((rd - 8) & 0x7) as u16) << 2; // insn[4:2]=rd'
                                            // funct3 = 000 (bits 15:13), opcode = 00 (bits 1:0)
    insn
}

/// C.LW: lw rd', offset(rs1') where rd', rs1' are x8-x15
pub fn c_lw(rd: u32, rs1: u32, offset: u32) -> u16 {
    assert!(
        (8..=15).contains(&rd) && (8..=15).contains(&rs1),
        "rd/rs1 must be x8-x15"
    );
    assert!(
        offset.is_multiple_of(4) && offset <= 124,
        "offset must be multiple of 4 and <=124"
    );

    let u = offset >> 2; // uimm bits [6:2]
    let mut insn: u16 = 0;
    // Per RISC-V C extension spec:
    // insn[5] = uimm[6], insn[6] = uimm[2], insn[12:10] = uimm[5:3]
    insn |= (((u >> 4) & 0x1) as u16) << 5; // insn[5] = uimm[6]
    insn |= ((u & 0x1) as u16) << 6; // insn[6] = uimm[2]
    insn |= (((u >> 1) & 0x7) as u16) << 10; // insn[12:10] = uimm[5:3]
    insn |= (((rs1 - 8) & 0x7) as u16) << 7; // insn[9:7] = rs1'
    insn |= (((rd - 8) & 0x7) as u16) << 2; // insn[4:2] = rd'
    insn |= (0b010u16) << 13; // funct3 = 010
    insn
}

/// C.SW: sw rs2', offset(rs1') where rs* are x8-x15
pub fn c_sw(rs1: u32, rs2: u32, offset: u32) -> u16 {
    assert!(
        (8..=15).contains(&rs1) && (8..=15).contains(&rs2),
        "rs1/rs2 must be x8-x15"
    );
    assert!(
        offset.is_multiple_of(4) && offset <= 124,
        "offset must be multiple of 4 and <=124"
    );

    let u = offset >> 2; // uimm bits [6:2]
    let mut insn: u16 = 0;
    // Per RISC-V C extension spec:
    // insn[5] = uimm[6], insn[6] = uimm[2], insn[12:10] = uimm[5:3]
    insn |= (((u >> 4) & 0x1) as u16) << 5; // insn[5] = uimm[6]
    insn |= ((u & 0x1) as u16) << 6; // insn[6] = uimm[2]
    insn |= (((u >> 1) & 0x7) as u16) << 10; // insn[12:10] = uimm[5:3]
    insn |= (((rs1 - 8) & 0x7) as u16) << 7; // insn[9:7] = rs1'
    insn |= (((rs2 - 8) & 0x7) as u16) << 2; // insn[4:2] = rs2'
    insn |= (0b110u16) << 13; // funct3 = 110
    insn
}

/// C.NOP (ADDI x0, x0, 0)
pub fn c_nop() -> u16 {
    0b0000_0000_0000_0001u16
}

/// C.ADDI: addi rd, rd, imm (rd in [0..31])
pub fn c_addi(rd: u32, imm: i32) -> u16 {
    let imm6 = imm & 0x3F; // 6-bit immediate
    let mut insn: u16 = 0;
    insn |= (((imm6 >> 5) & 0x1) as u16) << 12; // insn[12]
    insn |= ((imm6 & 0x1F) as u16) << 2; // insn[6:2]
    insn |= ((rd & 0x1F) as u16) << 7; // insn[11:7] = rd
    insn |= (0b000u16) << 13; // funct3 = 000
    insn |= 0b01u16; // opcode 01
    insn
}

/// C.JAL: jal x1, offset (signed, multiple of 2)
pub fn c_jal(offset: i32) -> u16 {
    // offset is signed 12-bit (multiple of 2). Map raw offset bits per spec.
    let imm_u = (offset as u32) & 0xFFF; // 12 bits (offset includes low 1'b0)
    let mut insn: u16 = 0;
    insn |= (((imm_u >> 11) & 0x1) as u16) << 12; // insn[12] = offset[11]
    insn |= (((imm_u >> 10) & 0x1) as u16) << 8; // insn[8] = offset[10]
    insn |= (((imm_u >> 8) & 0x3) as u16) << 9; // insn[10:9] = offset[9:8]
    insn |= (((imm_u >> 7) & 0x1) as u16) << 6; // insn[6] = offset[7]
    insn |= (((imm_u >> 6) & 0x1) as u16) << 7; // insn[7] = offset[6]
    insn |= (((imm_u >> 5) & 0x1) as u16) << 2; // insn[2] = offset[5]
    insn |= (((imm_u >> 4) & 0x1) as u16) << 11; // insn[11] = offset[4]
    insn |= (((imm_u >> 1) & 0x7) as u16) << 3; // insn[5:3] = offset[3:1]
    insn |= 0b01u16; // opcode
    insn |= (0b001u16) << 13; // funct3 = 001
    insn
}

/// C.LI: addi rd, x0, imm (6-bit signed)
pub fn c_li(rd: u32, imm: i32) -> u16 {
    let imm6 = imm & 0x3F;
    let mut insn: u16 = 0;
    insn |= (((imm6 >> 5) & 0x1) as u16) << 12; // insn[12]
    insn |= ((imm6 & 0x1F) as u16) << 2; // insn[6:2]
    insn |= ((rd & 0x1F) as u16) << 7; // insn[11:7]
    insn |= 0b01u16; // opcode
    insn |= (0b010u16) << 13; // funct3 = 010
    insn
}

/// C.ADDI16SP: addi x2, x2, nzimm (multiple of 16)
pub fn c_addi16sp(nzimm: u32) -> u16 {
    // Allow zero encoding (illegal) for tests; caller may pass zero intentionally
    // 6-bit field after removing 4 LSBs: max value is 63 * 16 = 1008
    assert!(
        nzimm.is_multiple_of(16) && nzimm <= 1008,
        "nzimm must be multiple of 16 and <=1008"
    );
    let nz = nzimm >> 4; // remove low 4 zeros
    let mut insn: u16 = 0;
    insn |= (((nz >> 5) & 0x1) as u16) << 12; // insn[12]
    insn |= (((nz >> 3) & 0x3) as u16) << 3; // insn[4:3]
    insn |= (((nz >> 2) & 0x1) as u16) << 5; // insn[5]
    insn |= (((nz >> 1) & 0x1) as u16) << 2; // insn[2]
    insn |= ((nz & 0x1) as u16) << 6; // insn[6]
    insn |= (2u16 & 0x1F) << 7; // rd_rs1 == 2
    insn |= 0b01u16; // opcode
    insn |= (0b011u16) << 13; // funct3 = 011
    insn
}

/// C.LUI: LUI rd, nzimm (compressed immediate field passed as small integer)
pub fn c_lui(rd: u32, nzimm_field: u32) -> u16 {
    // nzimm_field encodes {insn[12], insn[6:2]} (6 bits)
    assert!(nzimm_field <= 0x3F, "nzimm_field must fit in 6 bits");
    let mut insn: u16 = 0;
    insn |= (((nzimm_field >> 5) & 0x1) as u16) << 12; // insn[12]
    insn |= ((nzimm_field & 0x1F) as u16) << 2; // insn[6:2]
    insn |= ((rd & 0x1F) as u16) << 7; // insn[11:7]
    insn |= 0b01u16; // opcode
    insn |= (0b011u16) << 13; // funct3 = 011
    insn
}

/// C.SRLI: srli rd', rd', shamt (rd' encoded in rs1_full)
pub fn c_srli(rd: u32, shamt: u32) -> u16 {
    assert!((8..=15).contains(&rd));
    assert!(shamt != 0 && shamt <= 31);
    let mut insn: u16 = 0;
    insn |= ((shamt & 0x1F) as u16) << 2; // insn[6:2]
    insn |= (((rd - 8) & 0x7) as u16) << 7; // insn[11:7]
    insn |= (0b100u16) << 13; // funct3 = 100
    insn |= (0b00u16) << 10; // funct2 = 00 at [11:10]
    insn |= 0b01u16; // opcode
    insn
}

/// C.SRAI: srai rd', rd', shamt
pub fn c_srai(rd: u32, shamt: u32) -> u16 {
    assert!((8..=15).contains(&rd));
    assert!(shamt != 0 && shamt <= 31);
    let mut insn: u16 = 0;
    insn |= ((shamt & 0x1F) as u16) << 2; // insn[6:2]
    insn |= (((rd - 8) & 0x7) as u16) << 7; // insn[11:7]
    insn |= (0b100u16) << 13; // funct3 = 100
    insn |= (0b01u16) << 10; // funct2 = 01
    insn |= 0b01u16; // opcode
    insn
}

/// C.ANDI: andi rd', rd', imm
pub fn c_andi(rd: u32, imm: i32) -> u16 {
    assert!((8..=15).contains(&rd));
    let imm6 = imm & 0x3F;
    let mut insn: u16 = 0;
    insn |= (((imm6 >> 5) & 0x1) as u16) << 12; // insn[12]
    insn |= ((imm6 & 0x1F) as u16) << 2; // insn[6:2]
    insn |= (((rd - 8) & 0x7) as u16) << 7; // insn[11:7]
    insn |= (0b100u16) << 13; // funct3 = 100
    insn |= (0b10u16) << 10; // funct2 = 10
    insn |= 0b01u16; // opcode
    insn
}

/// C.SUB / C.XOR / C.OR / C.AND family (funct2_ca selects which)
pub fn c_sub(rd: u32, rs2: u32) -> u16 {
    c_rtype_family(rd, rs2, 0)
}
pub fn c_xor(rd: u32, rs2: u32) -> u16 {
    c_rtype_family(rd, rs2, 1)
}
pub fn c_or(rd: u32, rs2: u32) -> u16 {
    c_rtype_family(rd, rs2, 2)
}
pub fn c_and(rd: u32, rs2: u32) -> u16 {
    c_rtype_family(rd, rs2, 3)
}

fn c_rtype_family(rd: u32, rs2: u32, funct2_ca: u32) -> u16 {
    assert!((8..=15).contains(&rd) && (8..=15).contains(&rs2));
    let mut insn: u16 = 0;
    insn |= (((rs2 - 8) & 0x7) as u16) << 2; // rs2 in insn[4:2] or rs2_full in  (use compressed rs2 in 4:2)
    insn |= (((rd - 8) & 0x7) as u16) << 7; // rd in insn[11:7]
    insn |= (0b100u16) << 13; // funct3
    insn |= (0b11u16) << 10; // funct2 = 11 for this group
                             // set funct2_ca in bits[6:5]
    insn |= (funct2_ca as u16 & 0x3) << 5;
    insn |= 0b01u16; // opcode
    insn
}

/// C.J: jal x0, offset
pub fn c_j(offset: i32) -> u16 {
    // same bit layout as C.JAL but funct3 = 101; use raw offset bits per spec
    let imm_u = (offset as u32) & 0xFFF;
    let mut insn: u16 = 0;
    insn |= (((imm_u >> 11) & 0x1) as u16) << 12;
    insn |= (((imm_u >> 10) & 0x1) as u16) << 8;
    insn |= (((imm_u >> 8) & 0x3) as u16) << 9;
    insn |= (((imm_u >> 7) & 0x1) as u16) << 6;
    insn |= (((imm_u >> 6) & 0x1) as u16) << 7;
    insn |= (((imm_u >> 5) & 0x1) as u16) << 2;
    insn |= (((imm_u >> 4) & 0x1) as u16) << 11;
    insn |= (((imm_u >> 1) & 0x7) as u16) << 3;
    insn |= 0b01u16;
    insn |= (0b101u16) << 13; // funct3 = 101
    insn
}

/// C.BEQZ: beq rs1', x0, offset
pub fn c_beqz(rs1: u32, offset: i32) -> u16 {
    assert!((8..=15).contains(&rs1));
    let imm_u = (offset as u32) & 0x1FF; // 9 bits (offset includes low 1'b0)
    let mut insn: u16 = 0;
    // mapping per decompressor: imm_b = {insn[12], insn[6:5], insn[2], insn[11:10], insn[4:3], 1'b0}
    insn |= (((imm_u >> 8) & 0x1) as u16) << 12; // insn[12] = offset[8]
    insn |= (((imm_u >> 7) & 0x1) as u16) << 6; // insn[6] = offset[7]
    insn |= (((imm_u >> 6) & 0x1) as u16) << 5; // insn[5] = offset[6]
    insn |= (((imm_u >> 5) & 0x1) as u16) << 2; // insn[2] = offset[5]
    insn |= (((imm_u >> 3) & 0x3) as u16) << 10; // insn[11:10] = offset[4:3]
    insn |= (((imm_u >> 1) & 0x3) as u16) << 3; // insn[4:3] = offset[2:1]
    insn |= (((rs1 - 8) & 0x7) as u16) << 7; // rs1'
    insn |= 0b01u16;
    insn |= (0b110u16) << 13; // funct3 = 110
    insn
}

/// C.BNEZ: bne rs1', x0, offset
pub fn c_bnez(rs1: u32, offset: i32) -> u16 {
    assert!((8..=15).contains(&rs1));
    let imm_u = (offset as u32) & 0x1FF; // 9 bits
    let mut insn: u16 = 0;
    // mapping per decompressor: imm_b = {insn[12], insn[6:5], insn[2], insn[11:10], insn[4:3], 1'b0}
    insn |= (((imm_u >> 8) & 0x1) as u16) << 12; // insn[12]
    insn |= (((imm_u >> 7) & 0x1) as u16) << 6; // insn[6]
    insn |= (((imm_u >> 6) & 0x1) as u16) << 5; // insn[5]
    insn |= (((imm_u >> 5) & 0x1) as u16) << 2; // insn[2]
    insn |= (((imm_u >> 3) & 0x3) as u16) << 10; // insn[11:10]
    insn |= (((imm_u >> 1) & 0x3) as u16) << 3; // insn[4:3]
    insn |= (((rs1 - 8) & 0x7) as u16) << 7; // rs1'
    insn |= 0b01u16;
    insn |= (0b111u16) << 13; // funct3 = 111
    insn
}

/// Quadrant 2 encoders
/// C.SLLI: slli rd, rd, shamt
pub fn c_slli(rd: u32, shamt: u32) -> u16 {
    // Allow illegal encodings (e.g., shamt=0) for test coverage
    assert!(rd < 32);
    assert!(shamt <= 63);
    let mut insn: u16 = 0;
    insn |= ((shamt & 0x1F) as u16) << 2; // insn[6:2]
    insn |= ((rd & 0x1F) as u16) << 7; // insn[11:7]
    insn |= 0b10u16; // opcode = 10
    insn |= (0b000u16) << 13; // funct3 = 000
    insn
}

/// C.LWSP: lw rd, offset(x2) (rd != 0)
pub fn c_lwsp(rd: u32, offset: u32) -> u16 {
    // Allow rd==0 (illegal) for tests; offset must be multiple of 4
    assert!(offset.is_multiple_of(4) && offset <= 255);
    let u = offset >> 2; // uimm bits
                         // mapping per decompressor: uimm_lwsp = {insn[3:2], insn[12], insn[6:4], 2'b00}
                         // Reverse mapping from offset -> scattered fields:
                         // insn[6:4] = u[3:1], insn[12] = u[4], insn[3:2] = u[6:5]
    let mut insn: u16 = 0;
    insn |= ((u & 0x7) as u16) << 4; // insn[6:4] = u[3:1]
    insn |= (((u >> 3) & 0x1) as u16) << 12; // insn[12] = u[4]
    insn |= (((u >> 4) & 0x3) as u16) << 2; // insn[3:2] = u[6:5]
    insn |= ((rd & 0x1F) as u16) << 7; // rd
    insn |= 0b10u16; // opcode
    insn |= (0b010u16) << 13; // funct3 = 010
    insn
}

/// C.JR: jalr x0, 0(rs1)
pub fn c_jr(rs1: u32) -> u16 {
    // Allow rs1==0 (illegal) for tests
    assert!(rs1 < 32);
    let mut insn: u16 = 0;
    insn |= ((rs1 & 0x1F) as u16) << 7; // rd_rs1 in [11:7]
    insn |= 0b10u16; // opcode
    insn |= (0b100u16) << 13; // funct3 = 100
                              // ensure rs2 == 0 -> bits [6:2] = 0
    insn
}

/// C.MV: add rd, x0, rs2
pub fn c_mv(rd: u32, rs2: u32) -> u16 {
    // Allow illegal rd==0 case for tests
    let mut insn: u16 = 0;
    insn |= ((rs2 & 0x1F) as u16) << 2; // rs2 in [6:2]
    insn |= ((rd & 0x1F) as u16) << 7; // rd
    insn |= 0b10u16; // opcode
    insn |= (0b100u16) << 13; // funct3 = 100, insn[12]=0 indicates MV
    insn
}

/// C.EBREAK
pub fn c_ebreak() -> u16 {
    0b1001_0000_0000_0010u16
}

/// C.JALR: jalr x1, 0(rs1)
pub fn c_jalr(rs1: u32) -> u16 {
    assert!(rs1 != 0);
    let mut insn: u16 = 0;
    insn |= ((rs1 & 0x1F) as u16) << 7; // rd_rs1
    insn |= 0b10u16; // opcode
    insn |= (0b100u16) << 13; // funct3 = 100
                              // set insn[12] = 1 to select JALR/EBREAK group with insn[12]=1
    insn |= 1u16 << 12;
    insn
}

/// C.ADD: add rd, rd, rs2
pub fn c_add(rd: u32, rs2: u32) -> u16 {
    assert!(rd != 0 && rs2 != 0);
    let mut insn: u16 = 0;
    insn |= ((rs2 & 0x1F) as u16) << 2;
    insn |= ((rd & 0x1F) as u16) << 7;
    insn |= 0b10u16;
    insn |= (0b100u16) << 13;
    insn |= 1u16 << 12; // indicate add path
    insn
}

/// C.SWSP: sw rs2, offset(x2)
pub fn c_swsp(rs2: u32, offset: u32) -> u16 {
    assert!(offset.is_multiple_of(4) && offset <= 1020);
    let u = offset >> 2; // uimm bits
                         // mapping: uimm_swsp = {insn[8:7], insn[12:9], 2'b00}
    let mut insn: u16 = 0;
    // insn[8:7] <- u[7:6]
    insn |= (((u >> 4) & 0x3) as u16) << 7; // insn[8:7]
                                            // insn[12:9] <- u[5:2]
    insn |= ((u & 0xF) as u16) << 9; // insn[12:9]
    insn |= ((rs2 & 0x1F) as u16) << 2; // rs2
    insn |= 0b10u16;
    insn |= (0b110u16) << 13; // funct3 = 110
    insn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_addi() {
        // ADDI x1, x0, 5
        let instr = addi(1, 0, 5);
        assert_eq!(instr, 0x00500093);
    }

    #[test]
    fn test_encode_add() {
        // ADD x3, x1, x2
        let instr = add(3, 1, 2);
        assert_eq!(instr, 0x002081B3);
    }

    #[test]
    fn test_encode_beq() {
        // BEQ x1, x2, 8
        let instr = beq(1, 2, 8);
        assert_eq!(instr & 0x7F, 0b1100011); // Check opcode
    }

    #[test]
    fn test_encode_sw() {
        // SW x2, 0(x1)
        let instr = sw(1, 2, 0);
        // Verify opcode and basic structure
        assert_eq!(instr & 0x7F, 0b0100011); // Check opcode
        assert_eq!((instr >> 7) & 0x1F, 0); // Check imm[4:0]
        assert_eq!((instr >> 12) & 0x7, 0b010); // Check funct3 (SW)
        assert_eq!((instr >> 15) & 0x1F, 1); // Check rs1
        assert_eq!((instr >> 20) & 0x1F, 2); // Check rs2
    }

    #[test]
    fn test_encode_lui() {
        // LUI x1, 0x12345
        let instr = lui(1, 0x12345000);
        assert_eq!(instr & 0x7F, 0b0110111); // Check opcode
        assert_eq!((instr >> 7) & 0x1F, 1); // Check rd
    }
}

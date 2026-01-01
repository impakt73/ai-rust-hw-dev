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

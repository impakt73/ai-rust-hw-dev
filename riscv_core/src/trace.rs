/// RISC-V Instruction Trace Structures
///
/// Provides structured representation of executed instructions for debugging and analysis

use std::fmt;

/// Type of RISC-V instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionType {
    // R-type (register-register operations)
    Add,
    Sub,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Sra,
    Or,
    And,

    // I-type (immediate operations)
    Addi,
    Slti,
    Sltiu,
    Xori,
    Ori,
    Andi,
    Slli,
    Srli,
    Srai,

    // Load instructions
    Lw,
    Lh,
    Lb,
    Lhu,
    Lbu,

    // Store instructions
    Sw,
    Sh,
    Sb,

    // Branch instructions
    Beq,
    Bne,
    Blt,
    Bge,
    Bltu,
    Bgeu,

    // Upper immediate
    Lui,
    Auipc,

    // Jump instructions
    Jal,
    Jalr,

    // Unknown instruction
    Unknown,
}

/// Operand for an instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// Register operand with register number and value
    Register { reg: u8, value: u32 },
    /// Immediate value (signed)
    Immediate(i32),
}

/// Complete trace information for a single instruction
#[derive(Debug, Clone)]
pub struct InstructionTrace {
    /// Program counter
    pub pc: u32,
    /// Raw instruction word
    pub instruction: u32,
    /// Type of instruction
    pub inst_type: InstructionType,
    /// Destination register (if applicable)
    pub rd: Option<Operand>,
    /// Source register 1 (if applicable)
    pub rs1: Option<Operand>,
    /// Source register 2 (if applicable)
    pub rs2: Option<Operand>,
    /// Immediate value (if applicable)
    pub immediate: Option<i32>,
}

impl InstructionTrace {
    /// Create a new instruction trace from a raw instruction and register values
    pub fn from_instruction(
        pc: u32,
        instruction: u32,
        rs1_value: u32,
        rs2_value: u32,
        rd_value: u32,
    ) -> Self {
        let opcode = instruction & 0x7F;
        let rd_num = ((instruction >> 7) & 0x1F) as u8;
        let funct3 = ((instruction >> 12) & 0x7) as u8;
        let rs1_num = ((instruction >> 15) & 0x1F) as u8;
        let rs2_num = ((instruction >> 20) & 0x1F) as u8;
        let funct7 = ((instruction >> 25) & 0x7F) as u8;

        match opcode {
            0b0110011 => {
                // R-type
                let inst_type = match (funct3, funct7) {
                    (0b000, 0b0000000) => InstructionType::Add,
                    (0b000, 0b0100000) => InstructionType::Sub,
                    (0b001, 0b0000000) => InstructionType::Sll,
                    (0b010, 0b0000000) => InstructionType::Slt,
                    (0b011, 0b0000000) => InstructionType::Sltu,
                    (0b100, 0b0000000) => InstructionType::Xor,
                    (0b101, 0b0000000) => InstructionType::Srl,
                    (0b101, 0b0100000) => InstructionType::Sra,
                    (0b110, 0b0000000) => InstructionType::Or,
                    (0b111, 0b0000000) => InstructionType::And,
                    _ => InstructionType::Unknown,
                };
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: Some(Operand::Register {
                        reg: rs2_num,
                        value: rs2_value,
                    }),
                    immediate: None,
                }
            }
            0b0010011 => {
                // I-type ALU
                let imm = get_imm_i(instruction) as i32;
                let shamt = ((instruction >> 20) & 0x1F) as i32;
                let funct7 = (instruction >> 25) & 0x7F;

                let (inst_type, imm_val) = match funct3 {
                    0b000 => (InstructionType::Addi, imm),
                    0b010 => (InstructionType::Slti, imm),
                    0b011 => (InstructionType::Sltiu, imm),
                    0b100 => (InstructionType::Xori, imm),
                    0b110 => (InstructionType::Ori, imm),
                    0b111 => (InstructionType::Andi, imm),
                    0b001 => (InstructionType::Slli, shamt),
                    0b101 => {
                        if funct7 == 0b0000000 {
                            (InstructionType::Srli, shamt)
                        } else {
                            (InstructionType::Srai, shamt)
                        }
                    }
                    _ => (InstructionType::Unknown, imm),
                };

                InstructionTrace {
                    pc,
                    instruction,
                    inst_type,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: None,
                    immediate: Some(imm_val),
                }
            }
            0b0000011 => {
                // Load
                let imm = get_imm_i(instruction) as i32;
                let inst_type = match funct3 {
                    0b010 => InstructionType::Lw,
                    0b000 => InstructionType::Lb,
                    0b001 => InstructionType::Lh,
                    0b100 => InstructionType::Lbu,
                    0b101 => InstructionType::Lhu,
                    _ => InstructionType::Unknown,
                };
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: None,
                    immediate: Some(imm),
                }
            }
            0b0100011 => {
                // Store
                let imm = get_imm_s(instruction) as i32;
                let inst_type = match funct3 {
                    0b010 => InstructionType::Sw,
                    0b000 => InstructionType::Sb,
                    0b001 => InstructionType::Sh,
                    _ => InstructionType::Unknown,
                };
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type,
                    rd: None,
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: Some(Operand::Register {
                        reg: rs2_num,
                        value: rs2_value,
                    }),
                    immediate: Some(imm),
                }
            }
            0b1100011 => {
                // Branch
                let imm = get_imm_b(instruction) as i32;
                let inst_type = match funct3 {
                    0b000 => InstructionType::Beq,
                    0b001 => InstructionType::Bne,
                    0b100 => InstructionType::Blt,
                    0b101 => InstructionType::Bge,
                    0b110 => InstructionType::Bltu,
                    0b111 => InstructionType::Bgeu,
                    _ => InstructionType::Unknown,
                };
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type,
                    rd: None,
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: Some(Operand::Register {
                        reg: rs2_num,
                        value: rs2_value,
                    }),
                    immediate: Some(imm),
                }
            }
            0b0110111 => {
                // LUI
                let imm = (get_imm_u(instruction) >> 12) as i32;
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type: InstructionType::Lui,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: None,
                    rs2: None,
                    immediate: Some(imm),
                }
            }
            0b0010111 => {
                // AUIPC
                let imm = (get_imm_u(instruction) >> 12) as i32;
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type: InstructionType::Auipc,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: None,
                    rs2: None,
                    immediate: Some(imm),
                }
            }
            0b1101111 => {
                // JAL
                let imm = get_imm_j(instruction) as i32;
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type: InstructionType::Jal,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: None,
                    rs2: None,
                    immediate: Some(imm),
                }
            }
            0b1100111 => {
                // JALR
                let imm = get_imm_i(instruction) as i32;
                InstructionTrace {
                    pc,
                    instruction,
                    inst_type: InstructionType::Jalr,
                    rd: Some(Operand::Register {
                        reg: rd_num,
                        value: rd_value,
                    }),
                    rs1: Some(Operand::Register {
                        reg: rs1_num,
                        value: rs1_value,
                    }),
                    rs2: None,
                    immediate: Some(imm),
                }
            }
            _ => InstructionTrace {
                pc,
                instruction,
                inst_type: InstructionType::Unknown,
                rd: None,
                rs1: None,
                rs2: None,
                immediate: None,
            },
        }
    }
}

/// Display implementation for backward compatibility with text trace
impl fmt::Display for InstructionTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the existing disasm module for consistent formatting
        let disassembled = crate::disasm::disassemble_with_all_values(
            self.instruction,
            self.rs1.map_or(0, |op| match op {
                Operand::Register { value, .. } => value,
                _ => 0,
            }),
            self.rs2.map_or(0, |op| match op {
                Operand::Register { value, .. } => value,
                _ => 0,
            }),
            self.rd.map_or(0, |op| match op {
                Operand::Register { value, .. } => value,
                _ => 0,
            }),
        );
        write!(f, "{}", disassembled)
    }
}

/// Extract I-type immediate (sign-extended)
fn get_imm_i(instruction: u32) -> u32 {
    let imm = (instruction >> 20) & 0xFFF;
    // Sign extend from bit 11
    if imm & 0x800 != 0 {
        imm | 0xFFFFF000
    } else {
        imm
    }
}

/// Extract S-type immediate (sign-extended)
fn get_imm_s(instruction: u32) -> u32 {
    let imm_11_5 = (instruction >> 25) & 0x7F;
    let imm_4_0 = (instruction >> 7) & 0x1F;
    let imm = (imm_11_5 << 5) | imm_4_0;
    // Sign extend from bit 11
    if imm & 0x800 != 0 {
        imm | 0xFFFFF000
    } else {
        imm
    }
}

/// Extract B-type immediate (sign-extended, multiplied by 2)
fn get_imm_b(instruction: u32) -> u32 {
    let imm_12 = (instruction >> 31) & 0x1;
    let imm_10_5 = (instruction >> 25) & 0x3F;
    let imm_4_1 = (instruction >> 8) & 0xF;
    let imm_11 = (instruction >> 7) & 0x1;
    let imm = (imm_12 << 12) | (imm_11 << 11) | (imm_10_5 << 5) | (imm_4_1 << 1);
    // Sign extend from bit 12
    if imm & 0x1000 != 0 {
        imm | 0xFFFFE000
    } else {
        imm
    }
}

/// Extract U-type immediate
fn get_imm_u(instruction: u32) -> u32 {
    instruction & 0xFFFFF000
}

/// Extract J-type immediate (sign-extended, multiplied by 2)
fn get_imm_j(instruction: u32) -> u32 {
    let imm_20 = (instruction >> 31) & 0x1;
    let imm_10_1 = (instruction >> 21) & 0x3FF;
    let imm_11 = (instruction >> 20) & 0x1;
    let imm_19_12 = (instruction >> 12) & 0xFF;
    let imm = (imm_20 << 20) | (imm_19_12 << 12) | (imm_11 << 11) | (imm_10_1 << 1);
    // Sign extend from bit 20
    if imm & 0x100000 != 0 {
        imm | 0xFFE00000
    } else {
        imm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_add() {
        // add x1, x2, x3 with values: x2=10, x3=5, result=15
        let instruction = 0x003100B3;
        let trace = InstructionTrace::from_instruction(0x1000, instruction, 10, 5, 15);
        assert_eq!(trace.inst_type, InstructionType::Add);
        assert_eq!(trace.pc, 0x1000);
        assert!(matches!(trace.rd, Some(Operand::Register { reg: 1, value: 15 })));
        assert!(matches!(trace.rs1, Some(Operand::Register { reg: 2, value: 10 })));
        assert!(matches!(trace.rs2, Some(Operand::Register { reg: 3, value: 5 })));
    }

    #[test]
    fn test_trace_addi() {
        // addi x1, x2, 42 with x2=10, result=52
        let instruction = 0x02A10093;
        let trace = InstructionTrace::from_instruction(0x1000, instruction, 10, 0, 52);
        assert_eq!(trace.inst_type, InstructionType::Addi);
        assert_eq!(trace.immediate, Some(42));
        assert!(matches!(trace.rd, Some(Operand::Register { reg: 1, value: 52 })));
        assert!(matches!(trace.rs1, Some(Operand::Register { reg: 2, value: 10 })));
        assert!(trace.rs2.is_none());
    }

    #[test]
    fn test_trace_lw() {
        // lw x1, 4(x2) with x2=0x1000, loaded value=0xdeadbeef
        let instruction = 0x00412083;
        let trace = InstructionTrace::from_instruction(0x1000, instruction, 0x1000, 0, 0xdeadbeef);
        assert_eq!(trace.inst_type, InstructionType::Lw);
        assert_eq!(trace.immediate, Some(4));
        assert!(matches!(trace.rd, Some(Operand::Register { reg: 1, value: 0xdeadbeef })));
        assert!(matches!(trace.rs1, Some(Operand::Register { reg: 2, value: 0x1000 })));
    }

    #[test]
    fn test_trace_sw() {
        // sw x3, 8(x2) with x2=0x2000, x3=0xcafebabe
        let instruction = 0x00312423;
        let trace = InstructionTrace::from_instruction(0x1000, instruction, 0x2000, 0xcafebabe, 0);
        assert_eq!(trace.inst_type, InstructionType::Sw);
        assert_eq!(trace.immediate, Some(8));
        assert!(trace.rd.is_none());
        assert!(matches!(trace.rs1, Some(Operand::Register { reg: 2, value: 0x2000 })));
        assert!(matches!(trace.rs2, Some(Operand::Register { reg: 3, value: 0xcafebabe })));
    }

    #[test]
    fn test_trace_display() {
        // Test that Display trait produces expected output
        let instruction = 0x003100B3; // add x1, x2, x3
        let trace = InstructionTrace::from_instruction(0x1000, instruction, 10, 5, 15);
        let display = format!("{}", trace);
        assert!(display.contains("add"));
        assert!(display.contains("x1"));
        assert!(display.contains("x2"));
        assert!(display.contains("x3"));
    }
}

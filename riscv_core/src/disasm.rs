/// RISC-V RV32I Instruction Disassembler
///
/// Decodes 32-bit RISC-V instructions into human-readable assembly format
/// Disassemble a 32-bit RISC-V instruction into a human-readable string
pub fn disassemble(instruction: u32) -> String {
    disassemble_with_all_values(instruction, 0, 0, 0)
}

/// Disassemble a 32-bit RISC-V instruction with register values (legacy, without rd_value)
pub fn disassemble_with_values(instruction: u32, rs1_value: u32, rs2_value: u32) -> String {
    disassemble_with_all_values(instruction, rs1_value, rs2_value, 0)
}

/// Disassemble a 32-bit RISC-V instruction with all register values including destination
pub fn disassemble_with_all_values(
    instruction: u32,
    rs1_value: u32,
    rs2_value: u32,
    rd_value: u32,
) -> String {
    let opcode = instruction & 0x7F;
    let rd = ((instruction >> 7) & 0x1F) as u8;
    let funct3 = ((instruction >> 12) & 0x7) as u8;
    let rs1 = ((instruction >> 15) & 0x1F) as u8;
    let rs2 = ((instruction >> 20) & 0x1F) as u8;
    let funct7 = ((instruction >> 25) & 0x7F) as u8;

    match opcode {
        0b0110011 => disassemble_r_type(
            instruction,
            rd,
            funct3,
            rs1,
            rs2,
            funct7,
            rs1_value,
            rs2_value,
            rd_value,
        ),
        0b0010011 => disassemble_i_type_alu(instruction, rd, funct3, rs1, rs1_value, rd_value),
        0b0000011 => disassemble_load(rd, funct3, rs1, get_imm_i(instruction), rs1_value, rd_value),
        0b0100011 => disassemble_store(
            funct3,
            rs1,
            rs2,
            get_imm_s(instruction),
            rs1_value,
            rs2_value,
        ),
        0b1100011 => disassemble_branch(
            funct3,
            rs1,
            rs2,
            get_imm_b(instruction),
            rs1_value,
            rs2_value,
        ),
        0b0110111 => format!(
            "lui x{}=0x{:x}, 0x{:x}",
            rd,
            rd_value,
            get_imm_u(instruction) >> 12
        ),
        0b0010111 => format!(
            "auipc x{}=0x{:x}, 0x{:x}",
            rd,
            rd_value,
            get_imm_u(instruction) >> 12
        ),
        0b1101111 => format!(
            "jal x{}=0x{:x}, {}",
            rd,
            rd_value,
            get_imm_j(instruction) as i32
        ),
        0b1100111 => {
            let imm = get_imm_i(instruction) as i32;
            format!(
                "jalr x{}=0x{:x}, {}(x{}=0x{:x})",
                rd, rd_value, imm, rs1, rs1_value
            )
        }
        0b0001111 => disassemble_fence(instruction),
        0b1110011 => disassemble_system(instruction, rd, funct3, rs1, rs1_value, rd_value),
        0b0101111 => disassemble_atomic(
            instruction,
            rd,
            funct3,
            rs1,
            rs2,
            rs1_value,
            rs2_value,
            rd_value,
        ),
        _ => format!("unknown opcode 0x{:02x}", opcode),
    }
}

/// Disassemble R-type instructions (register-register operations)
#[allow(clippy::too_many_arguments)]
fn disassemble_r_type(
    _instruction: u32,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs2: u8,
    funct7: u8,
    rs1_value: u32,
    rs2_value: u32,
    rd_value: u32,
) -> String {
    let mnemonic = match (funct3, funct7) {
        // RV32I base instructions
        (0b000, 0b0000000) => "add",
        (0b000, 0b0100000) => "sub",
        (0b001, 0b0000000) => "sll",
        (0b010, 0b0000000) => "slt",
        (0b011, 0b0000000) => "sltu",
        (0b100, 0b0000000) => "xor",
        (0b101, 0b0000000) => "srl",
        (0b101, 0b0100000) => "sra",
        (0b110, 0b0000000) => "or",
        (0b111, 0b0000000) => "and",
        // M extension instructions (funct7 = 0000001)
        (0b000, 0b0000001) => "mul",
        (0b001, 0b0000001) => "mulh",
        (0b010, 0b0000001) => "mulhsu",
        (0b011, 0b0000001) => "mulhu",
        (0b100, 0b0000001) => "div",
        (0b101, 0b0000001) => "divu",
        (0b110, 0b0000001) => "rem",
        (0b111, 0b0000001) => "remu",
        _ => return format!("unknown R-type f3={} f7={}", funct3, funct7),
    };
    format!(
        "{} x{}=0x{:x}, x{}=0x{:x}, x{}=0x{:x}",
        mnemonic, rd, rd_value, rs1, rs1_value, rs2, rs2_value
    )
}

/// Disassemble I-type ALU instructions (immediate operations)
fn disassemble_i_type_alu(
    instruction: u32,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs1_value: u32,
    rd_value: u32,
) -> String {
    let imm = get_imm_i(instruction) as i32;
    let shamt = (instruction >> 20) & 0x1F;
    let funct7 = (instruction >> 25) & 0x7F;

    let mnemonic = match funct3 {
        0b000 => "addi",
        0b010 => "slti",
        0b011 => "sltiu",
        0b100 => "xori",
        0b110 => "ori",
        0b111 => "andi",
        0b001 => "slli",
        0b101 => {
            if funct7 == 0b0000000 {
                "srli"
            } else {
                "srai"
            }
        }
        _ => return format!("unknown I-type f3={}", funct3),
    };

    // Shift instructions use shamt instead of full immediate
    if matches!(funct3, 0b001 | 0b101) {
        format!(
            "{} x{}=0x{:x}, x{}=0x{:x}, {}",
            mnemonic, rd, rd_value, rs1, rs1_value, shamt
        )
    } else {
        format!(
            "{} x{}=0x{:x}, x{}=0x{:x}, {}",
            mnemonic, rd, rd_value, rs1, rs1_value, imm
        )
    }
}

/// Disassemble load instructions
fn disassemble_load(
    rd: u8,
    funct3: u8,
    rs1: u8,
    imm: u32,
    rs1_value: u32,
    rd_value: u32,
) -> String {
    let imm_signed = imm as i32;
    let mnemonic = match funct3 {
        0b010 => "lw",
        0b000 => "lb",
        0b001 => "lh",
        0b100 => "lbu",
        0b101 => "lhu",
        _ => return format!("unknown load f3={}", funct3),
    };
    format!(
        "{} x{}=0x{:x}, {}(x{}=0x{:x})",
        mnemonic, rd, rd_value, imm_signed, rs1, rs1_value
    )
}

/// Disassemble store instructions
fn disassemble_store(
    funct3: u8,
    rs1: u8,
    rs2: u8,
    imm: u32,
    rs1_value: u32,
    rs2_value: u32,
) -> String {
    let imm_signed = imm as i32;
    let mnemonic = match funct3 {
        0b010 => "sw",
        0b000 => "sb",
        0b001 => "sh",
        _ => return format!("unknown store f3={}", funct3),
    };
    format!(
        "{} x{}=0x{:x}, {}(x{}=0x{:x})",
        mnemonic, rs2, rs2_value, imm_signed, rs1, rs1_value
    )
}

/// Disassemble branch instructions
fn disassemble_branch(
    funct3: u8,
    rs1: u8,
    rs2: u8,
    imm: u32,
    rs1_value: u32,
    rs2_value: u32,
) -> String {
    let imm_signed = imm as i32;
    let mnemonic = match funct3 {
        0b000 => "beq",
        0b001 => "bne",
        0b100 => "blt",
        0b101 => "bge",
        0b110 => "bltu",
        0b111 => "bgeu",
        _ => return format!("unknown branch f3={}", funct3),
    };
    format!(
        "{} x{}=0x{:x}, x{}=0x{:x}, {}",
        mnemonic, rs1, rs1_value, rs2, rs2_value, imm_signed
    )
}

/// Disassemble FENCE instruction
fn disassemble_fence(_instruction: u32) -> String {
    // FENCE instruction - for now just return the basic mnemonic
    // Could decode pred/succ fields if needed
    "fence".to_string()
}

/// Disassemble SYSTEM instructions (ECALL, EBREAK, CSR)
fn disassemble_system(
    instruction: u32,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs1_value: u32,
    rd_value: u32,
) -> String {
    if funct3 == 0b000 {
        // ECALL or EBREAK (distinguished by imm[0])
        let imm = get_imm_i(instruction);
        if imm & 0x1 == 0 {
            "ecall".to_string()
        } else {
            "ebreak".to_string()
        }
    } else {
        // CSR instructions
        let csr = (instruction >> 20) & 0xFFF;
        let zimm = rs1; // For immediate CSR instructions, rs1 field holds zimm

        let mnemonic = match funct3 {
            0b001 => "csrrw",
            0b010 => "csrrs",
            0b011 => "csrrc",
            0b101 => "csrrwi",
            0b110 => "csrrsi",
            0b111 => "csrrci",
            _ => return format!("unknown SYSTEM f3={}", funct3),
        };

        // CSR immediate instructions use zimm instead of rs1
        if funct3 & 0b100 != 0 {
            format!(
                "{} x{}=0x{:x}, 0x{:x}, {}",
                mnemonic, rd, rd_value, csr, zimm
            )
        } else {
            format!(
                "{} x{}=0x{:x}, 0x{:x}, x{}=0x{:x}",
                mnemonic, rd, rd_value, csr, rs1, rs1_value
            )
        }
    }
}

/// Disassemble atomic instructions (A extension)
#[allow(clippy::too_many_arguments)]
fn disassemble_atomic(
    instruction: u32,
    rd: u8,
    funct3: u8,
    rs1: u8,
    rs2: u8,
    rs1_value: u32,
    rs2_value: u32,
    rd_value: u32,
) -> String {
    // Only support word-sized atomics (funct3 = 010)
    if funct3 != 0b010 {
        return format!("unknown atomic f3={}", funct3);
    }

    let funct5 = (instruction >> 27) & 0x1F;
    let aq = (instruction >> 26) & 0x1;
    let rl = (instruction >> 25) & 0x1;

    let ordering = match (aq, rl) {
        (0, 0) => "",
        (1, 0) => ".aq",
        (0, 1) => ".rl",
        (1, 1) => ".aqrl",
        _ => "",
    };

    match funct5 {
        0b00010 => {
            // LR.W - Load-Reserved
            format!(
                "lr.w{} x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs1, rs1_value
            )
        }
        0b00011 => {
            // SC.W - Store-Conditional
            format!(
                "sc.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b00001 => {
            // AMOSWAP.W
            format!(
                "amoswap.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b00000 => {
            // AMOADD.W
            format!(
                "amoadd.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b00100 => {
            // AMOXOR.W
            format!(
                "amoxor.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b01100 => {
            // AMOAND.W
            format!(
                "amoand.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b01000 => {
            // AMOOR.W
            format!(
                "amoor.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b10000 => {
            // AMOMIN.W
            format!(
                "amomin.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b10100 => {
            // AMOMAX.W
            format!(
                "amomax.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b11000 => {
            // AMOMINU.W
            format!(
                "amominu.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        0b11100 => {
            // AMOMAXU.W
            format!(
                "amomaxu.w{} x{}=0x{:x}, x{}=0x{:x}, (x{}=0x{:x})",
                ordering, rd, rd_value, rs2, rs2_value, rs1, rs1_value
            )
        }
        _ => format!("unknown atomic funct5={}", funct5),
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
    fn test_disassemble_add() {
        // add x1, x2, x3
        let instruction = 0x003100B3;
        assert_eq!(disassemble(instruction), "add x1=0x0, x2=0x0, x3=0x0");
    }

    #[test]
    fn test_disassemble_addi() {
        // addi x1, x2, 42
        let instruction = 0x02A10093;
        assert_eq!(disassemble(instruction), "addi x1=0x0, x2=0x0, 42");
    }

    #[test]
    fn test_disassemble_lw() {
        // lw x1, 4(x2)
        let instruction = 0x00412083;
        assert_eq!(disassemble(instruction), "lw x1=0x0, 4(x2=0x0)");
    }

    #[test]
    fn test_disassemble_sw() {
        // sw x3, 8(x2)
        let instruction = 0x00312423;
        assert_eq!(disassemble(instruction), "sw x3=0x0, 8(x2=0x0)");
    }

    #[test]
    fn test_disassemble_beq() {
        // beq x1, x2, 16
        let instruction = 0x00208863;
        assert_eq!(disassemble(instruction), "beq x1=0x0, x2=0x0, 16");
    }

    #[test]
    fn test_disassemble_jal() {
        // jal x1, 0x100
        let instruction = 0x100000EF;
        assert_eq!(disassemble(instruction), "jal x1=0x0, 256");
    }

    #[test]
    fn test_disassemble_ecall() {
        // ecall
        let instruction = 0x00000073;
        assert_eq!(disassemble(instruction), "ecall");
    }

    #[test]
    fn test_disassemble_ebreak() {
        // ebreak
        let instruction = 0x00100073;
        assert_eq!(disassemble(instruction), "ebreak");
    }

    #[test]
    fn test_disassemble_fence() {
        // fence
        let instruction = 0x0000000F;
        assert_eq!(disassemble(instruction), "fence");
    }

    #[test]
    fn test_disassemble_csrrw() {
        // csrrw x1, 0x300, x2
        let instruction = 0x300110F3;
        let result = disassemble(instruction);
        assert!(result.contains("csrrw"));
        assert!(result.contains("x1"));
        assert!(result.contains("0x300"));
    }

    #[test]
    fn test_disassemble_csrrs() {
        // csrrs x1, 0x300, x2
        let instruction = 0x300120F3;
        let result = disassemble(instruction);
        assert!(result.contains("csrrs"));
    }

    #[test]
    fn test_disassemble_csrrwi() {
        // csrrwi x1, 0x300, 5
        let instruction = 0x3002D0F3;
        let result = disassemble(instruction);
        assert!(result.contains("csrrwi"));
        assert!(result.contains("0x300"));
    }

    #[test]
    fn test_disassemble_mul() {
        // mul x1, x2, x3
        let instruction = 0x023100B3;
        assert_eq!(disassemble(instruction), "mul x1=0x0, x2=0x0, x3=0x0");
    }

    #[test]
    fn test_disassemble_div() {
        // div x1, x2, x3
        let instruction = 0x023140B3;
        assert_eq!(disassemble(instruction), "div x1=0x0, x2=0x0, x3=0x0");
    }

    #[test]
    fn test_disassemble_rem() {
        // rem x1, x2, x3
        let instruction = 0x023160B3;
        assert_eq!(disassemble(instruction), "rem x1=0x0, x2=0x0, x3=0x0");
    }
}

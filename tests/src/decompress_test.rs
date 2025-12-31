use riscv_core::{create_decompress_runtime, Decompress};

// Helper function to encode I-type instruction
fn encode_i_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

// Helper function to encode R-type instruction
fn encode_r_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, rs2: u32, funct7: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

// Helper function to encode S-type instruction
fn encode_s_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    let imm_11_5 = (imm_u >> 5) & 0x7F;
    let imm_4_0 = imm_u & 0x1F;
    (imm_11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm_4_0 << 7) | opcode
}

// Helper function to encode U-type instruction
fn encode_u_type(opcode: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFFF000) | (rd << 7) | opcode
}

// ========== QUADRANT 0 TESTS ==========

#[test]
fn test_decompress_c_addi4spn() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI4SPN: addi rd', x2, nzuimm
    // Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
    // Example: addi x8, x2, 64
    // nzuimm = 64 = 0b01000000 (bits 7-2 of the value, with implicit 00 at end)
    // Encoding in instruction:
    //   - nzuimm[9:6] = 0b0100 goes in bits [10:7]
    //   - nzuimm[5:4] = 0b00 goes in bits [12:11]
    //   - nzuimm[3] = 0b0 goes in bit [5]
    //   - nzuimm[2] = 0b0 goes in bit [6]
    let insn_16: u16 = 0b000_00_0100_000_00;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x8, x2, 64
    let expected = encode_i_type(0b0010011, 8, 0b000, 2, 64);

    assert_eq!(dut.insn_32, expected, "C.ADDI4SPN decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi4spn_illegal() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI4SPN with nzuimm = 0 is illegal
    let insn_16: u16 = 0b000_00_0000_000_00;
    dut.insn_16 = insn_16;
    dut.eval();

    assert_eq!(dut.is_compressed, 1);
    assert_eq!(
        dut.is_valid, 0,
        "C.ADDI4SPN with nzuimm=0 should be illegal"
    );
}

#[test]
fn test_decompress_c_lw() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LW: lw rd', offset(rs1')
    // Format: 010 offset[5:3] rs1' offset[2|6] rd' 00
    // Example: lw x10, 8(x9)
    // offset = 8 = 0b0001000
    // offset[5:3]=001, offset[2]=0, offset[6]=0
    let insn_16: u16 = 0b010_001_001_00_010_00;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lw x10 (rd'=010 -> x10), 8(x9) (rs1'=001 -> x9)
    let expected = encode_i_type(0b0000011, 10, 0b010, 9, 8);

    assert_eq!(dut.insn_32, expected, "C.LW decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_sw() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SW: sw rs2', offset(rs1')
    // Format: 110 offset[5:3] rs1' offset[2|6] rs2' 00
    // Example: sw x11, 12(x8)
    // offset = 12 = 0b0001100
    // offset[5:3]=001, offset[2]=1, offset[6]=0
    let insn_16: u16 = 0b110_001_000_10_011_00;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sw x11 (rs2'=011 -> x11), 12(x8) (rs1'=000 -> x8)
    let expected = encode_s_type(0b0100011, 0b010, 8, 11, 12);

    assert_eq!(dut.insn_32, expected, "C.SW decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

// ========== QUADRANT 1 TESTS ==========

#[test]
fn test_decompress_c_nop() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.NOP: addi x0, x0, 0
    // Format: 000 0 00000 0 01
    let insn_16: u16 = 0b000_0_00000_0_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x0, x0, 0 = 0x00000013
    let expected = 0x00000013u32;

    assert_eq!(dut.insn_32, expected, "C.NOP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI: addi rd, rd, nzimm
    // Format: 000 imm[5] rd imm[4:0] 01
    // Example: addi x10, x10, 5
    // imm = 5 = 0b000101
    let insn_16: u16 = 0b000_0_01010_00101_01;
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

    // C.ADDI with negative immediate
    // Example: addi x10, x10, -5
    // imm = -5 = 0b111011 (6-bit sign-extended)
    let insn_16: u16 = 0b000_1_01010_11011_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x10, x10, -5
    let expected = encode_i_type(0b0010011, 10, 0b000, 10, -5);

    assert_eq!(
        dut.insn_32, expected,
        "C.ADDI (negative) decompression failed"
    );
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_jal() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JAL: jal x1, offset
    // Format: 001 offset[11|4|9:8|10|6|7|3:1|5] 01
    // Use offset = 0 for simplicity
    // funct3=001, offset=0 (11 bits), opcode=01
    // Binary: 0010_0000_0000_0001 = 0x2001
    let insn_16: u16 = 0x2001;
    dut.insn_16 = insn_16;
    dut.eval();

    // JAL encoding is complex, just verify it's compressed and valid
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
    // Verify it's a JAL to x1
    assert_eq!(dut.insn_32 & 0x7F, 0b1101111); // JAL opcode
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 1); // rd = x1
}

#[test]
fn test_decompress_c_li() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LI: addi rd, x0, imm
    // Format: 010 imm[5] rd imm[4:0] 01
    // Example: addi x10, x0, 42
    let insn_16: u16 = 0b010_0_01010_10101_01; // imm = 42 = 0b101010 (but only 6 bits)
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x10, x0, 21 (only lower 5 bits + sign bit)
    let expected = encode_i_type(0b0010011, 10, 0b000, 0, 21);

    assert_eq!(dut.insn_32, expected, "C.LI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_addi16sp() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADDI16SP: addi x2, x2, nzimm
    // Format: 011 nzimm[9] 00010 nzimm[4|6|8:7|5] 01
    // Example: addi x2, x2, 32
    // nzimm = 32 = 0b00000100000 (with 4 implicit zeros: bits 9-5, then 0000)
    //   - nzimm[9] = 0 goes to bit [12]
    //   - nzimm[8:7] = 00 goes to bits [4:3]
    //   - nzimm[6] = 0 goes to bit [5]
    //   - nzimm[5] = 1 goes to bit [2]
    //   - nzimm[4] = 0 goes to bit [6]
    // Binary: 011_0_00010_00100_01 = 0x6105
    let insn_16: u16 = 0x6105;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: addi x2, x2, 32
    let expected = encode_i_type(0b0010011, 2, 0b000, 2, 32);

    assert_eq!(dut.insn_32, expected, "C.ADDI16SP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_lui() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LUI: lui rd, nzimm
    // Format: 011 nzimm[17] rd nzimm[16:12] 01
    // Example: lui x10, 1
    // nzimm = 1 << 12 = 0x1000
    let insn_16: u16 = 0b011_0_01010_00001_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lui x10, 1
    let expected = encode_u_type(0b0110111, 10, 0x1000);

    assert_eq!(dut.insn_32, expected, "C.LUI decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_srli() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SRLI: srli rd', rd', shamt
    // Format: 100 0 00 rs1'/rd' shamt[4:0] 01
    // Example: srli x8, x8, 5
    let insn_16: u16 = 0b100_0_00_000_00101_01;
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
    // Format: 100 0 01 rs1'/rd' shamt[4:0] 01
    // Example: srai x9, x9, 3
    // This is I-type with funct7=0100000
    let insn_16: u16 = 0b100_0_01_001_00011_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: srai x9, x9, 3 (I-type with special funct7)
    // SRAI is encoded as: imm[11:5]=0100000, imm[4:0]=shamt, rs1=9, funct3=101, rd=9, opcode=0010011
    let expected = encode_i_type(0b0010011, 9, 0b101, 9, 0x403); // imm = 0x403 = 0b010000000011

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
    // Example: andi x10, x10, 15
    // imm = 15 = 0b001111, so imm[5]=0, imm[4:0]=01111
    let insn_16: u16 = 0b100_0_10_010_01111_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: andi x10, x10, 15 (sign-extended, so still 15)
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
    // Example: sub x10, x10, x11
    let insn_16: u16 = 0b100_0_11_010_00_011_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sub x10, x10, x11
    let expected = encode_r_type(0b0110011, 10, 0b000, 10, 11, 0b0100000);

    assert_eq!(dut.insn_32, expected, "C.SUB decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_xor() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.XOR: xor rd', rd', rs2'
    // Format: 100 0 11 rd'/rs1' 01 rs2' 01
    let insn_16: u16 = 0b100_0_11_010_01_011_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: xor x10, x10, x11
    let expected = encode_r_type(0b0110011, 10, 0b100, 10, 11, 0b0000000);

    assert_eq!(dut.insn_32, expected, "C.XOR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_or() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.OR: or rd', rd', rs2'
    // Format: 100 0 11 rd'/rs1' 10 rs2' 01
    let insn_16: u16 = 0b100_0_11_010_10_011_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: or x10, x10, x11
    let expected = encode_r_type(0b0110011, 10, 0b110, 10, 11, 0b0000000);

    assert_eq!(dut.insn_32, expected, "C.OR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_and() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.AND: and rd', rd', rs2'
    // Format: 100 0 11 rd'/rs1' 11 rs2' 01
    let insn_16: u16 = 0b100_0_11_010_11_011_01;
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
    // Format: 101 offset[11|4|9:8|10|6|7|3:1|5] 01
    // Use offset = 0 for simplicity
    // funct3=101, offset=0 (11 bits), opcode=01
    // Binary: 1010_0000_0000_0001 = 0xA001
    let insn_16: u16 = 0xA001;
    dut.insn_16 = insn_16;
    dut.eval();

    println!(
        "C.J test - insn_16 = 0x{:04x} = 0b{:016b}",
        insn_16, insn_16
    );
    println!("C.J test - insn_32 = 0x{:08x}", dut.insn_32);
    println!(
        "C.J test - opcode = 0x{:02x} (expected 0x6F)",
        dut.insn_32 & 0x7F
    );

    // Verify it's a JAL to x0
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
    assert_eq!(dut.insn_32 & 0x7F, 0b1101111, "Wrong opcode"); // JAL opcode
    assert_eq!((dut.insn_32 >> 7) & 0x1F, 0); // rd = x0
}

#[test]
fn test_decompress_c_beqz() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.BEQZ: beq rs1', x0, offset
    // Format: 110 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
    // Example: beq x9, x0, 8
    let insn_16: u16 = 0b110_0_01_001_000_10_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Verify it's a BEQ with rs1' and x0
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
    assert_eq!(dut.insn_32 & 0x7F, 0b1100011); // BRANCH opcode
    assert_eq!((dut.insn_32 >> 12) & 0x7, 0b000); // funct3 = BEQ
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 9); // rs1 = x9
    assert_eq!((dut.insn_32 >> 20) & 0x1F, 0); // rs2 = x0
}

#[test]
fn test_decompress_c_bnez() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.BNEZ: bne rs1', x0, offset
    // Format: 111 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
    let insn_16: u16 = 0b111_0_01_001_000_10_01;
    dut.insn_16 = insn_16;
    dut.eval();

    // Verify it's a BNE with rs1' and x0
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
    assert_eq!(dut.insn_32 & 0x7F, 0b1100011); // BRANCH opcode
    assert_eq!((dut.insn_32 >> 12) & 0x7, 0b001); // funct3 = BNE
    assert_eq!((dut.insn_32 >> 15) & 0x1F, 9); // rs1 = x9
    assert_eq!((dut.insn_32 >> 20) & 0x1F, 0); // rs2 = x0
}

// ========== QUADRANT 2 TESTS ==========

#[test]
fn test_decompress_c_slli() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.SLLI: slli rd, rd, shamt
    // Format: 000 shamt[5] rd shamt[4:0] 10
    // Example: slli x10, x10, 5
    let insn_16: u16 = 0b000_0_01010_00101_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: slli x10, x10, 5
    let expected = encode_i_type(0b0010011, 10, 0b001, 10, 5);

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
    // Example: lw x10, 8(x2)
    // offset = 8 = 0b00001000
    // offset[5]=0, offset[4:2]=010, offset[7:6]=00
    let insn_16: u16 = 0b010_0_01010_010_00_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: lw x10, 8(x2)
    let expected = encode_i_type(0b0000011, 10, 0b010, 2, 8);

    assert_eq!(dut.insn_32, expected, "C.LWSP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_lwsp_illegal() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.LWSP with rd = 0 is illegal
    let insn_16: u16 = 0b010_0_00000_010_00_10;
    dut.insn_16 = insn_16;
    dut.eval();

    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 0, "C.LWSP with rd=0 should be illegal");
}

#[test]
fn test_decompress_c_jr() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JR: jalr x0, 0(rs1)
    // Format: 100 0 rs1 00000 10
    // Example: jalr x0, 0(x10)
    let insn_16: u16 = 0b100_0_01010_00000_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jalr x0, 0(x10)
    let expected = encode_i_type(0b1100111, 0, 0b000, 10, 0);

    assert_eq!(dut.insn_32, expected, "C.JR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_mv() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.MV: add rd, x0, rs2
    // Format: 100 0 rd rs2 10
    // Example: add x10, x0, x11
    let insn_16: u16 = 0b100_0_01010_01011_10;
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

    // C.EBREAK: ebreak
    // Format: 100 1 00000 00000 10
    let insn_16: u16 = 0b100_1_00000_00000_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: ebreak = 0x00100073
    let expected = 0x00100073u32;

    assert_eq!(dut.insn_32, expected, "C.EBREAK decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_jalr() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.JALR: jalr x1, 0(rs1)
    // Format: 100 1 rs1 00000 10
    // Example: jalr x1, 0(x10)
    let insn_16: u16 = 0b100_1_01010_00000_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: jalr x1, 0(x10)
    let expected = encode_i_type(0b1100111, 1, 0b000, 10, 0);

    assert_eq!(dut.insn_32, expected, "C.JALR decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

#[test]
fn test_decompress_c_add() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // C.ADD: add rd, rd, rs2
    // Format: 100 1 rd rs2 10
    // Example: add x10, x10, x11
    let insn_16: u16 = 0b100_1_01010_01011_10;
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
    // Format: 110 offset[5:2|7:6] rs2 10
    // Example: sw x10, 8(x2)
    // offset = 8 = 0b00001000
    // offset[5:2]=0010, offset[7:6]=00
    let insn_16: u16 = 0b110_0010_00_01010_10;
    dut.insn_16 = insn_16;
    dut.eval();

    // Expected: sw x10, 8(x2)
    let expected = encode_s_type(0b0100011, 0b010, 2, 10, 8);

    assert_eq!(dut.insn_32, expected, "C.SWSP decompression failed");
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}

// ========== SPECIAL CASES ==========

#[test]
fn test_decompress_non_compressed() {
    let runtime = create_decompress_runtime().expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();

    // Standard 32-bit instruction (lower 16 bits, opcode[1:0] == 11)
    // This is just the lower half - actual instruction assembly happens elsewhere
    let insn_16: u16 = 0b0000_0000_0001_0011; // Lower 16 bits of ADDI
    dut.insn_16 = insn_16;
    dut.eval();

    assert_eq!(dut.is_compressed, 0, "Should detect as non-compressed");
    assert_eq!(dut.is_valid, 1);
}

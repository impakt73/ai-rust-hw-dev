// RV32C Instruction Decompressor
// Expands 16-bit compressed instructions to 32-bit standard RISC-V instructions
// 
// This module is purely combinational and performs transparent decompression
// of all 27 RV32C compressed instructions defined in the RISC-V spec.
//
// Compressed instructions are identified by bits [1:0] != 2'b11

module decompress (
    input  logic [15:0] insn_16,        // 16-bit instruction input
    output logic [31:0] insn_32,        // 32-bit expanded instruction
    output logic        is_compressed,  // 1 if input was compressed
    output logic        is_valid        // 1 if valid instruction
);

    // Extract common fields
    logic [1:0]  quadrant;    // insn_16[1:0]
    logic [2:0]  funct3;      // insn_16[15:13]
    logic [4:0]  rd_full;     // Full 5-bit rd
    logic [4:0]  rs1_full;    // Full 5-bit rs1
    logic [4:0]  rs2_full;    // Full 5-bit rs2
    logic [2:0]  rd_comp;     // Compressed 3-bit rd (x8-x15)
    logic [2:0]  rs1_comp;    // Compressed 3-bit rs1 (x8-x15)
    logic [2:0]  rs2_comp;    // Compressed 3-bit rs2 (x8-x15)
    logic [31:0] imm;         // Immediate value
    
    assign quadrant = insn_16[1:0];
    assign funct3 = insn_16[15:13];
    
    // Compressed register mapping: x8-x15
    assign rd_comp = insn_16[4:2];
    assign rs1_comp = insn_16[9:7];
    assign rs2_comp = insn_16[4:2];
    
    // Determine if instruction is compressed
    assign is_compressed = (quadrant != 2'b11);
    
    always_comb begin
        // Default values
        insn_32 = 32'h00000013;  // Default to NOP (addi x0, x0, 0)
        is_valid = 1'b1;
        rd_full = 5'b0;
        rs1_full = 5'b0;
        rs2_full = 5'b0;
        imm = 32'b0;
        
        if (quadrant == 2'b11) begin
            // Standard 32-bit instruction (partial - needs more bits)
            // This will be handled by the fetch unit
            insn_32 = {16'h0, insn_16};
            is_valid = 1'b0;  // Incomplete, need full 32 bits
        end
        
        // ========== Quadrant 0 (opcode == 2'b00) ==========
        else if (quadrant == 2'b00) begin
            case (funct3)
                3'b000: begin // C.ADDI4SPN
                    // Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
                    // Expands to: addi rd', x2, nzuimm
                    rd_full = {2'b01, rd_comp};
                    imm = {22'b0, insn_16[10:7], insn_16[12:11], insn_16[5], insn_16[6], 2'b00};
                    insn_32 = {imm[11:0], 5'd2, 3'b000, rd_full, 7'b0010011};
                    is_valid = (imm != 0);  // nzuimm must be non-zero
                end
                
                3'b010: begin // C.LW
                    // Format: 010 offset[5:3] rs1' offset[2|6] rd' 00
                    // Expands to: lw rd', offset(rs1')
                    rd_full = {2'b01, rd_comp};
                    rs1_full = {2'b01, rs1_comp};
                    imm = {25'b0, insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                    insn_32 = {imm[11:0], rs1_full, 3'b010, rd_full, 7'b0000011};
                end
                
                3'b110: begin // C.SW
                    // Format: 110 offset[5:3] rs1' offset[2|6] rs2' 00
                    // Expands to: sw rs2', offset(rs1')
                    rs1_full = {2'b01, rs1_comp};
                    rs2_full = {2'b01, rs2_comp};
                    imm = {25'b0, insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                    insn_32 = {imm[11:5], rs2_full, rs1_full, 3'b010, imm[4:0], 7'b0100011};
                end
                
                default: begin
                    // Reserved or illegal
                    is_valid = 1'b0;
                end
            endcase
        end
        
        // ========== Quadrant 1 (opcode == 2'b01) ==========
        else if (quadrant == 2'b01) begin
            case (funct3)
                3'b000: begin // C.NOP or C.ADDI
                    rd_full = insn_16[11:7];
                    // Sign-extend 6-bit immediate: insn[12] is sign bit
                    imm = {{26{insn_16[12]}}, insn_16[12], insn_16[6:2]};
                    
                    if (rd_full == 5'b0 && imm == 0) begin
                        // C.NOP: addi x0, x0, 0
                        insn_32 = 32'h00000013;
                    end else begin
                        // C.ADDI: addi rd, rd, nzimm
                        insn_32 = {imm[11:0], rd_full, 3'b000, rd_full, 7'b0010011};
                        // Note: nzimm can be zero for non-x0 rd (hint instruction)
                    end
                end
                
                3'b001: begin // C.JAL (RV32C only, not in RV64C)
                    // Format: 001 offset[11|4|9:8|10|6|7|3:1|5] 01
                    // Expands to: jal x1, offset
                    imm = {{20{insn_16[12]}}, insn_16[12], insn_16[8], 
                           insn_16[10:9], insn_16[6], insn_16[7], insn_16[2], 
                           insn_16[11], insn_16[5:3], 1'b0};
                    insn_32 = {imm[20], imm[10:1], imm[11], imm[19:12], 5'd1, 7'b1101111};
                end
                
                3'b010: begin // C.LI
                    // Format: 010 imm[5] rd imm[4:0] 01
                    // Expands to: addi rd, x0, imm
                    rd_full = insn_16[11:7];
                    imm = {{26{insn_16[12]}}, insn_16[12], insn_16[6:2]};
                    insn_32 = {imm[11:0], 5'b0, 3'b000, rd_full, 7'b0010011};
                end
                
                3'b011: begin // C.ADDI16SP or C.LUI
                    rd_full = insn_16[11:7];
                    
                    if (rd_full == 5'd2) begin
                        // C.ADDI16SP: addi x2, x2, nzimm
                        // Format: 011 nzimm[9] 2 nzimm[4|6|8:7|5] 01
                        imm = {{22{insn_16[12]}}, insn_16[12], insn_16[4:3], 
                               insn_16[5], insn_16[2], insn_16[6], 4'b0};
                        insn_32 = {imm[11:0], 5'd2, 3'b000, 5'd2, 7'b0010011};
                        is_valid = (imm != 0);  // nzimm must be non-zero
                    end else if (rd_full != 5'b0) begin
                        // C.LUI: lui rd, nzimm
                        // Format: 011 nzimm[17] rd nzimm[16:12] 01
                        imm = {{14{insn_16[12]}}, insn_16[12], insn_16[6:2], 12'b0};
                        insn_32 = {imm[31:12], rd_full, 7'b0110111};
                        is_valid = (imm != 0);  // nzimm must be non-zero
                    end else begin
                        // Reserved (rd == 0)
                        is_valid = 1'b0;
                    end
                end
                
                3'b100: begin // ALU operations (SRLI, SRAI, ANDI, SUB, XOR, OR, AND)
                    logic [1:0] funct2_high;
                    logic [1:0] funct2_low;
                    funct2_high = insn_16[11:10];
                    funct2_low = insn_16[6:5];
                    rd_full = {2'b01, rs1_comp};
                    rs2_full = {2'b01, rs2_comp};
                    
                    case (funct2_high)
                        2'b00: begin // C.SRLI
                            // Expands to: srli rd', rd', shamt
                            imm = {26'b0, insn_16[12], insn_16[6:2]};
                            insn_32 = {7'b0000000, imm[4:0], rd_full, 3'b101, rd_full, 7'b0010011};
                        end
                        
                        2'b01: begin // C.SRAI
                            // Expands to: srai rd', rd', shamt
                            imm = {26'b0, insn_16[12], insn_16[6:2]};
                            insn_32 = {7'b0100000, imm[4:0], rd_full, 3'b101, rd_full, 7'b0010011};
                        end
                        
                        2'b10: begin // C.ANDI
                            // Expands to: andi rd', rd', imm
                            imm = {{26{insn_16[12]}}, insn_16[12], insn_16[6:2]};
                            insn_32 = {imm[11:0], rd_full, 3'b111, rd_full, 7'b0010011};
                        end
                        
                        2'b11: begin // Register-register ALU operations
                            if (!insn_16[12]) begin  // funct6[5] == 0
                                case (funct2_low)
                                    2'b00: begin // C.SUB
                                        insn_32 = {7'b0100000, rs2_full, rd_full, 3'b000, rd_full, 7'b0110011};
                                    end
                                    2'b01: begin // C.XOR
                                        insn_32 = {7'b0000000, rs2_full, rd_full, 3'b100, rd_full, 7'b0110011};
                                    end
                                    2'b10: begin // C.OR
                                        insn_32 = {7'b0000000, rs2_full, rd_full, 3'b110, rd_full, 7'b0110011};
                                    end
                                    2'b11: begin // C.AND
                                        insn_32 = {7'b0000000, rs2_full, rd_full, 3'b111, rd_full, 7'b0110011};
                                    end
                                endcase
                            end else begin
                                // Reserved for RV64/RV128
                                is_valid = 1'b0;
                            end
                        end
                    endcase
                end
                
                3'b101: begin // C.J
                    // Format: 101 offset[11|4|9:8|10|6|7|3:1|5] 01
                    // Expands to: jal x0, offset
                    imm = {{20{insn_16[12]}}, insn_16[12], insn_16[8], 
                           insn_16[10:9], insn_16[6], insn_16[7], insn_16[2], 
                           insn_16[11], insn_16[5:3], 1'b0};
                    insn_32 = {imm[20], imm[10:1], imm[11], imm[19:12], 5'b0, 7'b1101111};
                end
                
                3'b110: begin // C.BEQZ
                    // Format: 110 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
                    // Expands to: beq rs1', x0, offset
                    rs1_full = {2'b01, rs1_comp};
                    imm = {{23{insn_16[12]}}, insn_16[12], insn_16[6:5], insn_16[2], 
                           insn_16[11:10], insn_16[4:3], 1'b0};
                    // B-type encoding
                    insn_32 = {imm[12], imm[10:5], 5'b0, rs1_full, 3'b000, 
                               imm[4:1], imm[11], 7'b1100011};
                end
                
                3'b111: begin // C.BNEZ
                    // Format: 111 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
                    // Expands to: bne rs1', x0, offset
                    rs1_full = {2'b01, rs1_comp};
                    imm = {{23{insn_16[12]}}, insn_16[12], insn_16[6:5], insn_16[2], 
                           insn_16[11:10], insn_16[4:3], 1'b0};
                    // B-type encoding
                    insn_32 = {imm[12], imm[10:5], 5'b0, rs1_full, 3'b001, 
                               imm[4:1], imm[11], 7'b1100011};
                end
            endcase
        end
        
        // ========== Quadrant 2 (opcode == 2'b10) ==========
        else if (quadrant == 2'b10) begin
            case (funct3)
                3'b000: begin // C.SLLI
                    // Format: 000 shamt[5] rd shamt[4:0] 10
                    // Expands to: slli rd, rd, shamt
                    rd_full = insn_16[11:7];
                    imm = {26'b0, insn_16[12], insn_16[6:2]};
                    insn_32 = {7'b0000000, imm[4:0], rd_full, 3'b001, rd_full, 7'b0010011};
                    is_valid = (rd_full != 0);  // rd must not be x0
                end
                
                3'b010: begin // C.LWSP
                    // Format: 010 offset[5] rd offset[4:2|7:6] 10
                    // Expands to: lw rd, offset(x2)
                    rd_full = insn_16[11:7];
                    imm = {24'b0, insn_16[3:2], insn_16[12], insn_16[6:4], 2'b00};
                    insn_32 = {imm[11:0], 5'd2, 3'b010, rd_full, 7'b0000011};
                    is_valid = (rd_full != 0);  // rd must not be x0
                end
                
                3'b100: begin // C.JR, C.MV, C.EBREAK, C.JALR, C.ADD
                    rd_full = insn_16[11:7];
                    rs2_full = insn_16[6:2];
                    
                    if (!insn_16[12]) begin  // funct4[3] == 0
                        if (rs2_full == 5'b0) begin
                            // C.JR: jalr x0, 0(rs1)
                            insn_32 = {12'b0, rd_full, 3'b000, 5'b0, 7'b1100111};
                            is_valid = (rd_full != 0);  // rs1 must not be x0
                        end else begin
                            // C.MV: add rd, x0, rs2
                            insn_32 = {7'b0000000, rs2_full, 5'b0, 3'b000, rd_full, 7'b0110011};
                            is_valid = (rd_full != 0);  // rd must not be x0
                        end
                    end else begin  // funct4[3] == 1
                        if (rd_full == 5'b0 && rs2_full == 5'b0) begin
                            // C.EBREAK
                            insn_32 = 32'h00100073;  // ebreak
                        end else if (rs2_full == 5'b0) begin
                            // C.JALR: jalr x1, 0(rs1)
                            insn_32 = {12'b0, rd_full, 3'b000, 5'd1, 7'b1100111};
                            is_valid = (rd_full != 0);  // rs1 must not be x0
                        end else begin
                            // C.ADD: add rd, rd, rs2
                            insn_32 = {7'b0000000, rs2_full, rd_full, 3'b000, rd_full, 7'b0110011};
                            is_valid = (rd_full != 0);  // rd must not be x0
                        end
                    end
                end
                
                3'b110: begin // C.SWSP
                    // Format: 110 offset[5:2|7:6] rs2 10
                    // Expands to: sw rs2, offset(x2)
                    rs2_full = insn_16[6:2];
                    imm = {24'b0, insn_16[8:7], insn_16[12:9], 2'b00};
                    insn_32 = {imm[11:5], rs2_full, 5'd2, 3'b010, imm[4:0], 7'b0100011};
                end
                
                default: begin
                    // Reserved or illegal
                    is_valid = 1'b0;
                end
            endcase
        end
        
        // Check for all-zeros (illegal instruction)
        if (insn_16 == 16'h0000) begin
            is_valid = 1'b0;
        end
    end

endmodule

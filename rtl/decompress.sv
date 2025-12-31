// RV32C Compressed Instruction Decompressor
// Expands 16-bit compressed instructions to 32-bit standard RISC-V instructions

module decompress (
    input  logic [15:0] insn_16,        // 16-bit instruction input
    output logic [31:0] insn_32,        // 32-bit expanded instruction
    output logic        is_compressed,  // 1 if input was compressed
    output logic        is_valid        // 1 if valid instruction
);

    // Detect compressed vs standard instructions
    // Bits [1:0] != 11 indicates compressed instruction
    logic [1:0] opcode_low;
    logic [2:0] funct3;
    logic [1:0] funct2_misc;  // For quadrant 1, funct3=100 sub-operations
    logic [1:0] funct2_alu;   // For quadrant 1, funct3=100, funct2_misc=11 ALU ops
    
    assign opcode_low = insn_16[1:0];
    assign funct3 = insn_16[15:13];
    assign funct2_misc = insn_16[11:10];  // Distinguishes SRLI/SRAI/ANDI/ALU-ops
    assign funct2_alu = insn_16[6:5];     // For SUB/XOR/OR/AND
    
    // Check if instruction is compressed
    assign is_compressed = (opcode_low != 2'b11);
    
    // Register field extraction
    logic [4:0] rd, rs1, rs2;
    logic [4:0] rd_p, rs1_p, rs2_p;  // Compressed register specifiers
    
    // Compressed registers map to x8-x15
    assign rd_p  = {2'b01, insn_16[4:2]};
    assign rs1_p = {2'b01, insn_16[9:7]};
    assign rs2_p = {2'b01, insn_16[4:2]};
    
    // Standard register fields
    assign rd  = insn_16[11:7];
    assign rs1 = insn_16[11:7];
    assign rs2 = insn_16[6:2];
    
    // Immediate values (decoded based on instruction type)
    logic [31:0] imm_addi4spn;  // C.ADDI4SPN
    logic [31:0] imm_lw_sw;     // C.LW, C.SW
    logic [31:0] imm_addi;      // C.ADDI, C.LI, C.ANDI
    logic [31:0] imm_addi16sp;  // C.ADDI16SP
    logic [31:0] imm_lui;       // C.LUI
    logic [31:0] imm_j;         // C.J, C.JAL
    logic [31:0] imm_beqz;      // C.BEQZ, C.BNEZ
    logic [31:0] imm_lwsp;      // C.LWSP
    logic [31:0] imm_swsp;      // C.SWSP
    logic [5:0]  shamt_full;    // Shift amount (6 bits total)
    logic [4:0]  shamt;         // Shift amount (lower 5 bits)
    
    // Immediate decoding
    // C.ADDI4SPN: nzuimm[5:4|9:6|2|3] in bits [12:5]
    // Reconstruct as: {nzuimm[9:6], nzuimm[5:4], nzuimm[3], nzuimm[2], 2'b00}
    assign imm_addi4spn = {22'b0, insn_16[10:7], insn_16[12:11], insn_16[5], insn_16[6], 2'b00};
    
    // C.LW/C.SW: offset[5:3|2|6] in bits [12:10|6|5]
    // Reconstruct as: {offset[6], offset[5:3], offset[2], 2'b00}
    assign imm_lw_sw = {25'b0, insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
    
    // C.ADDI, C.LI, C.ANDI: imm[5|4:0] (sign-extended)
    assign imm_addi = {{26{insn_16[12]}}, insn_16[12], insn_16[6:2]};
    
    // C.ADDI16SP: nzimm[9|4|6|8:7|5] (sign-extended)
    assign imm_addi16sp = {{22{insn_16[12]}}, insn_16[12], insn_16[4:3], insn_16[5], insn_16[2], insn_16[6], 4'b0000};
    
    // C.LUI: nzimm[17|16:12] (sign-extended)
    assign imm_lui = {{14{insn_16[12]}}, insn_16[12], insn_16[6:2], 12'b0};
    
    // C.J/C.JAL: offset[11|4|9:8|10|6|7|3:1|5] (sign-extended)
    assign imm_j = {{20{insn_16[12]}}, insn_16[12], insn_16[8], insn_16[10:9], insn_16[6], insn_16[7], insn_16[2], insn_16[11], insn_16[5:3], 1'b0};
    
    // C.BEQZ/C.BNEZ: offset[8|4:3|7:6|2:1|5] (sign-extended)
    assign imm_beqz = {{23{insn_16[12]}}, insn_16[12], insn_16[6:5], insn_16[2], insn_16[11:10], insn_16[4:3], 1'b0};
    
    // C.LWSP: offset[5|4:2|7:6]
    assign imm_lwsp = {24'b0, insn_16[3:2], insn_16[12], insn_16[6:4], 2'b00};
    
    // C.SWSP: offset[5:2|7:6]
    assign imm_swsp = {24'b0, insn_16[8:7], insn_16[12:9], 2'b00};
    
    // Shift amount for C.SLLI, C.SRLI, C.SRAI
    assign shamt_full = {insn_16[12], insn_16[6:2]};
    assign shamt = shamt_full[4:0];
    
    // Decompression logic
    always_comb begin
        // Default values
        insn_32 = 32'h00000013;  // Default to ADDI x0, x0, 0 (NOP)
        is_valid = 1'b1;
        
        if (!is_compressed) begin
            // Not a compressed instruction - need more bits
            // This will be handled at a higher level
            insn_32 = 32'h00000013;
            is_valid = 1'b1;
        end else begin
            case (opcode_low)
                // ========== QUADRANT 0 (opcode[1:0] == 00) ==========
                2'b00: begin
                    case (funct3)
                        3'b000: begin
                            // C.ADDI4SPN: addi rd', x2, nzuimm
                            // Illegal if nzuimm == 0
                            if (imm_addi4spn == 0) begin
                                is_valid = 1'b0;
                                insn_32 = 32'h00000000;  // Illegal
                            end else begin
                                insn_32 = {imm_addi4spn[11:0], 5'd2, 3'b000, rd_p, 7'b0010011};
                            end
                        end
                        
                        3'b010: begin
                            // C.LW: lw rd', offset(rs1')
                            insn_32 = {imm_lw_sw[11:0], rs1_p, 3'b010, rd_p, 7'b0000011};
                        end
                        
                        3'b110: begin
                            // C.SW: sw rs2', offset(rs1')
                            insn_32 = {imm_lw_sw[11:5], rs2_p, rs1_p, 3'b010, imm_lw_sw[4:0], 7'b0100011};
                        end
                        
                        default: begin
                            // Reserved or illegal
                            is_valid = 1'b0;
                            insn_32 = 32'h00000000;
                        end
                    endcase
                end
                
                // ========== QUADRANT 1 (opcode[1:0] == 01) ==========
                2'b01: begin
                    case (funct3)
                        3'b000: begin
                            // C.NOP or C.ADDI
                            if (rd == 5'd0 && imm_addi == 0) begin
                                // C.NOP: addi x0, x0, 0
                                insn_32 = 32'h00000013;
                            end else if (rd != 5'd0) begin
                                // C.ADDI: addi rd, rd, nzimm
                                insn_32 = {imm_addi[11:0], rd, 3'b000, rd, 7'b0010011};
                            end else begin
                                // rd == 0 but imm != 0: reserved
                                is_valid = 1'b0;
                                insn_32 = 32'h00000000;
                            end
                        end
                        
                        3'b001: begin
                            // C.JAL: jal x1, offset (RV32C only)
                            insn_32 = {imm_j[20], imm_j[10:1], imm_j[11], imm_j[19:12], 5'd1, 7'b1101111};
                        end
                        
                        3'b010: begin
                            // C.LI: addi rd, x0, imm
                            insn_32 = {imm_addi[11:0], 5'd0, 3'b000, rd, 7'b0010011};
                        end
                        
                        3'b011: begin
                            if (rd == 5'd2) begin
                                // C.ADDI16SP: addi x2, x2, nzimm
                                // Illegal if nzimm == 0
                                if (imm_addi16sp == 0) begin
                                    is_valid = 1'b0;
                                    insn_32 = 32'h00000000;
                                end else begin
                                    insn_32 = {imm_addi16sp[11:0], 5'd2, 3'b000, 5'd2, 7'b0010011};
                                end
                            end else if (rd != 5'd0) begin
                                // C.LUI: lui rd, nzimm
                                // Illegal if rd == 0 or nzimm == 0
                                if (imm_lui == 0) begin
                                    is_valid = 1'b0;
                                    insn_32 = 32'h00000000;
                                end else begin
                                    insn_32 = {imm_lui[31:12], rd, 7'b0110111};
                                end
                            end else begin
                                // rd == 0: reserved
                                is_valid = 1'b0;
                                insn_32 = 32'h00000000;
                            end
                        end
                        
                        3'b100: begin
                            // Miscellaneous ALU instructions
                            case (funct2_misc)
                                2'b00: begin
                                    // C.SRLI: srli rd', rd', shamt
                                    // Illegal if shamt == 0
                                    if (shamt == 0) begin
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end else begin
                                        insn_32 = {7'b0000000, shamt, rs1_p, 3'b101, rs1_p, 7'b0010011};
                                    end
                                end
                                
                                2'b01: begin
                                    // C.SRAI: srai rd', rd', shamt
                                    // Illegal if shamt == 0
                                    if (shamt == 0) begin
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end else begin
                                        insn_32 = {7'b0100000, shamt, rs1_p, 3'b101, rs1_p, 7'b0010011};
                                    end
                                end
                                
                                2'b10: begin
                                    // C.ANDI: andi rd', rd', imm
                                    insn_32 = {imm_addi[11:0], rs1_p, 3'b111, rs1_p, 7'b0010011};
                                end
                                
                                2'b11: begin
                                    // Register-register operations
                                    if (insn_16[12] == 1'b0) begin
                                        case (funct2_alu)
                                            2'b00: begin
                                                // C.SUB: sub rd', rd', rs2'
                                                insn_32 = {7'b0100000, rs2_p, rs1_p, 3'b000, rs1_p, 7'b0110011};
                                            end
                                            2'b01: begin
                                                // C.XOR: xor rd', rd', rs2'
                                                insn_32 = {7'b0000000, rs2_p, rs1_p, 3'b100, rs1_p, 7'b0110011};
                                            end
                                            2'b10: begin
                                                // C.OR: or rd', rd', rs2'
                                                insn_32 = {7'b0000000, rs2_p, rs1_p, 3'b110, rs1_p, 7'b0110011};
                                            end
                                            2'b11: begin
                                                // C.AND: and rd', rd', rs2'
                                                insn_32 = {7'b0000000, rs2_p, rs1_p, 3'b111, rs1_p, 7'b0110011};
                                            end
                                        endcase
                                    end else begin
                                        // Reserved for RV64C/RV128C
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end
                                end
                            endcase
                        end
                        
                        3'b101: begin
                            // C.J: jal x0, offset
                            insn_32 = {imm_j[20], imm_j[10:1], imm_j[11], imm_j[19:12], 5'd0, 7'b1101111};
                        end
                        
                        3'b110: begin
                            // C.BEQZ: beq rs1', x0, offset
                            insn_32 = {imm_beqz[12], imm_beqz[10:5], 5'd0, rs1_p, 3'b000, imm_beqz[4:1], imm_beqz[11], 7'b1100011};
                        end
                        
                        3'b111: begin
                            // C.BNEZ: bne rs1', x0, offset
                            insn_32 = {imm_beqz[12], imm_beqz[10:5], 5'd0, rs1_p, 3'b001, imm_beqz[4:1], imm_beqz[11], 7'b1100011};
                        end
                    endcase
                end
                
                // ========== QUADRANT 2 (opcode[1:0] == 10) ==========
                2'b10: begin
                    case (funct3)
                        3'b000: begin
                            // C.SLLI: slli rd, rd, shamt
                            // Illegal if rd == 0 or shamt == 0
                            if (rd == 5'd0 || shamt == 0) begin
                                is_valid = 1'b0;
                                insn_32 = 32'h00000000;
                            end else begin
                                insn_32 = {7'b0000000, shamt, rd, 3'b001, rd, 7'b0010011};
                            end
                        end
                        
                        3'b010: begin
                            // C.LWSP: lw rd, offset(x2)
                            // Illegal if rd == 0
                            if (rd == 5'd0) begin
                                is_valid = 1'b0;
                                insn_32 = 32'h00000000;
                            end else begin
                                insn_32 = {imm_lwsp[11:0], 5'd2, 3'b010, rd, 7'b0000011};
                            end
                        end
                        
                        3'b100: begin
                            if (insn_16[12] == 1'b0) begin
                                if (rs2 == 5'd0) begin
                                    // C.JR: jalr x0, 0(rs1)
                                    // Illegal if rs1 == 0
                                    if (rs1 == 5'd0) begin
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end else begin
                                        insn_32 = {12'b0, rs1, 3'b000, 5'd0, 7'b1100111};
                                    end
                                end else begin
                                    // C.MV: add rd, x0, rs2
                                    // Illegal if rd == 0
                                    if (rd == 5'd0) begin
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end else begin
                                        insn_32 = {7'b0000000, rs2, 5'd0, 3'b000, rd, 7'b0110011};
                                    end
                                end
                            end else begin
                                // insn_16[12] == 1
                                if (rs1 == 5'd0 && rs2 == 5'd0) begin
                                    // C.EBREAK: ebreak
                                    insn_32 = 32'h00100073;
                                end else if (rs2 == 5'd0) begin
                                    // C.JALR: jalr x1, 0(rs1)
                                    insn_32 = {12'b0, rs1, 3'b000, 5'd1, 7'b1100111};
                                end else begin
                                    // C.ADD: add rd, rd, rs2
                                    // Illegal if rd == 0
                                    if (rd == 5'd0) begin
                                        is_valid = 1'b0;
                                        insn_32 = 32'h00000000;
                                    end else begin
                                        insn_32 = {7'b0000000, rs2, rd, 3'b000, rd, 7'b0110011};
                                    end
                                end
                            end
                        end
                        
                        3'b110: begin
                            // C.SWSP: sw rs2, offset(x2)
                            insn_32 = {imm_swsp[11:5], rs2, 5'd2, 3'b010, imm_swsp[4:0], 7'b0100011};
                        end
                        
                        default: begin
                            // Reserved or illegal
                            is_valid = 1'b0;
                            insn_32 = 32'h00000000;
                        end
                    endcase
                end
                
                default: begin
                    // Should not reach here (opcode[1:0] == 11 is handled above)
                    is_valid = 1'b0;
                    insn_32 = 32'h00000000;
                end
            endcase
        end
    end

endmodule

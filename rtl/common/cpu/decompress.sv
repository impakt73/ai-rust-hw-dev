`default_nettype none
// RV32C Instruction Decompressor
// Expands 16-bit compressed instructions to 32-bit standard RISC-V instructions
// Pure combinational logic (no clock, no state)

/* verilator lint_off WIDTHTRUNC */
/* verilator lint_off MULTIDRIVEN */
/* verilator lint_off WIDTHEXPAND */

module decompress (
    input wire logic [15:0] insn_16,        // 16-bit instruction input (lower half for detection)
    input wire logic [31:0] insn_32_in,     // Full 32-bit input (for non-compressed passthrough)
    output logic [31:0] insn_32,        // 32-bit expanded instruction
    output logic        is_compressed,  // 1 if input was compressed
    output logic        is_valid        // 1 if valid instruction
);

    // Detect compressed instruction (bits [1:0] != 2'b11)
    assign is_compressed = (insn_16[1:0] != 2'b11);
    
    // Quadrant extraction
    logic [1:0] quadrant;
    logic [2:0] funct3;
    assign quadrant = insn_16[1:0];
    assign funct3 = insn_16[15:13];
    
    // Helper signals for register decoding
    logic [4:0] rd_full, rs1_full, rs2_full;
    logic [2:0] rd_compressed, rs1_compressed, rs2_compressed;
    
    // Compressed register mapping: 3-bit -> 5-bit (x8-x15)
    assign rd_compressed = insn_16[4:2];
    assign rs1_compressed = insn_16[9:7];
    assign rs2_compressed = insn_16[4:2];
    
    // Full register addresses
    assign rd_full = {2'b01, rd_compressed};   // x8-x15
    assign rs1_full = {2'b01, rs1_compressed}; // x8-x15
    assign rs2_full = {2'b01, rs2_compressed}; // x8-x15
    
    // ============================================================
    // Main Decompression Logic
    // ============================================================
    always_comb begin
        // Default outputs
        is_valid = 1'b1;
        insn_32 = 32'h00000013;  // Default: NOP (ADDI x0, x0, 0)
        
        if (!is_compressed) begin
            // 32-bit instruction: pass through the full 32-bit input
            insn_32 = insn_32_in;
        end else begin
            // Compressed instruction: decompress based on quadrant
            case (quadrant)
                2'b00: decompress_quadrant_0();
                2'b01: decompress_quadrant_1();
                2'b10: decompress_quadrant_2();
                default: is_valid = 1'b0;
            endcase
        end
    end
    
    // ============================================================
    // Quadrant 0: C.ADDI4SPN, C.LW, C.SW
    // ============================================================
    task automatic decompress_quadrant_0();
        logic [9:0] nzuimm_addi4spn;
        logic [6:0] uimm_lw_sw;
        
        case (funct3)
            3'b000: begin  // C.ADDI4SPN
                // Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
                // Expands to: addi rd', x2, nzuimm
                nzuimm_addi4spn = {insn_16[10:7], insn_16[12:11], insn_16[5], insn_16[6], 2'b00};
                
                if (nzuimm_addi4spn == 10'b0) begin
                    is_valid = 1'b0;  // nzuimm must be non-zero
                end else begin
                    // ADDI rd', x2, nzuimm
                    insn_32 = {22'b0, nzuimm_addi4spn, 5'd2, 3'b000, rd_full, 7'b0010011};
                end
            end
            
            3'b010: begin  // C.LW
                // Format: 010 uimm[5:3] rs1' uimm[2|6] rd' 00
                // Bit positions: insn[12:10]=uimm[5:3], insn[6]=uimm[2], insn[5]=uimm[6]
                // Full uimm = {uimm[6:2], 2'b00} for word-aligned offset
                uimm_lw_sw = {insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                
                // LW rd', offset(rs1')
                insn_32 = {5'b0, uimm_lw_sw, rs1_full, 3'b010, rd_full, 7'b0000011};
            end
            
            3'b011: begin  // C.FLW (RV32FC)
                // Format: 011 uimm[5:3] rs1' uimm[2|6] rd' 00
                // Same bit positions as C.LW
                uimm_lw_sw = {insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                
                // FLW rd', offset(rs1')
                insn_32 = {5'b0, uimm_lw_sw, rs1_full, 3'b010, rd_full, 7'b0000111};
            end
            
            3'b110: begin  // C.SW
                // Format: 110 uimm[5:3] rs1' uimm[2|6] rs2' 00
                // Same bit mapping as C.LW: insn[5]=uimm[6], insn[12:10]=uimm[5:3], insn[6]=uimm[2]
                uimm_lw_sw = {insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                
                // SW rs2', offset(rs1')
                insn_32 = {5'b0, uimm_lw_sw[6:5], rs2_full, rs1_full, 3'b010, uimm_lw_sw[4:0], 7'b0100011};
            end
            
            3'b111: begin  // C.FSW (RV32FC)
                // Format: 111 uimm[5:3] rs1' uimm[2|6] rs2' 00
                // Same bit positions as C.SW
                uimm_lw_sw = {insn_16[5], insn_16[12:10], insn_16[6], 2'b00};
                
                // FSW rs2', offset(rs1')
                insn_32 = {5'b0, uimm_lw_sw[6:5], rs2_full, rs1_full, 3'b010, uimm_lw_sw[4:0], 7'b0100111};
            end
            
            default: is_valid = 1'b0;  // Reserved/illegal
        endcase
    endtask
    
    // ============================================================
    // Quadrant 1: Arithmetic, branches, jumps
    // ============================================================
    task automatic decompress_quadrant_1();
        logic [4:0] rd_rs1;
        logic [5:0] imm;
        logic [11:0] imm_j;
        logic [8:0] imm_b;
        logic [5:0] nzimm_addi;
        logic [9:0] nzimm_addi16sp;
        logic [5:0] nzimm_lui;  // 6-bit immediate for C.LUI
        logic [4:0] shamt;
        
        rd_rs1 = insn_16[11:7];  // Full 5-bit rd/rs1 for some instructions
        imm = {insn_16[12], insn_16[6:2]};  // 6-bit immediate
        
        case (funct3)
            3'b000: begin  // C.NOP / C.ADDI
                if (rd_rs1 == 5'b0 && imm == 6'b0) begin
                    // C.NOP
                    insn_32 = 32'h00000013;  // ADDI x0, x0, 0
                end else begin
                    // C.ADDI
                    nzimm_addi = imm;
                    // Sign-extend 6-bit immediate to 12 bits for ADDI
                    insn_32 = {{6{nzimm_addi[5]}}, nzimm_addi, rd_rs1, 3'b000, rd_rs1, 7'b0010011};
                end
            end
            
            3'b001: begin  // C.JAL (RV32 only)
                // Format: 001 imm[11|4|9:8|10|6|7|3:1|5] 01
                // Expands to: jal x1, offset
                imm_j = {insn_16[12], insn_16[8], insn_16[10:9], insn_16[6], 
                         insn_16[7], insn_16[2], insn_16[11], insn_16[5:3], 1'b0};
                
                // JAL x1, offset
                // J-type format: {imm[20], imm[10:1], imm[11], imm[19:12], rd, opcode}
                insn_32 = {imm_j[11],           // inst[31] = imm[20] (sign bit)
                           imm_j[10:1],         // inst[30:21] = imm[10:1]
                           imm_j[11],           // inst[20] = imm[11]
                           {8{imm_j[11]}},      // inst[19:12] = imm[19:12] (sign extension)
                           5'd1,                // rd = x1
                           7'b1101111};         // JAL opcode
            end
            
            3'b010: begin  // C.LI
                // Expands to: addi rd, x0, imm
                // ADDI format: {imm[11:0], rs1[4:0], funct3[2:0], rd[4:0], opcode[6:0]}
                // Sign-extend 6-bit imm to 12 bits
                insn_32 = {{6{imm[5]}}, imm, 5'b0, 3'b000, rd_rs1, 7'b0010011};
            end
            
            3'b011: begin  // C.ADDI16SP / C.LUI
                if (rd_rs1 == 5'd2) begin
                    // C.ADDI16SP
                    nzimm_addi16sp = {insn_16[12], insn_16[4:3], insn_16[5], insn_16[2], insn_16[6], 4'b0};
                    
                    if (nzimm_addi16sp == 10'b0) begin
                        is_valid = 1'b0;
                    end else begin
                        // ADDI x2, x2, nzimm
                        insn_32 = {{22{nzimm_addi16sp[9]}}, nzimm_addi16sp, 5'd2, 3'b000, 5'd2, 7'b0010011};
                    end
                end else begin
                    // C.LUI: Extract 6-bit immediate and construct 20-bit immediate for LUI
                    nzimm_lui = {insn_16[12], insn_16[6:2]};
                    
                    if (rd_rs1 == 5'b0 || nzimm_lui == 6'b0) begin
                        is_valid = 1'b0;
                    end else begin
                        // LUI rd, {sign_extend(nzimm_lui), 12'b0}
                        // Create 20-bit immediate: sign-extend 6-bit nzimm_lui to 20 bits
                        insn_32 = {{{14{nzimm_lui[5]}}, nzimm_lui}, rd_rs1, 7'b0110111};
                    end
                end
            end
            
            3'b100: begin  // C.SRLI, C.SRAI, C.ANDI, C.SUB, C.XOR, C.OR, C.AND
                logic [1:0] funct2;
                logic [1:0] funct2_ca;
                
                funct2 = insn_16[11:10];
                funct2_ca = insn_16[6:5];
                shamt = insn_16[6:2];
                
                case (funct2)
                    2'b00: begin  // C.SRLI
                        if (shamt == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // SRLI rd', rd', shamt
                            insn_32 = {7'b0000000, shamt, rs1_full, 3'b101, rs1_full, 7'b0010011};
                        end
                    end
                    
                    2'b01: begin  // C.SRAI
                        if (shamt == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // SRAI rd', rd', shamt
                            insn_32 = {7'b0100000, shamt, rs1_full, 3'b101, rs1_full, 7'b0010011};
                        end
                    end
                    
                    2'b10: begin  // C.ANDI
                        // ANDI rd', rd', imm
                        // Sign-extend 6-bit immediate to 12 bits for ANDI
                        insn_32 = {{6{imm[5]}}, imm, rs1_full, 3'b111, rs1_full, 7'b0010011};
                    end
                    
                    2'b11: begin  // C.SUB, C.XOR, C.OR, C.AND
                        if (insn_16[12] == 1'b0) begin
                            case (funct2_ca)
                                2'b00: begin  // C.SUB
                                    insn_32 = {7'b0100000, rs2_full, rs1_full, 3'b000, rs1_full, 7'b0110011};
                                end
                                2'b01: begin  // C.XOR
                                    insn_32 = {7'b0000000, rs2_full, rs1_full, 3'b100, rs1_full, 7'b0110011};
                                end
                                2'b10: begin  // C.OR
                                    insn_32 = {7'b0000000, rs2_full, rs1_full, 3'b110, rs1_full, 7'b0110011};
                                end
                                2'b11: begin  // C.AND
                                    insn_32 = {7'b0000000, rs2_full, rs1_full, 3'b111, rs1_full, 7'b0110011};
                                end
                            endcase
                        end else begin
                            is_valid = 1'b0;  // Reserved for RV64/RV128
                        end
                    end
                endcase
            end
            
            3'b101: begin  // C.J
                // Format: 101 imm[11|4|9:8|10|6|7|3:1|5] 01
                // Expands to: jal x0, offset
                // Extract 12-bit immediate from compressed instruction
                imm_j = {insn_16[12], insn_16[8], insn_16[10:9], insn_16[6], 
                         insn_16[7], insn_16[2], insn_16[11], insn_16[5:3], 1'b0};
                
                // JAL x0, offset
                // J-type format: {imm[20], imm[10:1], imm[11], imm[19:12], rd, opcode}
                // Sign-extend 12-bit imm_j to 21 bits, then place in scrambled J-type order
                insn_32 = {imm_j[11],           // inst[31] = imm[20] (sign bit, extended from imm[11])
                           imm_j[10:1],         // inst[30:21] = imm[10:1]
                           imm_j[11],           // inst[20] = imm[11]
                           {8{imm_j[11]}},      // inst[19:12] = imm[19:12] (sign extension)
                           5'b0,                // rd = x0
                           7'b1101111};         // JAL opcode
            end
            
            3'b110: begin  // C.BEQZ
                // Format: 110 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
                // Expands to: beq rs1', x0, offset
                // Extract bits: offset = {offset[8], offset[7:6], offset[5], offset[4:3], offset[2:1], 1'b0}
                imm_b = {insn_16[12], insn_16[6:5], insn_16[2], insn_16[11:10], insn_16[4:3], 1'b0};
                
                // BEQ rs1', x0, offset
                // B-type encoding needs 13-bit immediate: imm[12:0] where imm[0]=0
                // Sign-extend 9-bit imm_b to 13 bits
                // inst[31] = imm[12], inst[30:25] = imm[10:5], inst[11:8] = imm[4:1], inst[7] = imm[11]
                insn_32 = {imm_b[8],         // bit 31 (imm[12] - sign bit)
                           imm_b[8],         // bit 30 (imm[10])  
                           imm_b[8],         // bit 29 (imm[9])
                           imm_b[8],         // bit 28 (imm[8])
                           imm_b[7:5],       // bits 27:25 (imm[7:5])
                           5'b0,             // rs2 = x0
                           rs1_full,         // rs1
                           3'b000,           // funct3 = BEQ
                           imm_b[4:1],       // bits 11:8 (imm[4:1])
                           imm_b[8],         // bit 7 (imm[11])
                           7'b1100011};      // BRANCH opcode
            end
            
            3'b111: begin  // C.BNEZ
                // Format: 111 offset[8|4:3] rs1' offset[7:6|2:1|5] 01
                // Expands to: bne rs1', x0, offset
                imm_b = {insn_16[12], insn_16[6:5], insn_16[2], insn_16[11:10], insn_16[4:3], 1'b0};
                
                // BNE rs1', x0, offset  
                insn_32 = {imm_b[8],         // bit 31 (imm[12] - sign bit)
                           imm_b[8],         // bit 30 (imm[10])
                           imm_b[8],         // bit 29 (imm[9])
                           imm_b[8],         // bit 28 (imm[8])
                           imm_b[7:5],       // bits 27:25 (imm[7:5])
                           5'b0,             // rs2 = x0
                           rs1_full,         // rs1
                           3'b001,           // funct3 = BNE
                           imm_b[4:1],       // bits 11:8 (imm[4:1])
                           imm_b[8],         // bit 7 (imm[11])
                           7'b1100011};      // BRANCH opcode
            end
        endcase
    endtask
    
    // ============================================================
    // Quadrant 2: Shifts, loads/stores, jumps
    // ============================================================
    task automatic decompress_quadrant_2();
        logic [4:0] rd_rs1;
        logic [4:0] rs2;
        logic [5:0] shamt;
        logic [7:0] uimm_lwsp;
        logic [7:0] uimm_swsp;
        
        rd_rs1 = insn_16[11:7];
        rs2 = insn_16[6:2];
        shamt = {insn_16[12], insn_16[6:2]};
        
        case (funct3)
            3'b000: begin  // C.SLLI
                if (shamt == 6'b0 || rd_rs1 == 5'b0) begin
                    is_valid = 1'b0;
                end else begin
                    // SLLI rd, rd, shamt
                    insn_32 = {7'b0000000, shamt[4:0], rd_rs1, 3'b001, rd_rs1, 7'b0010011};
                end
            end
            
            3'b010: begin  // C.LWSP
                // Format: 010 uimm[5] rd uimm[4:2|7:6] 10
                // Expands to: lw rd, offset(x2)
                uimm_lwsp = {insn_16[3:2], insn_16[12], insn_16[6:4], 2'b00};
                
                if (rd_rs1 == 5'b0) begin
                    is_valid = 1'b0;  // Reserved
                end else begin
                    // LW rd, offset(x2)
                    insn_32 = {4'b0, uimm_lwsp, 5'd2, 3'b010, rd_rs1, 7'b0000011};
                end
            end
            
            3'b011: begin  // C.FLWSP (RV32FC)
                // Format: 011 uimm[5] rd uimm[4:2|7:6] 10
                // Same bit positions as C.LWSP
                uimm_lwsp = {insn_16[3:2], insn_16[12], insn_16[6:4], 2'b00};
                
                // FLW rd, offset(x2)
                insn_32 = {4'b0, uimm_lwsp, 5'd2, 3'b010, rd_rs1, 7'b0000111};
            end
            
            3'b100: begin  // C.JR, C.MV, C.EBREAK, C.JALR, C.ADD
                if (insn_16[12] == 1'b0) begin
                    if (rs2 == 5'b0) begin
                        // C.JR
                        if (rd_rs1 == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // JALR x0, 0(rs1)
                            insn_32 = {12'b0, rd_rs1, 3'b000, 5'b0, 7'b1100111};
                        end
                    end else begin
                        // C.MV
                        if (rd_rs1 == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // ADD rd, x0, rs2
                            insn_32 = {7'b0000000, rs2, 5'b0, 3'b000, rd_rs1, 7'b0110011};
                        end
                    end
                end else begin
                    if (rd_rs1 == 5'b0 && rs2 == 5'b0) begin
                        // C.EBREAK
                        insn_32 = 32'h00100073;
                    end else if (rs2 == 5'b0) begin
                        // C.JALR
                        if (rd_rs1 == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // JALR x1, 0(rs1)
                            insn_32 = {12'b0, rd_rs1, 3'b000, 5'd1, 7'b1100111};
                        end
                    end else begin
                        // C.ADD
                        if (rd_rs1 == 5'b0) begin
                            is_valid = 1'b0;
                        end else begin
                            // ADD rd, rd, rs2
                            insn_32 = {7'b0000000, rs2, rd_rs1, 3'b000, rd_rs1, 7'b0110011};
                        end
                    end
                end
            end
            
            3'b110: begin  // C.SWSP
                // Format: 110 uimm[5:2|7:6] rs2 10
                // Expands to: sw rs2, offset(x2)
                uimm_swsp = {insn_16[8:7], insn_16[12:9], 2'b00};
                
                // SW rs2, offset(x2)
                insn_32 = {4'b0, uimm_swsp[7:5], rs2, 5'd2, 3'b010, uimm_swsp[4:0], 7'b0100011};
            end
            
            3'b111: begin  // C.FSWSP (RV32FC)
                // Format: 111 uimm[5:2|7:6] rs2 10
                // Same bit positions as C.SWSP
                uimm_swsp = {insn_16[8:7], insn_16[12:9], 2'b00};
                
                // FSW rs2, offset(x2)
                insn_32 = {4'b0, uimm_swsp[7:5], rs2, 5'd2, 3'b010, uimm_swsp[4:0], 7'b0100111};
            end
            
            default: is_valid = 1'b0;  // Reserved/illegal
        endcase
    endtask

endmodule
`default_nettype wire

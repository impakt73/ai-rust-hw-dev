// Instruction Decoder Module
// Decodes RISC-V RV32I instructions

module decoder (
    input  logic [31:0] instruction,
    output logic [6:0]  opcode,
    output logic [4:0]  rd,
    output logic [4:0]  rs1,
    output logic [4:0]  rs2,
    output logic [2:0]  funct3,
    output logic [6:0]  funct7,
    output logic [31:0] imm_i,
    output logic [31:0] imm_s,
    output logic [31:0] imm_b,
    output logic [31:0] imm_u,
    output logic [31:0] imm_j,
    output logic [3:0]  alu_op,
    output logic        alu_src,      // 0: rs2, 1: immediate
    output logic        reg_write,
    output logic        mem_write,
    output logic        mem_to_reg,
    output logic        branch,
    output logic        jump
);

    // Extract fields from instruction
    assign opcode = instruction[6:0];
    assign rd     = instruction[11:7];
    assign funct3 = instruction[14:12];
    assign rs1    = instruction[19:15];
    assign rs2    = instruction[24:20];
    assign funct7 = instruction[31:25];

    // Immediate extraction with sign extension
    // I-type (ADDI, LW, etc.)
    assign imm_i = {{20{instruction[31]}}, instruction[31:20]};

    // S-type (SW, etc.)
    assign imm_s = {{20{instruction[31]}}, instruction[31:25], instruction[11:7]};

    // B-type (BEQ, BNE, etc.)
    assign imm_b = {{19{instruction[31]}}, instruction[31], instruction[7], instruction[30:25], instruction[11:8], 1'b0};

    // U-type (LUI, AUIPC)
    assign imm_u = {instruction[31:12], 12'b0};

    // J-type (JAL)
    assign imm_j = {{11{instruction[31]}}, instruction[31], instruction[19:12], instruction[20], instruction[30:21], 1'b0};

    // Opcodes
    localparam logic [6:0] OP_IMM    = 7'b0010011;  // I-type ALU operations
    localparam logic [6:0] OP_REG    = 7'b0110011;  // R-type ALU operations
    localparam logic [6:0] OP_LOAD   = 7'b0000011;  // Load instructions
    localparam logic [6:0] OP_STORE  = 7'b0100011;  // Store instructions
    localparam logic [6:0] OP_BRANCH = 7'b1100011;  // Branch instructions
    localparam logic [6:0] OP_LUI    = 7'b0110111;  // LUI
    localparam logic [6:0] OP_AUIPC  = 7'b0010111;  // AUIPC
    localparam logic [6:0] OP_JAL    = 7'b1101111;  // JAL
    localparam logic [6:0] OP_JALR   = 7'b1100111;  // JALR

    // ALU operations (must match alu.sv)
    localparam logic [3:0] ALU_ADD  = 4'b0000;
    localparam logic [3:0] ALU_SUB  = 4'b0001;
    localparam logic [3:0] ALU_AND  = 4'b0010;
    localparam logic [3:0] ALU_OR   = 4'b0011;
    localparam logic [3:0] ALU_XOR  = 4'b0100;
    localparam logic [3:0] ALU_SLL  = 4'b0101;
    localparam logic [3:0] ALU_SRL  = 4'b0110;
    localparam logic [3:0] ALU_SRA  = 4'b0111;
    localparam logic [3:0] ALU_SLT  = 4'b1000;
    localparam logic [3:0] ALU_SLTU = 4'b1001;

    // Control signals and ALU operation decoding
    always_comb begin
        // Default values
        alu_op = ALU_ADD;
        alu_src = 1'b0;
        reg_write = 1'b0;
        mem_write = 1'b0;
        mem_to_reg = 1'b0;
        branch = 1'b0;
        jump = 1'b0;

        case (opcode)
            OP_IMM: begin
                // I-type ALU operations (ADDI, ANDI, ORI, etc.)
                alu_src = 1'b1;  // Use immediate
                reg_write = 1'b1;
                case (funct3)
                    3'b000: alu_op = ALU_ADD;   // ADDI
                    3'b111: alu_op = ALU_AND;   // ANDI
                    3'b110: alu_op = ALU_OR;    // ORI
                    3'b100: alu_op = ALU_XOR;   // XORI
                    3'b001: alu_op = ALU_SLL;   // SLLI
                    3'b101: alu_op = (funct7[5]) ? ALU_SRA : ALU_SRL;  // SRAI/SRLI
                    3'b010: alu_op = ALU_SLT;   // SLTI
                    3'b011: alu_op = ALU_SLTU;  // SLTIU
                    default: alu_op = ALU_ADD;
                endcase
            end

            OP_REG: begin
                // R-type ALU operations (ADD, SUB, AND, OR, etc.)
                alu_src = 1'b0;  // Use rs2
                reg_write = 1'b1;
                case (funct3)
                    3'b000: alu_op = (funct7[5]) ? ALU_SUB : ALU_ADD;  // SUB/ADD
                    3'b111: alu_op = ALU_AND;   // AND
                    3'b110: alu_op = ALU_OR;    // OR
                    3'b100: alu_op = ALU_XOR;   // XOR
                    3'b001: alu_op = ALU_SLL;   // SLL
                    3'b101: alu_op = (funct7[5]) ? ALU_SRA : ALU_SRL;  // SRA/SRL
                    3'b010: alu_op = ALU_SLT;   // SLT
                    3'b011: alu_op = ALU_SLTU;  // SLTU
                    default: alu_op = ALU_ADD;
                endcase
            end

            OP_LOAD: begin
                // Load instructions (LW, LH, LB, etc.)
                alu_op = ALU_ADD;  // Calculate address
                alu_src = 1'b1;    // Use immediate offset
                reg_write = 1'b1;
                mem_to_reg = 1'b1;
            end

            OP_STORE: begin
                // Store instructions (SW, SH, SB, etc.)
                alu_op = ALU_ADD;  // Calculate address
                alu_src = 1'b1;    // Use immediate offset
                mem_write = 1'b1;
            end

            OP_BRANCH: begin
                // Branch instructions (BEQ, BNE, etc.)
                alu_op = ALU_SUB;  // Compare by subtraction
                alu_src = 1'b0;    // Use rs2
                branch = 1'b1;
            end

            OP_LUI: begin
                // LUI - Load Upper Immediate
                alu_op = ALU_ADD;
                alu_src = 1'b1;
                reg_write = 1'b1;
            end

            OP_AUIPC: begin
                // AUIPC - Add Upper Immediate to PC
                alu_op = ALU_ADD;
                alu_src = 1'b1;
                reg_write = 1'b1;
            end

            OP_JAL: begin
                // JAL - Jump and Link
                jump = 1'b1;
                reg_write = 1'b1;
            end

            OP_JALR: begin
                // JALR - Jump and Link Register
                jump = 1'b1;
                alu_src = 1'b1;
                reg_write = 1'b1;
            end

            default: begin
                // NOP or invalid instruction
                alu_op = ALU_ADD;
                alu_src = 1'b0;
                reg_write = 1'b0;
            end
        endcase
    end

endmodule

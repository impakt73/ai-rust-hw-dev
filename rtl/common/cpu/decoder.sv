`default_nettype none
// Instruction Decoder Module
// Decodes RISC-V RV32IMACF instructions
// Configurable extension support for resource-constrained FPGA targets

module decoder #(
    parameter ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point (default: enabled)
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        decode_en,
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
    output logic [4:0]  alu_op,
    output logic        alu_src,      // 0: rs2, 1: immediate
    output logic        reg_write,
    output logic        mem_write,
    output logic        mem_read,
    output logic        mem_to_reg,
    output logic        branch,
    output logic        jump,
    output logic        is_ecall,     // ECALL instruction
    output logic        is_ebreak,    // EBREAK instruction
    output logic        is_fence,     // FENCE instruction
    output logic        is_csr,       // CSR instruction
    output logic        is_auipc,     // AUIPC instruction
    output logic        is_lr,        // LR.W instruction (A extension)
    output logic        is_sc,        // SC.W instruction (A extension)
    output logic        is_amo,       // AMO instruction (A extension)
    output logic [4:0]  funct5,       // For atomic operation type
    // F extension outputs
    output logic [4:0]  fpu_op,       // FPU operation selector
    output logic        fp_reg_write, // FP register write enable
    output logic        fp_to_int,    // FP result goes to integer register
    output logic        int_to_fp,    // Integer source goes to FP unit
    output logic        is_fp_load,   // FLW instruction
    output logic        is_fp_store,  // FSW instruction
    output logic        instruction_valid  // Instruction is valid/recognized
);

    logic [6:0]  opcode_dec;
    logic [4:0]  rd_dec;
    logic [4:0]  rs1_dec;
    logic [4:0]  rs2_dec;
    logic [2:0]  funct3_dec;
    logic [6:0]  funct7_dec;
    logic [31:0] imm_i_dec;
    logic [31:0] imm_s_dec;
    logic [31:0] imm_b_dec;
    logic [31:0] imm_u_dec;
    logic [31:0] imm_j_dec;
    logic [4:0]  alu_op_dec;
    logic        alu_src_dec;
    logic        reg_write_dec;
    logic        mem_write_dec;
    logic        mem_read_dec;
    logic        mem_to_reg_dec;
    logic        branch_dec;
    logic        jump_dec;
    logic        is_ecall_dec;
    logic        is_ebreak_dec;
    logic        is_fence_dec;
    logic        is_csr_dec;
    logic        is_auipc_dec;
    logic        is_lr_dec;
    logic        is_sc_dec;
    logic        is_amo_dec;
    logic [4:0]  funct5_dec;
    logic [4:0]  fpu_op_dec;
    logic        fp_reg_write_dec;
    logic        fp_to_int_dec;
    logic        int_to_fp_dec;
    logic        is_fp_load_dec;
    logic        is_fp_store_dec;
    logic        instruction_valid_dec;

    // Extract fields from instruction
    assign opcode_dec = instruction[6:0];
    assign rd_dec     = instruction[11:7];
    assign funct3_dec = instruction[14:12];
    assign rs1_dec    = instruction[19:15];
    assign rs2_dec    = instruction[24:20];
    assign funct7_dec = instruction[31:25];
    assign funct5_dec = instruction[31:27];  // For atomic operations

    // Immediate extraction with sign extension
    // I-type (ADDI, LW, etc.)
    assign imm_i_dec = {{20{instruction[31]}}, instruction[31:20]};

    // S-type (SW, etc.)
    assign imm_s_dec = {{20{instruction[31]}}, instruction[31:25], instruction[11:7]};

    // B-type (BEQ, BNE, etc.)
    assign imm_b_dec = {{19{instruction[31]}}, instruction[31], instruction[7], instruction[30:25], instruction[11:8], 1'b0};

    // U-type (LUI, AUIPC)
    assign imm_u_dec = {instruction[31:12], 12'b0};

    // J-type (JAL)
    assign imm_j_dec = {{11{instruction[31]}}, instruction[31], instruction[19:12], instruction[20], instruction[30:21], 1'b0};

    // Opcodes
    localparam logic [6:0] OP_IMM      = 7'b0010011;  // I-type ALU operations
    localparam logic [6:0] OP_REG      = 7'b0110011;  // R-type ALU operations
    localparam logic [6:0] OP_LOAD     = 7'b0000011;  // Load instructions (LW, LH, LB, LHU, LBU)
    localparam logic [6:0] OP_LOAD_FP  = 7'b0000111;  // FP Load (FLW) - bit 2 = 1
    localparam logic [6:0] OP_STORE    = 7'b0100011;  // Store instructions (SW, SH, SB)
    localparam logic [6:0] OP_STORE_FP = 7'b0100111;  // FP Store (FSW) - bit 2 = 1
    localparam logic [6:0] OP_BRANCH   = 7'b1100011;  // Branch instructions
    localparam logic [6:0] OP_LUI      = 7'b0110111;  // LUI
    localparam logic [6:0] OP_AUIPC    = 7'b0010111;  // AUIPC
    localparam logic [6:0] OP_JAL      = 7'b1101111;  // JAL
    localparam logic [6:0] OP_JALR     = 7'b1100111;  // JALR
    localparam logic [6:0] OP_FENCE    = 7'b0001111;  // FENCE
    localparam logic [6:0] OP_SYSTEM   = 7'b1110011;  // SYSTEM (ECALL, EBREAK, CSR*)
    localparam logic [6:0] OP_AMO      = 7'b0101111;  // Atomic operations (A extension)
    // F extension opcodes
    localparam logic [6:0] OP_FP       = 7'b1010011;  // FP computational
    localparam logic [6:0] OP_FMADD    = 7'b1000011;  // Fused multiply-add
    localparam logic [6:0] OP_FMSUB    = 7'b1000111;  // Fused multiply-sub
    localparam logic [6:0] OP_FNMSUB   = 7'b1001011;  // Fused negate-multiply-sub
    localparam logic [6:0] OP_FNMADD   = 7'b1001111;  // Fused negate-multiply-add

    // ALU operations (must match alu.sv)
    localparam logic [4:0] ALU_ADD  = 5'b00000;
    localparam logic [4:0] ALU_SUB  = 5'b00001;
    localparam logic [4:0] ALU_AND  = 5'b00010;
    localparam logic [4:0] ALU_OR   = 5'b00011;
    localparam logic [4:0] ALU_XOR  = 5'b00100;
    localparam logic [4:0] ALU_SLL  = 5'b00101;
    localparam logic [4:0] ALU_SRL  = 5'b00110;
    localparam logic [4:0] ALU_SRA  = 5'b00111;
    localparam logic [4:0] ALU_SLT  = 5'b01000;
    localparam logic [4:0] ALU_SLTU = 5'b01001;
    
    // M Extension operations
    localparam logic [4:0] ALU_MUL    = 5'b01010;
    localparam logic [4:0] ALU_MULH   = 5'b01011;
    localparam logic [4:0] ALU_MULHSU = 5'b01100;
    localparam logic [4:0] ALU_MULHU  = 5'b01101;
    localparam logic [4:0] ALU_DIV    = 5'b01110;
    localparam logic [4:0] ALU_DIVU   = 5'b01111;
    localparam logic [4:0] ALU_REM    = 5'b10000;
    localparam logic [4:0] ALU_REMU   = 5'b10001;
    
    // A Extension operations (MIN/MAX for atomic instructions)
    localparam logic [4:0] ALU_MIN    = 5'b10010;
    localparam logic [4:0] ALU_MAX    = 5'b10011;
    localparam logic [4:0] ALU_MINU   = 5'b10100;
    localparam logic [4:0] ALU_MAXU   = 5'b10101;
    
    // F Extension FPU operations (MUST match fpu.sv encodings exactly)
    localparam logic [4:0] FPU_ADD    = 5'b00000;  // FADD.S
    localparam logic [4:0] FPU_SUB    = 5'b00001;  // FSUB.S
    localparam logic [4:0] FPU_MUL    = 5'b00010;  // FMUL.S
    localparam logic [4:0] FPU_DIV    = 5'b00011;  // FDIV.S
    localparam logic [4:0] FPU_SQRT   = 5'b00100;  // FSQRT.S
    localparam logic [4:0] FPU_MIN    = 5'b00101;  // FMIN.S
    localparam logic [4:0] FPU_MAX    = 5'b00110;  // FMAX.S
    localparam logic [4:0] FPU_MADD   = 5'b00111;  // FMADD.S
    localparam logic [4:0] FPU_MSUB   = 5'b01000;  // FMSUB.S
    localparam logic [4:0] FPU_NMSUB  = 5'b01001;  // FNMSUB.S
    localparam logic [4:0] FPU_NMADD  = 5'b01010;  // FNMADD.S
    localparam logic [4:0] FPU_SGNJ   = 5'b01011;  // FSGNJ.S
    localparam logic [4:0] FPU_SGNJN  = 5'b01100;  // FSGNJN.S
    localparam logic [4:0] FPU_SGNJX  = 5'b01101;  // FSGNJX.S
    localparam logic [4:0] FPU_CVTWS  = 5'b01110;  // FCVT.W.S
    localparam logic [4:0] FPU_CVTWUS = 5'b01111;  // FCVT.WU.S
    localparam logic [4:0] FPU_CVTSW  = 5'b10000;  // FCVT.S.W
    localparam logic [4:0] FPU_CVTSWU = 5'b10001;  // FCVT.S.WU
    localparam logic [4:0] FPU_FEQ    = 5'b10010;  // FEQ.S
    localparam logic [4:0] FPU_FLT    = 5'b10011;  // FLT.S
    localparam logic [4:0] FPU_FLE    = 5'b10100;  // FLE.S
    localparam logic [4:0] FPU_FCLASS = 5'b10101;  // FCLASS.S
    localparam logic [4:0] FPU_MVXW   = 5'b10110;  // FMV.X.W
    localparam logic [4:0] FPU_MVWX   = 5'b10111;  // FMV.W.X

    // Control signals and ALU operation decoding
    always_comb begin
        // Default values
        alu_op_dec = ALU_ADD;
        alu_src_dec = 1'b0;
        reg_write_dec = 1'b0;
        mem_write_dec = 1'b0;
        mem_read_dec = 1'b0;
        mem_to_reg_dec = 1'b0;
        branch_dec = 1'b0;
        jump_dec = 1'b0;
        is_ecall_dec = 1'b0;
        is_ebreak_dec = 1'b0;
        is_fence_dec = 1'b0;
        is_csr_dec = 1'b0;
        is_auipc_dec = 1'b0;
        is_lr_dec = 1'b0;
        is_sc_dec = 1'b0;
        is_amo_dec = 1'b0;
        // F extension defaults
        fpu_op_dec = FPU_ADD;
        fp_reg_write_dec = 1'b0;
        fp_to_int_dec = 1'b0;
        int_to_fp_dec = 1'b0;
        is_fp_load_dec = 1'b0;
        is_fp_store_dec = 1'b0;
        // Instruction validity (valid until proven otherwise in default case)
        instruction_valid_dec = 1'b1;

        case (opcode_dec)
            OP_IMM: begin
                // I-type ALU operations (ADDI, ANDI, ORI, etc.)
                alu_src_dec = 1'b1;  // Use immediate
                reg_write_dec = 1'b1;
                case (funct3_dec)
                    3'b000: alu_op_dec = ALU_ADD;   // ADDI
                    3'b111: alu_op_dec = ALU_AND;   // ANDI
                    3'b110: alu_op_dec = ALU_OR;    // ORI
                    3'b100: alu_op_dec = ALU_XOR;   // XORI
                    3'b001: alu_op_dec = ALU_SLL;   // SLLI
                    3'b101: alu_op_dec = (funct7_dec[5]) ? ALU_SRA : ALU_SRL;  // SRAI/SRLI
                    3'b010: alu_op_dec = ALU_SLT;   // SLTI
                    3'b011: alu_op_dec = ALU_SLTU;  // SLTIU
                    default: alu_op_dec = ALU_ADD;
                endcase
            end

            OP_REG: begin
                // R-type ALU operations (ADD, SUB, AND, OR, etc.) and M extension
                alu_src_dec = 1'b0;  // Use rs2_dec
                reg_write_dec = 1'b1;
                
                // Check for M extension (funct7_dec = 0000001)
                if (funct7_dec == 7'b0000001) begin
                    // M extension instructions (multiplication and division)
                    if (ENABLE_M_EXT) begin
                        case (funct3_dec)
                            3'b000: alu_op_dec = ALU_MUL;     // MUL
                            3'b001: alu_op_dec = ALU_MULH;    // MULH
                            3'b010: alu_op_dec = ALU_MULHSU;  // MULHSU
                            3'b011: alu_op_dec = ALU_MULHU;   // MULHU
                            3'b100: alu_op_dec = ALU_DIV;     // DIV
                            3'b101: alu_op_dec = ALU_DIVU;    // DIVU
                            3'b110: alu_op_dec = ALU_REM;     // REM
                            3'b111: alu_op_dec = ALU_REMU;    // REMU
                            default: alu_op_dec = ALU_ADD;
                        endcase
                    end else begin
                        // M extension disabled - treat as invalid/NOP
                        alu_op_dec = ALU_ADD;
                        alu_src_dec = 1'b0;
                        reg_write_dec = 1'b0;
                    end
                end else begin
                    // Standard RV32I R-type instructions
                    case (funct3_dec)
                        3'b000: alu_op_dec = (funct7_dec[5]) ? ALU_SUB : ALU_ADD;  // SUB/ADD
                        3'b111: alu_op_dec = ALU_AND;   // AND
                        3'b110: alu_op_dec = ALU_OR;    // OR
                        3'b100: alu_op_dec = ALU_XOR;   // XOR
                        3'b001: alu_op_dec = ALU_SLL;   // SLL
                        3'b101: alu_op_dec = (funct7_dec[5]) ? ALU_SRA : ALU_SRL;  // SRA/SRL
                        3'b010: alu_op_dec = ALU_SLT;   // SLT
                        3'b011: alu_op_dec = ALU_SLTU;  // SLTU
                        default: alu_op_dec = ALU_ADD;
                    endcase
                end
            end

            OP_LOAD: begin
                // Integer load instructions (LW, LH, LB, LHU, LBU)
                alu_op_dec = ALU_ADD;  // Calculate address
                alu_src_dec = 1'b1;    // Use immediate offset
                mem_read_dec = 1'b1;
                reg_write_dec = 1'b1;
                mem_to_reg_dec = 1'b1;
            end

            OP_LOAD_FP: begin
                // FP load instruction (FLW)
                // Opcode 0b0000111 (bit 2 = 1 distinguishes from OP_LOAD)
                if (ENABLE_F_EXT) begin
                    alu_op_dec = ALU_ADD;  // Calculate address
                    alu_src_dec = 1'b1;    // Use immediate offset
                    mem_read_dec = 1'b1;
                    is_fp_load_dec = 1'b1;
                    fp_reg_write_dec = 1'b1;  // Write to FP register file
                    // Note: funct3_dec is always 010 for FLW (word-sized FP load)
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end

            OP_STORE: begin
                // Integer store instructions (SW, SH, SB)
                alu_op_dec = ALU_ADD;  // Calculate address
                alu_src_dec = 1'b1;    // Use immediate offset
                mem_write_dec = 1'b1;
            end

            OP_STORE_FP: begin
                // FP store instruction (FSW)
                // Opcode 0b0100111 (bit 2 = 1 distinguishes from OP_STORE)
                if (ENABLE_F_EXT) begin
                    alu_op_dec = ALU_ADD;  // Calculate address
                    alu_src_dec = 1'b1;    // Use immediate offset
                    mem_write_dec = 1'b1;
                    is_fp_store_dec = 1'b1;
                    // Note: funct3_dec is always 010 for FSW (word-sized FP store)
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end

            OP_BRANCH: begin
                // Branch instructions (BEQ, BNE, etc.)
                alu_op_dec = ALU_SUB;  // Compare by subtraction
                alu_src_dec = 1'b0;    // Use rs2_dec
                branch_dec = 1'b1;
            end

            OP_LUI: begin
                // LUI - Load Upper Immediate
                alu_op_dec = ALU_ADD;
                alu_src_dec = 1'b1;
                reg_write_dec = 1'b1;
            end

            OP_AUIPC: begin
                // AUIPC - Add Upper Immediate to PC
                alu_op_dec = ALU_ADD;
                alu_src_dec = 1'b1;
                reg_write_dec = 1'b1;
                is_auipc_dec = 1'b1;
            end

            OP_JAL: begin
                // JAL - Jump and Link
                jump_dec = 1'b1;
                reg_write_dec = 1'b1;
            end

            OP_JALR: begin
                // JALR - Jump and Link Register
                jump_dec = 1'b1;
                alu_src_dec = 1'b1;
                reg_write_dec = 1'b1;
            end

            OP_FENCE: begin
                // FENCE - Memory ordering (NOP for single-cycle CPU)
                is_fence_dec = 1'b1;
            end

            OP_SYSTEM: begin
                // SYSTEM instructions: ECALL, EBREAK, CSR*
                if (funct3_dec == 3'b000) begin
                    // ECALL or EBREAK (distinguished by imm[0])
                    if (imm_i_dec[0] == 1'b0) begin
                        is_ecall_dec = 1'b1;
                    end else begin
                        is_ebreak_dec = 1'b1;
                    end
                end else begin
                    // CSR instructions (CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI)
                    is_csr_dec = 1'b1;
                    reg_write_dec = 1'b1;  // CSR instructions write to rd_dec
                end
            end
            
            OP_AMO: begin
                // Atomic operations (A extension)
                // All atomic operations (except SC) read from memory
                alu_op_dec = ALU_ADD;     // Default for address calculation
                alu_src_dec = 1'b0;       // Use rs1_dec for base address
                reg_write_dec = 1'b1;     // All atomics write to rd_dec
                mem_read_dec = 1'b1;      // Default: all atomics read (overridden for SC)
                
                // Decode specific atomic operation based on funct5_dec
                case (funct5_dec)
                    5'b00010: begin   // LR.W
                        is_lr_dec = 1'b1;
                        mem_write_dec = 1'b0;  // LR only reads
                    end
                    5'b00011: begin   // SC.W
                        is_sc_dec = 1'b1;
                        mem_read_dec = 1'b0;   // SC doesn't read - it checks reservation
                        mem_write_dec = 1'b1;  // SC conditionally writes
                    end
                    5'b00001: begin   // AMOSWAP.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        // SWAP doesn't need ALU operation (direct data path)
                    end
                    5'b00000: begin   // AMOADD.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_ADD;
                    end
                    5'b00100: begin   // AMOXOR.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_XOR;
                    end
                    5'b01100: begin   // AMOAND.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_AND;
                    end
                    5'b01000: begin   // AMOOR.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_OR;
                    end
                    5'b10000: begin   // AMOMIN.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_MIN;
                    end
                    5'b10100: begin   // AMOMAX.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_MAX;
                    end
                    5'b11000: begin   // AMOMINU.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_MINU;
                    end
                    5'b11100: begin   // AMOMAXU.W
                        is_amo_dec = 1'b1;
                        mem_write_dec = 1'b1;
                        alu_op_dec = ALU_MAXU;
                    end
                    default: begin    // Unknown atomic operation - treat as NOP
                        is_amo_dec = 1'b0;
                        mem_write_dec = 1'b0;
                        mem_read_dec = 1'b0;
                        reg_write_dec = 1'b0;
                    end
                endcase
            end
            
            OP_FP: begin
                // FP computational instructions
                if (ENABLE_F_EXT) begin
                    case (funct7_dec)
                        7'b0000000: begin  // FADD.S
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = FPU_ADD;
                        end
                        7'b0000100: begin  // FSUB.S
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = FPU_SUB;
                        end
                        7'b0001000: begin  // FMUL.S
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = FPU_MUL;
                        end
                        7'b0001100: begin  // FDIV.S
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = FPU_DIV;
                        end
                        7'b0101100: begin  // FSQRT.S
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = FPU_SQRT;
                        end
                        7'b0010000: begin  // Sign injection (FSGNJ, FSGNJN, FSGNJX)
                            fp_reg_write_dec = 1'b1;
                            case (funct3_dec)
                                3'b000: fpu_op_dec = FPU_SGNJ;   // FSGNJ.S
                                3'b001: fpu_op_dec = FPU_SGNJN;  // FSGNJN.S
                                3'b010: fpu_op_dec = FPU_SGNJX;  // FSGNJX.S
                                default: fpu_op_dec = FPU_SGNJ;
                            endcase
                        end
                        7'b0010100: begin  // MIN/MAX
                            fp_reg_write_dec = 1'b1;
                            fpu_op_dec = (funct3_dec == 3'b000) ? FPU_MIN : FPU_MAX;
                        end
                        7'b1010000: begin  // Comparisons (FLE, FLT, FEQ)
                            fp_reg_write_dec = 1'b0;
                            reg_write_dec = 1'b1;  // Write to integer register
                            fp_to_int_dec = 1'b1;
                            case (funct3_dec)
                                3'b000: fpu_op_dec = FPU_FLE;  // FLE.S
                                3'b001: fpu_op_dec = FPU_FLT;  // FLT.S
                                3'b010: fpu_op_dec = FPU_FEQ;  // FEQ.S
                                default: fpu_op_dec = FPU_FEQ;
                            endcase
                        end
                        7'b1100000: begin  // FCVT.W.S, FCVT.WU.S
                            fp_reg_write_dec = 1'b0;
                            reg_write_dec = 1'b1;
                            fp_to_int_dec = 1'b1;
                            fpu_op_dec = (rs2_dec == 5'b00000) ? FPU_CVTWS : FPU_CVTWUS;
                        end
                        7'b1101000: begin  // FCVT.S.W, FCVT.S.WU
                            fp_reg_write_dec = 1'b1;
                            int_to_fp_dec = 1'b1;
                            fpu_op_dec = (rs2_dec == 5'b00000) ? FPU_CVTSW : FPU_CVTSWU;
                        end
                        7'b1110000: begin
                            if (funct3_dec == 3'b000) begin
                                // FMV.X.W - Move FP to integer
                                fp_reg_write_dec = 1'b0;
                                reg_write_dec = 1'b1;
                                fp_to_int_dec = 1'b1;
                                fpu_op_dec = FPU_MVXW;
                            end else begin
                                // FCLASS.S - Classify FP number
                                fp_reg_write_dec = 1'b0;
                                reg_write_dec = 1'b1;
                                fp_to_int_dec = 1'b1;
                                fpu_op_dec = FPU_FCLASS;
                            end
                        end
                        7'b1111000: begin  // FMV.W.X - Move integer to FP
                            fp_reg_write_dec = 1'b1;
                            int_to_fp_dec = 1'b1;
                            fpu_op_dec = FPU_MVWX;
                        end
                        default: begin
                            fp_reg_write_dec = 1'b0;
                            fpu_op_dec = FPU_ADD;
                        end
                    endcase
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end
            
            OP_FMADD: begin  // FMADD.S
                if (ENABLE_F_EXT) begin
                    fp_reg_write_dec = 1'b1;
                    fpu_op_dec = FPU_MADD;
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end
            
            OP_FMSUB: begin  // FMSUB.S
                if (ENABLE_F_EXT) begin
                    fp_reg_write_dec = 1'b1;
                    fpu_op_dec = FPU_MSUB;
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end
            
            OP_FNMSUB: begin  // FNMSUB.S
                if (ENABLE_F_EXT) begin
                    fp_reg_write_dec = 1'b1;
                    fpu_op_dec = FPU_NMSUB;
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end
            
            OP_FNMADD: begin  // FNMADD.S
                if (ENABLE_F_EXT) begin
                    fp_reg_write_dec = 1'b1;
                    fpu_op_dec = FPU_NMADD;
                end
                // else: F extension disabled - all signals remain at default (NOP)
            end

            default: begin
                // Invalid instruction - unrecognized opcode_dec
                alu_op_dec = ALU_ADD;
                alu_src_dec = 1'b0;
                reg_write_dec = 1'b0;
                instruction_valid_dec = 1'b0;  // Signal invalid instruction
            end
        endcase
    end

    // Hold the last decoded instruction between fetches so the multi-cycle CPU can
    // consume a stable set of decoder outputs through DECODE/EXECUTE/WRITEBACK.
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            opcode <= 7'h0;
            rd <= 5'h0;
            rs1 <= 5'h0;
            rs2 <= 5'h0;
            funct3 <= 3'h0;
            funct7 <= 7'h0;
            imm_i <= 32'h0;
            imm_s <= 32'h0;
            imm_b <= 32'h0;
            imm_u <= 32'h0;
            imm_j <= 32'h0;
            alu_op <= 5'h0;
            alu_src <= 1'b0;
            reg_write <= 1'b0;
            mem_write <= 1'b0;
            mem_read <= 1'b0;
            mem_to_reg <= 1'b0;
            branch <= 1'b0;
            jump <= 1'b0;
            is_ecall <= 1'b0;
            is_ebreak <= 1'b0;
            is_fence <= 1'b0;
            is_csr <= 1'b0;
            is_auipc <= 1'b0;
            is_lr <= 1'b0;
            is_sc <= 1'b0;
            is_amo <= 1'b0;
            funct5 <= 5'h0;
            fpu_op <= 5'h0;
            fp_reg_write <= 1'b0;
            fp_to_int <= 1'b0;
            int_to_fp <= 1'b0;
            is_fp_load <= 1'b0;
            is_fp_store <= 1'b0;
            instruction_valid <= 1'b0;
        end else if (decode_en) begin
            opcode <= opcode_dec;
            rd <= rd_dec;
            rs1 <= rs1_dec;
            rs2 <= rs2_dec;
            funct3 <= funct3_dec;
            funct7 <= funct7_dec;
            imm_i <= imm_i_dec;
            imm_s <= imm_s_dec;
            imm_b <= imm_b_dec;
            imm_u <= imm_u_dec;
            imm_j <= imm_j_dec;
            alu_op <= alu_op_dec;
            alu_src <= alu_src_dec;
            reg_write <= reg_write_dec;
            mem_write <= mem_write_dec;
            mem_read <= mem_read_dec;
            mem_to_reg <= mem_to_reg_dec;
            branch <= branch_dec;
            jump <= jump_dec;
            is_ecall <= is_ecall_dec;
            is_ebreak <= is_ebreak_dec;
            is_fence <= is_fence_dec;
            is_csr <= is_csr_dec;
            is_auipc <= is_auipc_dec;
            is_lr <= is_lr_dec;
            is_sc <= is_sc_dec;
            is_amo <= is_amo_dec;
            funct5 <= funct5_dec;
            fpu_op <= fpu_op_dec;
            fp_reg_write <= fp_reg_write_dec;
            fp_to_int <= fp_to_int_dec;
            int_to_fp <= int_to_fp_dec;
            is_fp_load <= is_fp_load_dec;
            is_fp_store <= is_fp_store_dec;
            instruction_valid <= instruction_valid_dec;
        end
    end

endmodule
`default_nettype wire

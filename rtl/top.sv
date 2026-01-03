// Top-Level CPU Module
// Multi-cycle RISC-V RV32IM processor with variable-latency memory support

module top (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory interface (exposed to testbench)
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    output logic        imem_req,     // NEW: Request instruction fetch
    input  logic        imem_ready,   // NEW: Memory has valid data
    
    // Data memory interface (exposed to testbench)
    output logic [31:0] dmem_addr,
    output logic [31:0] dmem_wdata,
    input  logic [31:0] dmem_rdata,
    output logic        dmem_we,
    output logic        dmem_re,      // Memory read enable
    output logic [1:0]  dmem_size,    // Memory operation size: 00=byte, 01=halfword, 10=word
    output logic        dmem_req,     // NEW: Request data memory operation
    input  logic        dmem_ready,   // NEW: Memory operation complete
    
    // System control signals
    output logic        halted,       // CPU halted (ECALL/EBREAK)
    output logic        instr_complete, // NEW: High for 1 cycle when instruction done
    
    // Debug outputs (for tracing register values)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);

    // ============================================================
    // FSM State Definitions
    // ============================================================
    typedef enum logic [3:0] {
        S_IDLE       = 4'b0000,  // After reset
        S_FETCH      = 4'b0001,  // Fetch instruction (wait for imem_ready)
        S_DECODE     = 4'b0010,  // Decode and read registers
        S_EXECUTE    = 4'b0011,  // ALU operation
        S_MEM_ADDR   = 4'b0100,  // Calculate memory address
        S_MEM_READ   = 4'b0101,  // Load from memory (wait for dmem_ready)
        S_MEM_WRITE  = 4'b0110,  // Store to memory (wait for dmem_ready)
        S_WRITEBACK  = 4'b0111,  // Write result to register
        S_BRANCH     = 4'b1000,  // Branch decision
        S_CSR        = 4'b1001,  // CSR operation
        S_HALT       = 4'b1010   // ECALL/EBREAK
    } state_t;

    state_t current_state, next_state;

    // ============================================================
    // Internal signals
    // ============================================================
    logic [31:0] pc;
    logic [31:0] instruction;
    
    // Decoder outputs (combinational - will be captured in registers)
    logic [6:0]  opcode;
    logic [4:0]  rd;
    logic [4:0]  rs1;
    logic [4:0]  rs2;
    logic [2:0]  funct3;
    logic [6:0]  funct7;
    logic [31:0] imm_i;
    logic [31:0] imm_s;
    logic [31:0] imm_b;
    logic [31:0] imm_u;
    logic [31:0] imm_j;
    logic [4:0]  alu_op;
    logic        alu_src;
    logic        reg_write;
    logic        mem_write;
    logic        mem_read;
    logic        mem_to_reg;
    logic        branch;
    logic        jump;
    logic        is_ecall;
    logic        is_ebreak;
    logic        is_fence;
    logic        is_csr;
    
    // ============================================================
    // Staging Registers (Flip-Flops for Multi-Cycle Operation)
    // ============================================================
    
    // Instruction Register
    logic [31:0] ir_reg;
    logic ir_write;
    
    // Operand Registers
    logic [31:0] a_reg;  // rs1 data
    logic [31:0] b_reg;  // rs2 data
    logic a_reg_write, b_reg_write;
    
    // Result Registers
    logic [31:0] alu_out_reg;  // ALU output
    logic [31:0] mdr;          // Memory data register
    logic alu_out_write, mdr_write;
    
    // Decoder Output Registers (all control signals stored)
    logic [6:0]  opcode_reg;
    logic [4:0]  rd_reg, rs1_reg, rs2_reg;
    logic [2:0]  funct3_reg;
    logic [6:0]  funct7_reg;
    logic [31:0] imm_i_reg, imm_s_reg, imm_b_reg, imm_u_reg, imm_j_reg;
    logic [4:0]  alu_op_reg;
    logic        alu_src_reg, reg_write_reg, mem_write_reg, mem_read_reg;
    logic        mem_to_reg_reg, branch_reg, jump_reg;
    logic        is_ecall_reg, is_ebreak_reg, is_fence_reg, is_csr_reg;
    logic        decode_reg_write;
    
    // Control Signals
    logic        pc_write;
    logic        reg_write_en;
    
    // Register file signals
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] rd_data;
    
    // ALU signals
    logic [31:0] alu_a;
    logic [31:0] alu_b;
    logic [31:0] alu_result;
    logic        alu_zero;
    
    // Branch/Jump logic
    logic        take_branch;
    
    // CSR signals
    logic [11:0] csr_addr;
    logic [31:0] csr_rdata;
    
    // Memory interface signals
    logic [31:0] formatted_load_data;
    
    assign csr_addr = imm_i_reg[11:0];  // CSR address from registered immediate field
    
    // ============================================================
    // State Register (Flip-Flop Based FSM)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            current_state <= S_IDLE;
        else
            current_state <= next_state;
    end
    
    // ============================================================
    // Staging Register Implementations (All Flip-Flops)
    // ============================================================
    
    // Instruction Register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            ir_reg <= 32'h0;
        else if (ir_write)
            ir_reg <= imem_data;
    end
    
    // Operand Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            a_reg <= 32'h0;
            b_reg <= 32'h0;
        end else begin
            if (a_reg_write) a_reg <= rs1_data;
            if (b_reg_write) b_reg <= rs2_data;
        end
    end
    
    // Result Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            alu_out_reg <= 32'h0;
            mdr <= 32'h0;
        end else begin
            if (alu_out_write) alu_out_reg <= alu_result;
            if (mdr_write) mdr <= formatted_load_data;
        end
    end
    
    // Decoder Output Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            opcode_reg <= 7'h0;
            rd_reg <= 5'h0;
            rs1_reg <= 5'h0;
            rs2_reg <= 5'h0;
            funct3_reg <= 3'h0;
            funct7_reg <= 7'h0;
            imm_i_reg <= 32'h0;
            imm_s_reg <= 32'h0;
            imm_b_reg <= 32'h0;
            imm_u_reg <= 32'h0;
            imm_j_reg <= 32'h0;
            alu_op_reg <= 5'h0;
            alu_src_reg <= 1'b0;
            reg_write_reg <= 1'b0;
            mem_write_reg <= 1'b0;
            mem_read_reg <= 1'b0;
            mem_to_reg_reg <= 1'b0;
            branch_reg <= 1'b0;
            jump_reg <= 1'b0;
            is_ecall_reg <= 1'b0;
            is_ebreak_reg <= 1'b0;
            is_fence_reg <= 1'b0;
            is_csr_reg <= 1'b0;
        end else if (decode_reg_write) begin
            opcode_reg <= opcode;
            rd_reg <= rd;
            rs1_reg <= rs1;
            rs2_reg <= rs2;
            funct3_reg <= funct3;
            funct7_reg <= funct7;
            imm_i_reg <= imm_i;
            imm_s_reg <= imm_s;
            imm_b_reg <= imm_b;
            imm_u_reg <= imm_u;
            imm_j_reg <= imm_j;
            alu_op_reg <= alu_op;
            alu_src_reg <= alu_src;
            reg_write_reg <= reg_write;
            mem_write_reg <= mem_write;
            mem_read_reg <= mem_read;
            mem_to_reg_reg <= mem_to_reg;
            branch_reg <= branch;
            jump_reg <= jump;
            is_ecall_reg <= is_ecall;
            is_ebreak_reg <= is_ebreak;
            is_fence_reg <= is_fence;
            is_csr_reg <= is_csr;
        end
    end
    
    // ============================================================
    // Program Counter with Multi-Cycle Control
    // ============================================================
    logic [31:0] next_pc_value;
    
    always_comb begin
        next_pc_value = pc + 32'd4;  // Default sequential
        
        if (current_state == S_BRANCH) begin
            if (take_branch)
                next_pc_value = pc + imm_b_reg;
        end else if (current_state == S_WRITEBACK) begin
            if (opcode_reg == 7'b1101111)  // JAL
                next_pc_value = pc + imm_j_reg;
            else if (opcode_reg == 7'b1100111)  // JALR
                next_pc_value = (a_reg + imm_i_reg) & ~32'h1;
        end
    end
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            pc <= boot_addr;
        else if (pc_write)
            pc <= next_pc_value;
    end
    
    // Halted signal
    assign halted = (current_state == S_HALT);
    
    // ============================================================
    // Instruction Memory Address Assignment
    // ============================================================
    assign imem_addr = pc;
    assign instruction = ir_reg;  // Use registered instruction
    
    // ============================================================
    // FSM Next-State Logic
    // ============================================================
    always_comb begin
        next_state = current_state;
        
        case (current_state)
            S_IDLE: begin
                next_state = S_FETCH;
            end
            
            S_FETCH: begin
                // Wait for instruction memory ready
                if (imem_ready)
                    next_state = S_DECODE;
                else
                    next_state = S_FETCH;
            end
            
            S_DECODE: begin
                // Use combinational decoder outputs (opcode, not opcode_reg)
                // because decode_reg_write captures them THIS cycle
                case (opcode)
                    7'b0110011,  // R-type
                    7'b0010011,  // I-type arithmetic
                    7'b0110111,  // LUI
                    7'b0010111,  // AUIPC
                    7'b1101111,  // JAL
                    7'b1100111:  // JALR
                        next_state = S_EXECUTE;
                    
                    7'b0000011,  // Load
                    7'b0100011:  // Store
                        next_state = S_MEM_ADDR;
                    
                    7'b1100011:  // Branch
                        next_state = S_BRANCH;
                    
                    7'b1110011: begin  // SYSTEM
                        if (is_ecall || is_ebreak)
                            next_state = S_HALT;
                        else if (is_csr)
                            next_state = S_CSR;
                        else  // FENCE
                            next_state = S_FETCH;
                    end
                    
                    default: next_state = S_FETCH;
                endcase
            end
            
            S_EXECUTE: begin
                next_state = S_WRITEBACK;
            end
            
            S_MEM_ADDR: begin
                if (mem_read_reg)
                    next_state = S_MEM_READ;
                else
                    next_state = S_MEM_WRITE;
            end
            
            S_MEM_READ: begin
                // Wait for data memory ready
                if (dmem_ready)
                    next_state = S_WRITEBACK;
                else
                    next_state = S_MEM_READ;
            end
            
            S_MEM_WRITE: begin
                // Wait for data memory ready
                if (dmem_ready)
                    next_state = S_FETCH;
                else
                    next_state = S_MEM_WRITE;
            end
            
            S_WRITEBACK: begin
                next_state = S_FETCH;
            end
            
            S_BRANCH: begin
                next_state = S_FETCH;
            end
            
            S_CSR: begin
                next_state = S_WRITEBACK;
            end
            
            S_HALT: begin
                next_state = S_HALT;
            end
            
            default: begin
                next_state = S_IDLE;
            end
        endcase
    end
    
    // ============================================================
    // FSM Control Signal Output Logic
    // ============================================================
    always_comb begin
        // Default all control signals to inactive
        ir_write = 1'b0;
        a_reg_write = 1'b0;
        b_reg_write = 1'b0;
        alu_out_write = 1'b0;
        mdr_write = 1'b0;
        pc_write = 1'b0;
        reg_write_en = 1'b0;
        decode_reg_write = 1'b0;
        imem_req = 1'b0;
        dmem_req = 1'b0;
        instr_complete = 1'b0;
        
        case (current_state)
            S_FETCH: begin
                imem_req = 1'b1;
                if (imem_ready)
                    ir_write = 1'b1;
            end
            
            S_DECODE: begin
                a_reg_write = 1'b1;
                b_reg_write = 1'b1;
                decode_reg_write = 1'b1;
                // FENCE completes here
                if (is_fence) begin
                    pc_write = 1'b1;
                    instr_complete = 1'b1;
                end
            end
            
            S_EXECUTE: begin
                alu_out_write = 1'b1;
            end
            
            S_MEM_ADDR: begin
                alu_out_write = 1'b1;
            end
            
            S_MEM_READ: begin
                dmem_req = 1'b1;
                if (dmem_ready)
                    mdr_write = 1'b1;
            end
            
            S_MEM_WRITE: begin
                dmem_req = 1'b1;
                if (dmem_ready) begin
                    pc_write = 1'b1;
                    instr_complete = 1'b1;
                end
            end
            
            S_WRITEBACK: begin
                reg_write_en = 1'b1;
                pc_write = 1'b1;
                instr_complete = 1'b1;
            end
            
            S_BRANCH: begin
                pc_write = 1'b1;
                instr_complete = 1'b1;
            end
            
            S_CSR: begin
                // CSR operations: CSR read/modify happens here
            end
            
            S_HALT: begin
                // HALT state: all control signals remain inactive
            end
            
            default: begin
                // All inactive
            end
        endcase
    end
    
    // ============================================================
    // Module Instantiations
    // ============================================================
    
    // ============================================================
    // Module Instantiations
    // ============================================================
    
    // Branch Decision Unit (uses registered signals for multi-cycle operation)
    branch_unit u_branch_unit (
        .branch(branch_reg),
        .funct3(funct3_reg),
        .rs1_data(a_reg),
        .rs2_data(b_reg),
        .alu_zero(alu_zero),
        .take_branch(take_branch)
    );
    
    // Decoder instantiation (decodes ir_reg)
    decoder u_decoder (
        .instruction(ir_reg),
        .opcode(opcode),
        .rd(rd),
        .rs1(rs1),
        .rs2(rs2),
        .funct3(funct3),
        .funct7(funct7),
        .imm_i(imm_i),
        .imm_s(imm_s),
        .imm_b(imm_b),
        .imm_u(imm_u),
        .imm_j(imm_j),
        .alu_op(alu_op),
        .alu_src(alu_src),
        .reg_write(reg_write),
        .mem_write(mem_write),
        .mem_read(mem_read),
        .mem_to_reg(mem_to_reg),
        .branch(branch),
        .jump(jump),
        .is_ecall(is_ecall),
        .is_ebreak(is_ebreak),
        .is_fence(is_fence),
        .is_csr(is_csr)
    );
    
    // Register file instantiation (write enable gated by FSM)
    regfile u_regfile (
        .clk(clk),
        .we(reg_write_en & reg_write_reg),  // Gated by FSM
        .rs1_addr(rs1_reg),
        .rs2_addr(rs2_reg),
        .rd_addr(rd_reg),
        .rd_data(rd_data),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );
    
    // ALU source selection (multi-cycle: uses registered operands and control)
    always_comb begin
        // Default sources
        alu_a = a_reg;
        alu_b = alu_src_reg ? ((opcode_reg == 7'b0100011) ? imm_s_reg : imm_i_reg) : b_reg;
        
        // Special cases in EXECUTE state
        if (current_state == S_EXECUTE) begin
            case (opcode_reg)
                7'b0010111: begin // AUIPC
                    alu_a = pc;
                    alu_b = imm_u_reg;
                end
                7'b1101111, 7'b1100111: begin // JAL, JALR
                    // Compute PC+4 for return address
                    alu_a = pc;
                    alu_b = 32'd4;
                end
                default: begin
                    // Use default
                end
            endcase
        end
    end
    
    // ALU instantiation (uses registered control signals)
    alu u_alu (
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op_reg),
        .result(alu_result),
        .zero(alu_zero)
    );
    
    // Memory Interface Module (uses registered control signals)
    mem_interface u_mem_interface (
        .funct3(funct3_reg),
        .mem_write(mem_write_reg),
        .mem_read(mem_read_reg),
        .alu_result(alu_out_reg),  // Use registered ALU output
        .rs2_data(b_reg),           // Use registered rs2 data
        .dmem_rdata(dmem_rdata),
        .dmem_addr(dmem_addr),
        .dmem_wdata(dmem_wdata),
        .dmem_we(dmem_we),
        .dmem_re(dmem_re),
        .dmem_size(dmem_size),
        .formatted_load_data(formatted_load_data)
    );
    
    // CSR File Module (uses registered signals)
    csr_file u_csr_file (
        .clk(clk),
        .rst_n(rst_n),
        .is_csr(is_csr_reg & (current_state == S_CSR)),  // Gated by FSM
        .funct3(funct3_reg),
        .rs1(rs1_reg),
        .csr_addr(csr_addr),
        .rs1_data(a_reg),  // Use registered rs1 data
        .csr_rdata(csr_rdata)
    );
    
    // Writeback Multiplexer Module (uses registered signals)
    writeback_mux u_writeback_mux (
        .opcode(opcode_reg),
        .jump(jump_reg),
        .is_csr(is_csr_reg),
        .mem_to_reg(mem_to_reg_reg),
        .pc(pc),
        .imm_u(imm_u_reg),
        .alu_result(alu_out_reg),  // Use registered ALU output
        .csr_rdata(csr_rdata),
        .formatted_load_data(mdr),  // Use MDR (memory data register)
        .rd_data(rd_data)
    );
    
    // Debug outputs (use registered operands)
    assign debug_rs1_data = a_reg;
    assign debug_rs2_data = b_reg;
    assign debug_rd_data = rd_data;

endmodule

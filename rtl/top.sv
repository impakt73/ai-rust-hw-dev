// Top-Level CPU Module
// Multi-cycle non-pipelined RISC-V RV32I/M processor

module top (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory interface (exposed to testbench)
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    
    // Data memory interface (exposed to testbench)
    output logic [31:0] dmem_addr,
    output logic [31:0] dmem_wdata,
    input  logic [31:0] dmem_rdata,
    output logic        dmem_we,
    output logic        dmem_re,    // Memory read enable
    output logic [1:0]  dmem_size,  // Memory operation size: 00=byte, 01=halfword, 10=word
    
    // System control signals
    output logic        halted,         // CPU halted (ECALL/EBREAK)
    output logic        instr_complete, // Asserted for one cycle when instruction completes
    
    // Debug outputs (for tracing register values)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);

    // ------------------------------------------------------------------------
    // State machine definition
    // ------------------------------------------------------------------------
    typedef enum logic [3:0] {
        S_IDLE       = 4'b0000,
        S_FETCH      = 4'b0001,
        S_DECODE     = 4'b0010,
        S_EXECUTE    = 4'b0011,
        S_MEM_ADDR   = 4'b0100,
        S_MEM_READ   = 4'b0101,
        S_MEM_WRITE  = 4'b0110,
        S_WRITEBACK  = 4'b0111,
        S_BRANCH     = 4'b1000,
        S_CSR        = 4'b1001,
        S_HALT       = 4'b1010
    } state_t;

    state_t current_state, next_state;

    // ------------------------------------------------------------------------
    // Internal registers and control signals
    // ------------------------------------------------------------------------
    logic [31:0] pc;
    logic [31:0] next_pc_value;
    logic [31:0] instruction;

    // Decoder outputs
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

    // Latching registers
    logic [31:0] ir;
    logic [31:0] a_reg;
    logic [31:0] b_reg;
    logic [31:0] alu_out_reg;
    logic [31:0] mdr;

    // Latched decoder outputs
    logic [6:0]  opcode_latched;
    logic [4:0]  rd_latched;
    logic [4:0]  rs1_latched;
    logic [4:0]  rs2_latched;
    logic [2:0]  funct3_latched;
    logic [6:0]  funct7_latched;
    logic [31:0] imm_i_latched;
    logic [31:0] imm_s_latched;
    logic [31:0] imm_b_latched;
    logic [31:0] imm_u_latched;
    logic [31:0] imm_j_latched;
    logic [4:0]  alu_op_latched;
    logic        alu_src_latched;
    logic        reg_write_latched;
    logic        mem_write_latched;
    logic        mem_read_latched;
    logic        mem_to_reg_latched;
    logic        branch_latched;
    logic        jump_latched;
    logic        is_ecall_latched;
    logic        is_ebreak_latched;
    logic        is_fence_latched;
    logic        is_csr_latched;
    logic [31:0] csr_rdata_latched;

    // Control signals
    logic ir_write;
    logic a_reg_write;
    logic b_reg_write;
    logic alu_out_write;
    logic mdr_write;
    logic pc_write;
    logic reg_write_en;
    logic decode_latch;

    // Register file signals
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] rd_data;

    // ALU signals
    logic [31:0] alu_a;
    logic [31:0] alu_b;
    logic [31:0] alu_result;
    logic        alu_zero;

    // Branch decision
    logic        take_branch;

    // CSR registers
    logic [31:0] csr_file [0:4095];
    logic [11:0] csr_addr;
    logic [31:0] csr_rdata;

    assign csr_addr   = imm_i[11:0];
    assign imem_addr  = pc;
    assign instruction = (current_state == S_FETCH) ? imem_data : ir;

    // Halt logic
    assign halted = (current_state == S_HALT);

    // ------------------------------------------------------------------------
    // Decoder instantiation
    // ------------------------------------------------------------------------
    decoder u_decoder (
        .instruction(instruction),
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

    // ------------------------------------------------------------------------
    // Register file
    // ------------------------------------------------------------------------
    regfile u_regfile (
        .clk(clk),
        .we(reg_write_en && reg_write_latched),
        .rs1_addr(rs1),
        .rs2_addr(rs2),
        .rd_addr(rd_latched),
        .rd_data(rd_data),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );

    // ------------------------------------------------------------------------
    // State register
    // ------------------------------------------------------------------------
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            current_state <= S_IDLE;
        end else begin
            current_state <= next_state;
        end
    end

    // ------------------------------------------------------------------------
    // Next state logic
    // ------------------------------------------------------------------------
    always_comb begin
        next_state = current_state;
        case (current_state)
            S_IDLE: begin
                next_state = S_FETCH;
            end
            S_FETCH: begin
                next_state = S_DECODE;
            end
            S_DECODE: begin
                if (is_ecall || is_ebreak) begin
                    next_state = S_HALT;
                end else if (is_fence) begin
                    next_state = S_FETCH;
                end else if (is_csr) begin
                    next_state = S_CSR;
                end else if (mem_read || mem_write) begin
                    next_state = S_MEM_ADDR;
                end else if (branch) begin
                    next_state = S_BRANCH;
                end else begin
                    next_state = S_EXECUTE; // R/I, LUI, AUIPC, JAL, JALR
                end
            end
            S_EXECUTE: begin
                next_state = S_WRITEBACK;
            end
            S_MEM_ADDR: begin
                if (mem_read_latched) begin
                    next_state = S_MEM_READ;
                end else begin
                    next_state = S_MEM_WRITE;
                end
            end
            S_MEM_READ: begin
                next_state = S_WRITEBACK;
            end
            S_MEM_WRITE: begin
                next_state = S_FETCH;
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
            default: next_state = S_IDLE;
        endcase
    end

    // ------------------------------------------------------------------------
    // Control signal generation
    // ------------------------------------------------------------------------
    always_comb begin
        ir_write       = 1'b0;
        a_reg_write    = 1'b0;
        b_reg_write    = 1'b0;
        alu_out_write  = 1'b0;
        mdr_write      = 1'b0;
        pc_write       = 1'b0;
        reg_write_en   = 1'b0;
        decode_latch   = 1'b0;
        dmem_we        = 1'b0;
        dmem_re        = 1'b0;
        instr_complete = 1'b0;

        case (current_state)
            S_FETCH: begin
                ir_write = 1'b1;
            end
            S_DECODE: begin
                a_reg_write  = 1'b1;
                b_reg_write  = 1'b1;
                decode_latch = 1'b1;
                if (is_fence) begin
                    pc_write       = 1'b1;
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
                dmem_re   = 1'b1;
                mdr_write = 1'b1;
            end
            S_MEM_WRITE: begin
                dmem_we        = 1'b1;
                pc_write       = 1'b1;
                instr_complete = 1'b1;
            end
            S_WRITEBACK: begin
                reg_write_en   = 1'b1;
                pc_write       = 1'b1;
                instr_complete = 1'b1;
            end
            S_BRANCH: begin
                pc_write       = 1'b1;
                instr_complete = 1'b1;
            end
            default: ;
        endcase
    end

    // ------------------------------------------------------------------------
    // Latching logic
    // ------------------------------------------------------------------------
    // Instruction register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            ir <= 32'h0;
        end else if (ir_write) begin
            ir <= imem_data;
        end
    end

    // A register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            a_reg <= 32'h0;
        end else if (a_reg_write) begin
            a_reg <= rs1_data;
        end
    end

    // B register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            b_reg <= 32'h0;
        end else if (b_reg_write) begin
            b_reg <= rs2_data;
        end
    end

    // ALU output register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            alu_out_reg <= 32'h0;
        end else if (alu_out_write) begin
            alu_out_reg <= alu_result;
        end
    end

    // Memory data register
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            mdr <= 32'h0;
        end else if (mdr_write) begin
            mdr <= formatted_load_data;
        end
    end

    // Decoder output latches
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            opcode_latched      <= 7'h0;
            rd_latched          <= 5'h0;
            rs1_latched         <= 5'h0;
            rs2_latched         <= 5'h0;
            funct3_latched      <= 3'h0;
            funct7_latched      <= 7'h0;
            imm_i_latched       <= 32'h0;
            imm_s_latched       <= 32'h0;
            imm_b_latched       <= 32'h0;
            imm_u_latched       <= 32'h0;
            imm_j_latched       <= 32'h0;
            alu_op_latched      <= 5'h0;
            alu_src_latched     <= 1'b0;
            reg_write_latched   <= 1'b0;
            mem_write_latched   <= 1'b0;
            mem_read_latched    <= 1'b0;
            mem_to_reg_latched  <= 1'b0;
            branch_latched      <= 1'b0;
            jump_latched        <= 1'b0;
            is_ecall_latched    <= 1'b0;
            is_ebreak_latched   <= 1'b0;
            is_fence_latched    <= 1'b0;
            is_csr_latched      <= 1'b0;
            csr_rdata_latched   <= 32'h0;
        end else if (decode_latch) begin
            opcode_latched      <= opcode;
            rd_latched          <= rd;
            rs1_latched         <= rs1;
            rs2_latched         <= rs2;
            funct3_latched      <= funct3;
            funct7_latched      <= funct7;
            imm_i_latched       <= imm_i;
            imm_s_latched       <= imm_s;
            imm_b_latched       <= imm_b;
            imm_u_latched       <= imm_u;
            imm_j_latched       <= imm_j;
            alu_op_latched      <= alu_op;
            alu_src_latched     <= alu_src;
            reg_write_latched   <= reg_write;
            mem_write_latched   <= mem_write;
            mem_read_latched    <= mem_read;
            mem_to_reg_latched  <= mem_to_reg;
            branch_latched      <= branch;
            jump_latched        <= jump;
            is_ecall_latched    <= is_ecall;
            is_ebreak_latched   <= is_ebreak;
            is_fence_latched    <= is_fence;
            is_csr_latched      <= is_csr;
            csr_rdata_latched   <= csr_rdata;
        end
    end

    // ------------------------------------------------------------------------
    // ALU input selection
    // ------------------------------------------------------------------------
    always_comb begin
        case (current_state)
            S_EXECUTE: begin
                if (opcode_latched == 7'b0110111) begin // LUI
                    alu_a = 32'h0;
                end else if (opcode_latched == 7'b0010111 || jump_latched) begin // AUIPC or JAL/JALR
                    alu_a = pc;
                end else begin
                    alu_a = a_reg;
                end
            end
            S_MEM_ADDR: begin
                alu_a = a_reg;
            end
            default: begin
                alu_a = a_reg;
            end
        endcase
    end

    always_comb begin
        case (current_state)
            S_EXECUTE: begin
                if (opcode_latched == 7'b0110111) begin
                    alu_b = imm_u_latched;
                end else if (opcode_latched == 7'b0010111) begin
                    alu_b = imm_u_latched;
                end else if (jump_latched) begin
                    alu_b = 32'd4;
                end else if (alu_src_latched) begin
                    alu_b = imm_i_latched;
                end else begin
                    alu_b = b_reg;
                end
            end
            S_MEM_ADDR: begin
                alu_b = mem_write_latched ? imm_s_latched : imm_i_latched;
            end
            default: begin
                alu_b = b_reg;
            end
        endcase
    end

    // Use latched ALU operation
    logic [4:0] alu_op_sel;
    assign alu_op_sel = alu_op_latched;

    // ALU instantiation
    alu u_alu (
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op_sel),
        .result(alu_result),
        .zero(alu_zero)
    );

    // ------------------------------------------------------------------------
    // Branch decision logic (latched values)
    // ------------------------------------------------------------------------
    always_comb begin
        take_branch = 1'b0;
        if (branch_latched) begin
            case (funct3_latched)
                3'b000: take_branch = (a_reg == b_reg);                              // BEQ
                3'b001: take_branch = (a_reg != b_reg);                              // BNE
                3'b100: take_branch = ($signed(a_reg) < $signed(b_reg));             // BLT
                3'b101: take_branch = ($signed(a_reg) >= $signed(b_reg));            // BGE
                3'b110: take_branch = (a_reg < b_reg);                               // BLTU
                3'b111: take_branch = (a_reg >= b_reg);                              // BGEU
                default: take_branch = 1'b0;
            endcase
        end
    end

    // ------------------------------------------------------------------------
    // PC update logic
    // ------------------------------------------------------------------------
    always_comb begin
        next_pc_value = pc + 32'd4;
        case (current_state)
            S_BRANCH: begin
                if (take_branch) begin
                    next_pc_value = pc + imm_b_latched;
                end else begin
                    next_pc_value = pc + 32'd4;
                end
            end
            S_WRITEBACK: begin
                if (opcode_latched == 7'b1101111) begin
                    next_pc_value = pc + imm_j_latched;
                end else if (opcode_latched == 7'b1100111) begin
                    next_pc_value = (a_reg + imm_i_latched) & ~32'h1;
                end else begin
                    next_pc_value = pc + 32'd4;
                end
            end
            default: next_pc_value = pc + 32'd4;
        endcase
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pc <= boot_addr;
        end else if (pc_write && !halted) begin
            pc <= next_pc_value;
        end
    end

    // ------------------------------------------------------------------------
    // Memory interface
    // ------------------------------------------------------------------------
    assign dmem_addr  = alu_out_reg;
    assign dmem_wdata = b_reg;
    assign dmem_size  = funct3_latched[1:0];

    // Load data formatting (use latched funct3)
    logic [31:0] formatted_load_data;
    always_comb begin
        case (funct3_latched)
            3'b000: formatted_load_data = {{24{dmem_rdata[7]}}, dmem_rdata[7:0]};   // LB
            3'b001: formatted_load_data = {{16{dmem_rdata[15]}}, dmem_rdata[15:0]}; // LH
            3'b100: formatted_load_data = {24'b0, dmem_rdata[7:0]};                 // LBU
            3'b101: formatted_load_data = {16'b0, dmem_rdata[15:0]};                // LHU
            default: formatted_load_data = dmem_rdata;                              // LW
        endcase
    end

    // ------------------------------------------------------------------------
    // CSR logic
    // ------------------------------------------------------------------------
    assign csr_rdata = csr_file[csr_addr];

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            for (int i = 0; i < 4096; i++) begin
                csr_file[i] = 32'h0;
            end
        end else if (current_state == S_CSR && is_csr_latched) begin
            case (funct3_latched)
                3'b001: csr_file[imm_i_latched[11:0]] <= a_reg;                                  // CSRRW
                3'b010: if (rs1_latched != 5'b0) csr_file[imm_i_latched[11:0]] <= csr_rdata_latched | a_reg; // CSRRS
                3'b011: if (rs1_latched != 5'b0) csr_file[imm_i_latched[11:0]] <= csr_rdata_latched & ~a_reg; // CSRRC
                3'b101: csr_file[imm_i_latched[11:0]] <= {27'b0, rs1_latched};                   // CSRRWI
                3'b110: if (rs1_latched != 5'b0) csr_file[imm_i_latched[11:0]] <= csr_rdata_latched | {27'b0, rs1_latched}; // CSRRSI
                3'b111: if (rs1_latched != 5'b0) csr_file[imm_i_latched[11:0]] <= csr_rdata_latched & ~{27'b0, rs1_latched}; // CSRRCI
                default: ;
            endcase
        end
    end

    // ------------------------------------------------------------------------
    // Write-back data selection
    // ------------------------------------------------------------------------
    always_comb begin
        if (opcode_latched == 7'b0110111) begin
            rd_data = alu_out_reg;                 // LUI
        end else if (opcode_latched == 7'b0010111) begin
            rd_data = alu_out_reg;                 // AUIPC
        end else if (jump_latched) begin
            rd_data = alu_out_reg;                 // Return address (PC + 4)
        end else if (is_csr_latched) begin
            rd_data = csr_rdata_latched;           // CSR read value
        end else if (mem_to_reg_latched) begin
            rd_data = mdr;                         // Load
        end else begin
            rd_data = alu_out_reg;                 // ALU result
        end
    end

    // ------------------------------------------------------------------------
    // Debug outputs
    // ------------------------------------------------------------------------
    assign debug_rs1_data = rs1_data;
    assign debug_rs2_data = rs2_data;
    assign debug_rd_data  = rd_data;

endmodule

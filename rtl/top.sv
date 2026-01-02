// Top-Level CPU Module
// Single-cycle RISC-V RV32I processor

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
    output logic        halted,       // CPU halted (ECALL/EBREAK)
    
    // Debug outputs (for tracing register values)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);

    // Internal signals
    logic [31:0] pc;
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
    
    assign csr_addr = imm_i[11:0];  // CSR address from immediate field
    
    
    // Program Counter
    assign imem_addr = pc;
    assign instruction = imem_data;
    
    // PC Control Module
    pc_control u_pc_control (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(boot_addr),
        .branch(branch),
        .take_branch(take_branch),
        .jump(jump),
        .is_ecall(is_ecall),
        .is_ebreak(is_ebreak),
        .opcode(opcode),
        .rs1_data(rs1_data),
        .imm_i(imm_i),
        .imm_b(imm_b),
        .imm_j(imm_j),
        .pc(pc),
        .halted(halted)
    );
    
    // Branch Decision Unit
    branch_unit u_branch_unit (
        .branch(branch),
        .funct3(funct3),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data),
        .alu_zero(alu_zero),
        .take_branch(take_branch)
    );
    
    // Decoder instantiation
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
    
    // Register file instantiation
    regfile u_regfile (
        .clk(clk),
        .we(reg_write),
        .rs1_addr(rs1),
        .rs2_addr(rs2),
        .rd_addr(rd),
        .rd_data(rd_data),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );
    
    // ALU source selection
    assign alu_a = rs1_data;
    assign alu_b = alu_src
                    ? ((opcode == 7'b0100011) ? imm_s : imm_i)
                    : rs2_data;
    
    // ALU instantiation
    alu u_alu (
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op),
        .result(alu_result),
        .zero(alu_zero)
    );
    
    // Memory Interface Module
    mem_interface u_mem_interface (
        .funct3(funct3),
        .mem_write(mem_write),
        .mem_read(mem_read),
        .alu_result(alu_result),
        .rs2_data(rs2_data),
        .dmem_rdata(dmem_rdata),
        .dmem_addr(dmem_addr),
        .dmem_wdata(dmem_wdata),
        .dmem_we(dmem_we),
        .dmem_re(dmem_re),
        .dmem_size(dmem_size),
        .formatted_load_data(formatted_load_data)
    );
    
    // CSR File Module
    csr_file u_csr_file (
        .clk(clk),
        .rst_n(rst_n),
        .is_csr(is_csr),
        .funct3(funct3),
        .rs1(rs1),
        .csr_addr(csr_addr),
        .rs1_data(rs1_data),
        .csr_rdata(csr_rdata)
    );
    
    // Writeback Multiplexer Module
    writeback_mux u_writeback_mux (
        .opcode(opcode),
        .jump(jump),
        .is_csr(is_csr),
        .mem_to_reg(mem_to_reg),
        .pc(pc),
        .imm_u(imm_u),
        .alu_result(alu_result),
        .csr_rdata(csr_rdata),
        .formatted_load_data(formatted_load_data),
        .rd_data(rd_data)
    );
    
    // Debug outputs
    assign debug_rs1_data = rs1_data;
    assign debug_rs2_data = rs2_data;
    assign debug_rd_data = rd_data;

endmodule

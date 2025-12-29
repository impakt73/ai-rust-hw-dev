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
    
    // FIFO interface (for memory-mapped I/O communication)
    input  logic        fifo_rd_en,
    output logic [7:0]  fifo_rd_data,
    output logic        fifo_empty,
    output logic        fifo_full,
    
    // Debug outputs (for tracing register values)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);

    // Internal signals
    logic [31:0] pc;
    logic [31:0] next_pc;
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
    logic [3:0]  alu_op;
    logic        alu_src;
    logic        reg_write;
    logic        mem_write;
    logic        mem_to_reg;
    logic        branch;
    logic        jump;
    
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
    
    // Program Counter
    assign imem_addr = pc;
    assign instruction = imem_data;
    
    // PC update logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pc <= boot_addr;
        end else begin
            pc <= next_pc;
        end
    end
    
    // Next PC calculation
    always_comb begin
        if (jump) begin
            // JAL or JALR
            if (opcode == 7'b1100111) begin
                // JALR: PC = (rs1 + imm) & ~1
                next_pc = (rs1_data + imm_i) & ~32'h1;
            end else begin
                // JAL: PC = PC + imm
                next_pc = pc + imm_j;
            end
        end else if (branch && take_branch) begin
            next_pc = pc + imm_b;
        end else begin
            next_pc = pc + 32'd4;  // Next sequential instruction
        end
    end
    
    // Branch decision logic
    always_comb begin
        take_branch = 1'b0;
        if (branch) begin
            case (funct3)
                3'b000: take_branch = alu_zero;                              // BEQ
                3'b001: take_branch = !alu_zero;                             // BNE
                3'b100: take_branch = ($signed(rs1_data) <  $signed(rs2_data));  // BLT (signed)
                3'b101: take_branch = ($signed(rs1_data) >= $signed(rs2_data));  // BGE (signed)
                3'b110: take_branch = (rs1_data <  rs2_data);                // BLTU (unsigned)
                3'b111: take_branch = (rs1_data >= rs2_data);                // BGEU (unsigned)
                default: take_branch = 1'b0;
            endcase
        end
    end
    
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
        .mem_to_reg(mem_to_reg),
        .branch(branch),
        .jump(jump)
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
    
    // Special handling for LUI
    logic [31:0] lui_result;
    assign lui_result = imm_u;
    
    // ALU instantiation
    alu u_alu (
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op),
        .result(alu_result),
        .zero(alu_zero)
    );
    
    // Data memory interface
    assign dmem_addr = alu_result;
    assign dmem_wdata = rs2_data;
    assign dmem_we = mem_write;
    
    // Memory-mapped I/O region for FIFO (0xF0000000 - 0xF000000F)
    localparam MMIO_BASE = 32'hF0000000;
    localparam MMIO_DATA_OFFSET = 32'h0;
    localparam MMIO_STATUS_OFFSET = 32'h4;
    
    logic is_mmio_access;
    logic is_mmio_data_reg;
    logic is_mmio_status_reg;
    logic fifo_wr_en;
    logic [31:0] mmio_rdata;
    
    assign is_mmio_access = (alu_result >= MMIO_BASE) && (alu_result < (MMIO_BASE + 32'h10));
    assign is_mmio_data_reg = is_mmio_access && ((alu_result - MMIO_BASE) == MMIO_DATA_OFFSET);
    assign is_mmio_status_reg = is_mmio_access && ((alu_result - MMIO_BASE) == MMIO_STATUS_OFFSET);
    
    // FIFO write enable: write to MMIO data register
    assign fifo_wr_en = mem_write && is_mmio_data_reg;
    
    // FIFO instantiation
    fifo #(
        .DEPTH(16),
        .WIDTH(8)
    ) u_fifo (
        .clk(clk),
        .rst_n(rst_n),
        .wr_en(fifo_wr_en),
        .wr_data(rs2_data[7:0]),
        .rd_en(fifo_rd_en),
        .rd_data(fifo_rd_data),
        .empty(fifo_empty),
        .full(fifo_full)
    );
    
    // MMIO read data
    always_comb begin
        if (is_mmio_status_reg) begin
            // STATUS register: bit 0 = empty, bit 1 = full
            mmio_rdata = {30'b0, fifo_full, fifo_empty};
        end else if (is_mmio_data_reg) begin
            // DATA register: zero-extended FIFO read data
            mmio_rdata = {24'b0, fifo_rd_data};
        end else begin
            mmio_rdata = 32'b0;
        end
    end
    
    // Write-back data selection
    always_comb begin
        if (opcode == 7'b0110111) begin
            // LUI - Load Upper Immediate
            rd_data = lui_result;
        end else if (opcode == 7'b0010111) begin
            // AUIPC - Add Upper Immediate to PC
            rd_data = pc + imm_u;
        end else if (jump) begin
            // JAL/JALR - Store return address (PC + 4)
            rd_data = pc + 32'd4;
        end else if (mem_to_reg) begin
            // Load instruction - check if MMIO or regular memory
            if (is_mmio_access) begin
                rd_data = mmio_rdata;
            end else begin
                rd_data = dmem_rdata;
            end
        end else begin
            // ALU result
            rd_data = alu_result;
        end
    end
    
    // Debug outputs
    assign debug_rs1_data = rs1_data;
    assign debug_rs2_data = rs2_data;
    assign debug_rd_data = rd_data;

endmodule

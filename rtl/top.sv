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
    assign dmem_we = mem_write;
    
    // Store data formatting based on funct3 (byte/halfword/word)
    logic [31:0] formatted_store_data;
    always_comb begin
        case (funct3)
            3'b000: begin // SB - Store Byte
                // Replicate byte to all positions based on address alignment
                case (alu_result[1:0])
                    2'b00: formatted_store_data = {24'b0, rs2_data[7:0]};
                    2'b01: formatted_store_data = {16'b0, rs2_data[7:0], 8'b0};
                    2'b10: formatted_store_data = {8'b0, rs2_data[7:0], 16'b0};
                    2'b11: formatted_store_data = {rs2_data[7:0], 24'b0};
                endcase
            end
            3'b001: begin // SH - Store Halfword
                // Replicate halfword based on address alignment
                case (alu_result[1])
                    1'b0: formatted_store_data = {16'b0, rs2_data[15:0]};
                    1'b1: formatted_store_data = {rs2_data[15:0], 16'b0};
                endcase
            end
            default: formatted_store_data = rs2_data; // SW - Store Word
        endcase
    end
    assign dmem_wdata = formatted_store_data;
    
    // Load data formatting based on funct3 (byte/halfword/word)
    logic [31:0] formatted_load_data;
    always_comb begin
        case (funct3)
            3'b000: begin // LB - Load Byte (sign-extended)
                case (alu_result[1:0])
                    2'b00: formatted_load_data = {{24{dmem_rdata[7]}}, dmem_rdata[7:0]};
                    2'b01: formatted_load_data = {{24{dmem_rdata[15]}}, dmem_rdata[15:8]};
                    2'b10: formatted_load_data = {{24{dmem_rdata[23]}}, dmem_rdata[23:16]};
                    2'b11: formatted_load_data = {{24{dmem_rdata[31]}}, dmem_rdata[31:24]};
                endcase
            end
            3'b001: begin // LH - Load Halfword (sign-extended)
                case (alu_result[1])
                    1'b0: formatted_load_data = {{16{dmem_rdata[15]}}, dmem_rdata[15:0]};
                    1'b1: formatted_load_data = {{16{dmem_rdata[31]}}, dmem_rdata[31:16]};
                endcase
            end
            3'b100: begin // LBU - Load Byte Unsigned (zero-extended)
                case (alu_result[1:0])
                    2'b00: formatted_load_data = {24'b0, dmem_rdata[7:0]};
                    2'b01: formatted_load_data = {24'b0, dmem_rdata[15:8]};
                    2'b10: formatted_load_data = {24'b0, dmem_rdata[23:16]};
                    2'b11: formatted_load_data = {24'b0, dmem_rdata[31:24]};
                endcase
            end
            3'b101: begin // LHU - Load Halfword Unsigned (zero-extended)
                case (alu_result[1])
                    1'b0: formatted_load_data = {16'b0, dmem_rdata[15:0]};
                    1'b1: formatted_load_data = {16'b0, dmem_rdata[31:16]};
                endcase
            end
            default: formatted_load_data = dmem_rdata; // LW - Load Word
        endcase
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
            // Load instruction - Use formatted memory data
            rd_data = formatted_load_data;
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

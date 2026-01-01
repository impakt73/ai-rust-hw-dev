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
    
    // Debug outputs (for tracing register values and instruction info)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data,
    
    // Debug outputs for RV32C support (instruction tracing)
    output logic [31:0] debug_pc,              // Current PC value
    output logic [31:0] debug_fetched_insn,    // Raw instruction from ifetch
    output logic [31:0] debug_executed_insn,   // Instruction being executed (post-decompression)
    output logic        debug_is_compressed    // Whether current instruction is compressed
);

    // Internal signals
    logic [31:0] pc;
    logic [31:0] next_pc;
    logic [31:0] instruction;
    
    // RV32C support signals
    logic [31:0] fetched_instruction;  // Full instruction from ifetch
    logic        fetch_valid;
    logic [31:0] decompressed_insn;
    logic        is_compressed;
    logic        decompress_valid;
    logic [31:0] pc_increment;  // 2 or 4 based on instruction size
    
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
    
    // CSR registers (4096 possible, but we only implement a few)
    logic [31:0] csr_file [0:4095];
    logic [11:0] csr_addr;
    logic [31:0] csr_rdata;
    
    assign csr_addr = imm_i[11:0];  // CSR address from immediate field
    
    // Program Counter and instruction fetch
    // Note: imem_addr is now driven by ifetch module
    
    // Instruction fetch unit - handles 16/32-bit instruction fetching
    ifetch u_ifetch (
        .clk(clk),
        .rst_n(rst_n),
        .pc(pc),
        .imem_data(imem_data),
        .imem_addr(imem_addr),
        .instruction(fetched_instruction),
        .valid(fetch_valid)
    );
    
    // Decompressor - expands compressed instructions to standard 32-bit format
    decompress u_decompress (
        .insn_16(fetched_instruction[15:0]),
        .insn_32(decompressed_insn),
        .is_compressed(is_compressed),
        .is_valid(decompress_valid)
    );
    
    // Select instruction based on compression
    // If compressed but invalid, treat as NOP (ADDI x0, x0, 0)
    // This prevents execution of undefined behavior while avoiding CPU halt
    assign instruction = is_compressed ? 
                         (decompress_valid ? decompressed_insn : 32'h00000013) : 
                         fetched_instruction;
    
    // PC increment: 2 for compressed, 4 for standard
    assign pc_increment = is_compressed ? 32'd2 : 32'd4;
    
    // Halt control for ECALL/EBREAK
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            halted <= 1'b0;
        end else if (is_ecall || is_ebreak) begin
            halted <= 1'b1;
        end
    end
    
    // PC update logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pc <= boot_addr;
        end else if (!halted && !is_ecall && !is_ebreak) begin
            // Advance PC only when not halted and not executing ECALL/EBREAK
            pc <= next_pc;
        end
        // If halted or executing ECALL/EBREAK, PC stays the same
    end
    
    // Next PC calculation - updated for RV32C support
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
            // Sequential: increment by 2 or 4 depending on instruction size
            next_pc = pc + pc_increment;
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
    assign dmem_wdata = rs2_data;  // Pass data directly - no formatting needed
    assign dmem_we = mem_write;
    assign dmem_re = mem_read;
    
    // Encode memory operation size from funct3
    // funct3[1:0] distinguishes byte (00), halfword (01), word (10)
    // For loads: LB=000, LH=001, LW=010, LBU=100, LHU=101
    // For stores: SB=000, SH=001, SW=010
    assign dmem_size = funct3[1:0];
    
    // Load data sign/zero extension based on funct3
    // The simulator will return the exact byte/halfword requested
    // We only need to perform sign/zero extension here
    logic [31:0] formatted_load_data;
    always_comb begin
        case (funct3)
            3'b000: begin // LB - Load Byte (sign-extended)
                formatted_load_data = {{24{dmem_rdata[7]}}, dmem_rdata[7:0]};
            end
            3'b001: begin // LH - Load Halfword (sign-extended)
                formatted_load_data = {{16{dmem_rdata[15]}}, dmem_rdata[15:0]};
            end
            3'b100: begin // LBU - Load Byte Unsigned (zero-extended)
                formatted_load_data = {24'b0, dmem_rdata[7:0]};
            end
            3'b101: begin // LHU - Load Halfword Unsigned (zero-extended)
                formatted_load_data = {16'b0, dmem_rdata[15:0]};
            end
            default: formatted_load_data = dmem_rdata; // LW - Load Word
        endcase
    end
    
    // CSR register file
    // Read CSR value
    assign csr_rdata = csr_file[csr_addr];
    
    // CSR write logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            // Initialize all CSRs to 0
            for (int i = 0; i < 4096; i++) begin
                csr_file[i] = 32'h0;  // Use blocking assignment for initialization
            end
        end else if (is_csr) begin
            // CSR write operations
            case (funct3)
                3'b001: csr_file[csr_addr] <= rs1_data;                                     // CSRRW
                3'b010: if (rs1 != 5'b0) csr_file[csr_addr] <= csr_rdata | rs1_data;        // CSRRS (no write when rs1 == x0)
                3'b011: if (rs1 != 5'b0) csr_file[csr_addr] <= csr_rdata & ~rs1_data;       // CSRRC (no write when rs1 == x0)
                3'b101: csr_file[csr_addr] <= {27'b0, rs1};                                 // CSRRWI
                3'b110: if (rs1 != 5'b0) csr_file[csr_addr] <= csr_rdata | {27'b0, rs1};    // CSRRSI (no write when zimm[4:0] == 0)
                3'b111: if (rs1 != 5'b0) csr_file[csr_addr] <= csr_rdata & ~{27'b0, rs1};   // CSRRCI (no write when zimm[4:0] == 0)
                default: ; // Do nothing
            endcase
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
        end else if (is_csr) begin
            // CSR instruction - Return old CSR value
            rd_data = csr_rdata;
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
    
    // Debug outputs for RV32C support
    assign debug_pc = pc;
    assign debug_fetched_insn = fetched_instruction;
    assign debug_executed_insn = instruction;
    assign debug_is_compressed = is_compressed;

endmodule

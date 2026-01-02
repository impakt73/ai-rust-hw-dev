// Program Counter Control Module
// Manages PC register and next PC calculation

module pc_control (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Control signals
    input  logic        branch,
    input  logic        take_branch,
    input  logic        jump,
    input  logic        is_ecall,
    input  logic        is_ebreak,
    
    // Jump/Branch targets
    input  logic [6:0]  opcode,
    input  logic [31:0] rs1_data,
    input  logic [31:0] imm_i,
    input  logic [31:0] imm_b,
    input  logic [31:0] imm_j,
    
    // Outputs
    output logic [31:0] pc,
    output logic        halted
);

    logic [31:0] next_pc;
    
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

endmodule

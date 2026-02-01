// Branch Decision Unit
// Evaluates branch conditions for RISC-V branch instructions
// All comparisons are performed directly on registered operands to break
// the timing path through the ALU.

module branch_unit (
    input  logic        branch,
    input  logic [2:0]  funct3,
    input  logic [31:0] rs1_data,
    input  logic [31:0] rs2_data,
    input  logic        alu_zero,  // Unused - kept for interface compatibility
    
    output logic        take_branch
);

    // Branch decision logic
    // Note: All comparisons are done directly on registered operands (a_reg, b_reg)
    // to avoid dependency on ALU result, improving timing closure.
    always_comb begin
        take_branch = 1'b0;
        if (branch) begin
            case (funct3)
                3'b000: take_branch = (rs1_data == rs2_data);                    // BEQ
                3'b001: take_branch = (rs1_data != rs2_data);                    // BNE
                3'b100: take_branch = ($signed(rs1_data) <  $signed(rs2_data));  // BLT (signed)
                3'b101: take_branch = ($signed(rs1_data) >= $signed(rs2_data));  // BGE (signed)
                3'b110: take_branch = (rs1_data <  rs2_data);                    // BLTU (unsigned)
                3'b111: take_branch = (rs1_data >= rs2_data);                    // BGEU (unsigned)
                default: take_branch = 1'b0;
            endcase
        end
    end

endmodule

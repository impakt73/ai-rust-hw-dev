`default_nettype none
// Branch Decision Unit
// Evaluates branch conditions for RISC-V branch instructions
// All comparisons are performed directly on registered operands to break
// the timing path through the ALU.

module branch_unit (
    input wire logic        clk,
    input wire logic        rst,
    input wire logic        branch,
    input wire logic [2:0]  funct3,
    input wire logic [31:0] rs1_data,
    input wire logic [31:0] rs2_data,
    
    output logic        take_branch
);

    logic take_branch_next;

    // Branch decision logic
    // Note: All comparisons are done directly on registered operands (rs1_data, rs2_data)
    // during the CPU's S_EXECUTE state and then registered locally to break the
    // path into the following S_BRANCH stage.
    always_comb begin
        take_branch_next = 1'b0;
        if (branch) begin
            case (funct3)
                3'b000: take_branch_next = (rs1_data == rs2_data);                    // BEQ
                3'b001: take_branch_next = (rs1_data != rs2_data);                    // BNE
                3'b100: take_branch_next = ($signed(rs1_data) <  $signed(rs2_data));  // BLT (signed)
                3'b101: take_branch_next = ($signed(rs1_data) >= $signed(rs2_data));  // BGE (signed)
                3'b110: take_branch_next = (rs1_data <  rs2_data);                    // BLTU (unsigned)
                3'b111: take_branch_next = (rs1_data >= rs2_data);                    // BGEU (unsigned)
                default: take_branch_next = 1'b0;
            endcase
        end
    end

    always_ff @(posedge clk) begin
        if (rst)
            take_branch <= 1'b0;
        else
            take_branch <= take_branch_next;
    end

endmodule
`default_nettype wire

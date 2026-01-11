// Floating Point Register File Module
// 32x32-bit FP register file for RISC-V RV32F extension
// Unlike integer x0, all FP registers (f0-f31) are writable

module fp_regfile (
    input  logic        clk,
    input  logic        rst_n,        // Active-low reset
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // FP source register 1 address
    input  logic [4:0]  rs2_addr,     // FP source register 2 address
    input  logic [4:0]  rs3_addr,     // FP source register 3 address (for FMADD, etc.)
    input  logic [4:0]  rd_addr,      // FP destination register address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data,     // Read data 2
    output logic [31:0] rs3_data      // Read data 3
);

    // 32x32-bit FP register array
    logic [31:0] fp_registers [31:0];

    // Read operations (combinational/asynchronous)
    // All FP registers are readable (no special handling like x0)
    always_comb begin
        rs1_data = fp_registers[rs1_addr];
        rs2_data = fp_registers[rs2_addr];
        rs3_data = fp_registers[rs3_addr];
    end

    // Write operation (synchronous)
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            // Reset all FP registers to +0.0 (0x00000000)
            for (int i = 0; i < 32; i++) begin
                fp_registers[i] <= 32'h00000000;
            end
        end else if (we) begin
            // All FP registers can be written (no x0-like restriction)
            fp_registers[rd_addr] <= rd_data;
        end
    end

endmodule

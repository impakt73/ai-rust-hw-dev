// Register File Module
// 32x32-bit register file for RISC-V RV32I
// x0 is hardwired to 0

module regfile (
    input  logic        clk,
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // Read address 1
    input  logic [4:0]  rs2_addr,     // Read address 2
    input  logic [4:0]  rd_addr,      // Write address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data      // Read data 2
);

    // 32x32-bit register array
    logic [31:0] registers [31:0];

    // Read operations (combinational/asynchronous)
    always_comb begin
        // x0 is always 0
        if (rs1_addr == 5'd0)
            rs1_data = 32'd0;
        else
            rs1_data = registers[rs1_addr];
    end

    always_comb begin
        // x0 is always 0
        if (rs2_addr == 5'd0)
            rs2_data = 32'd0;
        else
            rs2_data = registers[rs2_addr];
    end

    // Write operation (synchronous)
    always_ff @(posedge clk) begin
        // Only write if write enable is high and address is not x0
        if (we && rd_addr != 5'd0) begin
            registers[rd_addr] <= rd_data;
        end
    end

endmodule

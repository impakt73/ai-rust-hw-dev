module fpu (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        fpu_start,
    input  logic [31:0] fs1,
    input  logic [31:0] fs2,
    input  logic [31:0] fs3,
    input  logic [31:0] int_src,
    input  logic [4:0]  fpu_op,
    input  logic [2:0]  rm,
    output logic [31:0] fp_result,
    output logic [31:0] int_result,
    output logic [4:0]  fflags,
    output logic        fpu_ready
);
    assign fp_result = 32'h0;
    assign int_result = 32'h0;
    assign fflags = 5'h0;
    assign fpu_ready = 1'b1;
endmodule

module fp_regfile (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        we,
    input  logic [4:0]  rs1_addr,
    input  logic [4:0]  rs2_addr,
    input  logic [4:0]  rs3_addr,
    input  logic [4:0]  rd_addr,
    input  logic [31:0] rd_data,
    output logic [31:0] rs1_data,
    output logic [31:0] rs2_data,
    output logic [31:0] rs3_data
);
    assign rs1_data = 32'h0;
    assign rs2_data = 32'h0;
    assign rs3_data = 32'h0;
endmodule

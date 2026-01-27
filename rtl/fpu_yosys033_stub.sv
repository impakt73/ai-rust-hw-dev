// Simplified FPU stub for Yosys 0.33 compatibility
// This module provides the same interface as the full FPU but with minimal functionality
// Only implements operations that don't require complex floating-point arithmetic

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

    // FPU Operation Encodings (subset)
    localparam logic [4:0] FPU_SGNJ   = 5'b01011;
    localparam logic [4:0] FPU_SGNJN  = 5'b01100;
    localparam logic [4:0] FPU_SGNJX  = 5'b01101;
    localparam logic [4:0] FPU_MVXW   = 5'b10110;
    localparam logic [4:0] FPU_MVWX   = 5'b10111;

    // Always ready for this stub implementation
    assign fpu_ready = 1'b1;

    always @(*) begin
        fp_result = 32'h00000000;
        int_result = 32'h0;
        fflags = 5'b0;
        
        case (fpu_op)
            FPU_SGNJ:  fp_result = {fs2[31], fs1[30:0]};
            FPU_SGNJN: fp_result = {~fs2[31], fs1[30:0]};
            FPU_SGNJX: fp_result = {fs1[31] ^ fs2[31], fs1[30:0]};
            FPU_MVXW:  int_result = fs1;
            FPU_MVWX:  fp_result = int_src;
            default:   fp_result = 32'h00000000;  // Return +0.0 for unsupported ops
        endcase
    end

endmodule

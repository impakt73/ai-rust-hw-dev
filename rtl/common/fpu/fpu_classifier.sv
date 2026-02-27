// FPU Value Classifier Module
// Provides classification signals for floating-point values

module fpu_classifier (
    input  logic [31:0] val,
    output logic        is_nan,
    output logic        is_snan,
    output logic        is_inf,
    output logic        is_zero,
    output logic        is_subnormal
);

    // IEEE 754 classification
    always_comb begin
        is_nan = (val[30:23] == 8'hFF) && (val[22:0] != 23'h0);
        is_snan = (val[30:23] == 8'hFF) && (val[22:0] != 23'h0) && (val[22] == 1'b0);
        is_inf = (val[30:23] == 8'hFF) && (val[22:0] == 23'h0);
        is_zero = (val[30:0] == 31'h0);
        is_subnormal = (val[30:23] == 8'h00) && (val[22:0] != 23'h0);
    end

endmodule

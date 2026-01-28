// FPU Multiplier Module
// Implements IEEE 754 single-precision multiplication

module fpu_multiplier (
    input  logic [31:0] a,
    input  logic [31:0] b,
    output logic [31:0] result,
    output logic [4:0]  flags
);

    // IEEE 754 constants
    localparam [31:0] QNAN = 32'h7FC00000;
    
    logic a_is_nan, b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero;
    logic result_sign;
    logic [8:0] result_exp_wide;
    logic [7:0] result_exp;
    logic [47:0] product;
    logic [22:0] result_mant;
    
    always_comb begin
        // Classification
        a_is_nan = (a[30:23] == 8'hFF) && (a[22:0] != 23'h0);
        b_is_nan = (b[30:23] == 8'hFF) && (b[22:0] != 23'h0);
        a_is_inf = (a[30:23] == 8'hFF) && (a[22:0] == 23'h0);
        b_is_inf = (b[30:23] == 8'hFF) && (b[22:0] == 23'h0);
        a_is_zero = (a[30:0] == 31'h0);
        b_is_zero = (b[30:0] == 31'h0);
        
        flags = 5'b0;
        
        // Handle special cases
        if (a_is_nan || b_is_nan) begin
            result = QNAN;
        end else if (a_is_inf) begin
            if (b_is_zero) begin
                result = QNAN;
            end else begin
                result = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (b_is_inf) begin
            if (a_is_zero) begin
                result = QNAN;
            end else begin
                result = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (a_is_zero || b_is_zero) begin
            result = {a[31] ^ b[31], 31'h0};
        end else begin
            // Normal multiplication
            result_sign = a[31] ^ b[31];
            result_exp_wide = {1'b0, a[30:23]} + {1'b0, b[30:23]} - 9'd127;
            product = {1'b1, a[22:0]} * {1'b1, b[22:0]};
            
            if (product[47]) begin
                result_mant = product[46:24];
                result_exp_wide = result_exp_wide + 1;
            end else begin
                result_mant = product[45:23];
            end
            
            if (result_exp_wide[8] || result_exp_wide > 254) begin
                // Overflow to infinity
                result = {result_sign, 8'hFF, 23'h0};
            end else if (result_exp_wide == 0) begin
                // Underflow to zero
                result = {result_sign, 31'h0};
            end else begin
                result_exp = result_exp_wide[7:0];
                result = {result_sign, result_exp, result_mant};
            end
        end
    end

endmodule

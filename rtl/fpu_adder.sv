// FPU Adder/Subtractor Module
// Implements IEEE 754 single-precision addition and subtraction

module fpu_adder (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic        is_sub,    // 1 for subtraction, 0 for addition
    output logic [31:0] result,
    output logic [4:0]  flags
);

    // IEEE 754 constants
    localparam [31:0] POS_ZERO = 32'h00000000;
    localparam [31:0] QNAN     = 32'h7FC00000;
    
    logic a_is_nan, b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero;
    logic a_sign, b_sign, result_sign;
    logic [7:0] a_exp, b_exp, result_exp;
    logic [23:0] a_mant, b_mant;
    logic [24:0] result_mant;
    logic [7:0] exp_diff;
    integer norm_shift;
    
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
        end else if (a_is_inf && b_is_inf) begin
            if (a[31] != (b[31] ^ is_sub)) begin
                result = QNAN;
            end else begin
                result = a;
            end
        end else if (a_is_inf) begin
            result = a;
        end else if (b_is_inf) begin
            result = is_sub ? {~b[31], b[30:0]} : b;
        end else if (a_is_zero) begin
            result = is_sub ? {~b[31], b[30:0]} : b;
        end else if (b_is_zero) begin
            result = a;
        end else begin
            // Normal addition/subtraction
            a_sign = a[31];
            b_sign = b[31] ^ is_sub;
            a_exp = a[30:23];
            b_exp = b[30:23];
            a_mant = {1'b1, a[22:0]};
            b_mant = {1'b1, b[22:0]};
            
            // Align mantissas
            if (a_exp > b_exp) begin
                exp_diff = a_exp - b_exp;
                if (exp_diff < 24) begin
                    b_mant = b_mant >> exp_diff;
                end else begin
                    b_mant = 24'h0;
                end
                result_exp = a_exp;
            end else begin
                exp_diff = b_exp - a_exp;
                if (exp_diff < 24) begin
                    a_mant = a_mant >> exp_diff;
                end else begin
                    a_mant = 24'h0;
                end
                result_exp = b_exp;
            end
            
            // Add or subtract
            if (a_sign == b_sign) begin
                result_mant = a_mant + b_mant;
                result_sign = a_sign;
                
                // Normalize if overflow
                if (result_mant[24]) begin
                    result_mant = result_mant >> 1;
                    result_exp = result_exp + 1;
                end
            end else begin
                if (a_mant >= b_mant) begin
                    result_mant = a_mant - b_mant;
                    result_sign = a_sign;
                end else begin
                    result_mant = b_mant - a_mant;
                    result_sign = b_sign;
                end
                
                // Normalize - find leading one (replace while loop with priority encoder)
                norm_shift = 0;
                if (result_mant != 0) begin
                    // Priority encoder to find MSB position
                    if (!result_mant[23]) begin
                        if (result_mant[22:16] != 0) begin
                            for (int i = 22; i >= 16; i--) begin
                                if (result_mant[i] && norm_shift == 0) begin
                                    norm_shift = 23 - i;
                                end
                            end
                        end else if (result_mant[15:8] != 0) begin
                            for (int i = 15; i >= 8; i--) begin
                                if (result_mant[i] && norm_shift == 0) begin
                                    norm_shift = 23 - i;
                                end
                            end
                        end else begin
                            for (int i = 7; i >= 0; i--) begin
                                if (result_mant[i] && norm_shift == 0) begin
                                    norm_shift = 23 - i;
                                end
                            end
                        end
                        
                        // Apply normalization
                        if (norm_shift <= result_exp) begin
                            result_mant = result_mant << norm_shift;
                            result_exp = result_exp - norm_shift;
                        end else begin
                            // Underflow to zero
                            result_mant = 0;
                        end
                    end
                end
            end
            
            // Return result
            if (result_mant == 0) begin
                result = POS_ZERO;
            end else begin
                result = {result_sign, result_exp, result_mant[22:0]};
            end
        end
    end

endmodule

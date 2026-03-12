`default_nettype none
// FPU Fused Multiply-Add Module
// Implements (a * b) +/- c with sign control
// Self-contained implementation with inlined multiplier and adder logic

module fpu_fma (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [31:0] c,
    input  logic        negate_product,   // If 1, negate (a*b)
    input  logic        negate_addend,    // If 1, subtract c instead of add
    output logic [31:0] result,
    output logic [4:0]  flags
);

    // IEEE 754 constants
    localparam [31:0] POS_ZERO = 32'h00000000;
    localparam [31:0] QNAN     = 32'h7FC00000;

    // ============================================================
    // Stage 1: Multiplication (a * b)
    // ============================================================
    logic [31:0] product;
    logic [4:0]  mul_flags;
    
    // Multiplier internal signals
    logic a_mul_is_nan, b_mul_is_nan, a_mul_is_inf, b_mul_is_inf, a_mul_is_zero, b_mul_is_zero;
    logic mul_result_sign;
    logic [8:0] mul_result_exp_wide;
    logic [7:0] mul_result_exp;
    logic [47:0] mul_product;
    logic [22:0] mul_result_mant;
    
    always_comb begin
        // Classification for multiplication
        a_mul_is_nan = (a[30:23] == 8'hFF) && (a[22:0] != 23'h0);
        b_mul_is_nan = (b[30:23] == 8'hFF) && (b[22:0] != 23'h0);
        a_mul_is_inf = (a[30:23] == 8'hFF) && (a[22:0] == 23'h0);
        b_mul_is_inf = (b[30:23] == 8'hFF) && (b[22:0] == 23'h0);
        a_mul_is_zero = (a[30:0] == 31'h0);
        b_mul_is_zero = (b[30:0] == 31'h0);
        
        // Initialize all signals to avoid latches
        mul_flags = 5'b0;
        product = 32'h0;
        mul_result_sign = 1'b0;
        mul_result_exp_wide = 9'b0;
        mul_result_exp = 8'b0;
        mul_product = 48'h0;
        mul_result_mant = 23'h0;
        
        // Handle special cases
        if (a_mul_is_nan || b_mul_is_nan) begin
            product = QNAN;
        end else if (a_mul_is_inf) begin
            if (b_mul_is_zero) begin
                product = QNAN;
            end else begin
                product = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (b_mul_is_inf) begin
            if (a_mul_is_zero) begin
                product = QNAN;
            end else begin
                product = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (a_mul_is_zero || b_mul_is_zero) begin
            product = {a[31] ^ b[31], 31'h0};
        end else begin
            // Normal multiplication
            mul_result_sign = a[31] ^ b[31];
            mul_result_exp_wide = {1'b0, a[30:23]} + {1'b0, b[30:23]} - 9'd127;
            mul_product = {1'b1, a[22:0]} * {1'b1, b[22:0]};
            
            if (mul_product[47]) begin
                mul_result_mant = mul_product[46:24];
                mul_result_exp_wide = mul_result_exp_wide + 1;
            end else begin
                mul_result_mant = mul_product[45:23];
            end
            
            if (mul_result_exp_wide[8] || mul_result_exp_wide > 254) begin
                // Overflow to infinity
                product = {mul_result_sign, 8'hFF, 23'h0};
            end else if (mul_result_exp_wide == 0) begin
                // Underflow to zero
                product = {mul_result_sign, 31'h0};
            end else begin
                mul_result_exp = mul_result_exp_wide[7:0];
                product = {mul_result_sign, mul_result_exp, mul_result_mant};
            end
        end
    end
    
    // Apply product sign negation if needed
    logic [31:0] product_signed;
    assign product_signed = negate_product ? {~product[31], product[30:0]} : product;
    
    // ============================================================
    // Stage 2: Addition (product_signed +/- c)
    // ============================================================
    logic [4:0] add_flags;
    
    // Adder internal signals
    logic add_a_is_nan, add_b_is_nan, add_a_is_inf, add_b_is_inf, add_a_is_zero, add_b_is_zero;
    logic add_a_sign, add_b_sign, add_result_sign;
    logic [7:0] add_a_exp, add_b_exp, add_result_exp;
    logic [23:0] add_a_mant, add_b_mant;
    logic [24:0] add_result_mant;
    logic [7:0] add_exp_diff;
    logic [7:0] add_norm_shift;
    
    always_comb begin
        // Classification for addition
        add_a_is_nan = (product_signed[30:23] == 8'hFF) && (product_signed[22:0] != 23'h0);
        add_b_is_nan = (c[30:23] == 8'hFF) && (c[22:0] != 23'h0);
        add_a_is_inf = (product_signed[30:23] == 8'hFF) && (product_signed[22:0] == 23'h0);
        add_b_is_inf = (c[30:23] == 8'hFF) && (c[22:0] == 23'h0);
        add_a_is_zero = (product_signed[30:0] == 31'h0);
        add_b_is_zero = (c[30:0] == 31'h0);
        
        // Initialize all signals to avoid latches
        add_flags = 5'b0;
        result = POS_ZERO;
        add_a_sign = 1'b0;
        add_b_sign = 1'b0;
        add_result_sign = 1'b0;
        add_a_exp = 8'b0;
        add_b_exp = 8'b0;
        add_result_exp = 8'b0;
        add_a_mant = 24'b0;
        add_b_mant = 24'b0;
        add_result_mant = 25'b0;
        add_exp_diff = 8'b0;
        add_norm_shift = 0;
        
        // Handle special cases
        if (add_a_is_nan || add_b_is_nan) begin
            result = QNAN;
        end else if (add_a_is_inf && add_b_is_inf) begin
            if (product_signed[31] != (c[31] ^ negate_addend)) begin
                result = QNAN;
            end else begin
                result = product_signed;
            end
        end else if (add_a_is_inf) begin
            result = product_signed;
        end else if (add_b_is_inf) begin
            result = negate_addend ? {~c[31], c[30:0]} : c;
        end else if (add_a_is_zero) begin
            result = negate_addend ? {~c[31], c[30:0]} : c;
        end else if (add_b_is_zero) begin
            result = product_signed;
        end else begin
            // Normal addition/subtraction
            add_a_sign = product_signed[31];
            add_b_sign = c[31] ^ negate_addend;
            add_a_exp = product_signed[30:23];
            add_b_exp = c[30:23];
            add_a_mant = {1'b1, product_signed[22:0]};
            add_b_mant = {1'b1, c[22:0]};
            
            // Align mantissas
            if (add_a_exp > add_b_exp) begin
                add_exp_diff = add_a_exp - add_b_exp;
                if (add_exp_diff < 24) begin
                    add_b_mant = add_b_mant >> add_exp_diff;
                end else begin
                    add_b_mant = 24'h0;
                end
                add_result_exp = add_a_exp;
            end else begin
                add_exp_diff = add_b_exp - add_a_exp;
                if (add_exp_diff < 24) begin
                    add_a_mant = add_a_mant >> add_exp_diff;
                end else begin
                    add_a_mant = 24'h0;
                end
                add_result_exp = add_b_exp;
            end
            
            // Add or subtract
            if (add_a_sign == add_b_sign) begin
                add_result_mant = add_a_mant + add_b_mant;
                add_result_sign = add_a_sign;
                
                // Normalize if overflow
                if (add_result_mant[24]) begin
                    add_result_mant = add_result_mant >> 1;
                    add_result_exp = add_result_exp + 1;
                end
            end else begin
                if (add_a_mant >= add_b_mant) begin
                    add_result_mant = add_a_mant - add_b_mant;
                    add_result_sign = add_a_sign;
                end else begin
                    add_result_mant = add_b_mant - add_a_mant;
                    add_result_sign = add_b_sign;
                end
                
                // Normalize - find leading one (replace while loop with priority encoder)
                add_norm_shift = 0;
                if (add_result_mant != 0) begin
                    // Priority encoder to find MSB position
                    if (!add_result_mant[23]) begin
                        if (add_result_mant[22:16] != 0) begin
                            for (int i = 22; i >= 16; i--) begin
                                if (add_result_mant[i] && add_norm_shift == 0) begin
                                    add_norm_shift = 8'(23 - i);
                                end
                            end
                        end else if (add_result_mant[15:8] != 0) begin
                            for (int i = 15; i >= 8; i--) begin
                                if (add_result_mant[i] && add_norm_shift == 0) begin
                                    add_norm_shift = 8'(23 - i);
                                end
                            end
                        end else begin
                            for (int i = 7; i >= 0; i--) begin
                                if (add_result_mant[i] && add_norm_shift == 0) begin
                                    add_norm_shift = 8'(23 - i);
                                end
                            end
                        end
                        
                        // Apply normalization
                        if (add_norm_shift <= add_result_exp) begin
                            add_result_mant = add_result_mant << add_norm_shift;
                            add_result_exp = add_result_exp - 8'(add_norm_shift);
                        end else begin
                            // Underflow to zero
                            add_result_mant = 0;
                        end
                    end
                end
            end
            
            // Return result
            if (add_result_mant == 0) begin
                result = POS_ZERO;
            end else begin
                result = {add_result_sign, add_result_exp, add_result_mant[22:0]};
            end
        end
    end
    
    // Combine flags (prioritize mul flags, then add flags)
    assign flags = mul_flags | add_flags;

endmodule
`default_nettype wire

`default_nettype none
// FPU Division Assemble Module
// Assembles the final result after division completes

module fpu_div_assemble (
    input wire logic [31:0] a,
    input wire logic [31:0] b,
    input wire logic [47:0] quotient_raw,
    output logic [31:0] result,
    output logic [4:0]  flags
);

    localparam MANT_MSB_POS = 24;
    
    logic result_sign;
    logic [8:0] result_exp_wide;
    logic [7:0] result_exp;
    logic [47:0] quotient;
    logic [22:0] result_mant;
    logic [47:0] normalized_quotient;
    logic [7:0] msb_pos;  // 8 bits to hold values 0-47 and avoid truncation warnings
    logic signed [8:0] shift;  // 9 bits to handle negative values and proper width
    
    always_comb begin
        // Initialize all variables to avoid latches
        flags = 5'b0;
        result_mant = 23'h0;
        normalized_quotient = 48'h0;
        msb_pos = 0;
        shift = 0;
        result_exp = 8'h0;
        result = 32'h0;
        
        result_sign = a[31] ^ b[31];
        result_exp_wide = {1'b0, a[30:23]} - {1'b0, b[30:23]} + 9'd127;
        quotient = quotient_raw;
        
        // Normalize the quotient
        if (quotient[MANT_MSB_POS]) begin
            // Normal case: MSB at bit MANT_MSB_POS
            result_mant = quotient[MANT_MSB_POS-1:1];
        end else if (quotient[MANT_MSB_POS-1]) begin
            // MSB at bit MANT_MSB_POS-1
            result_mant = quotient[MANT_MSB_POS-2:0];
            result_exp_wide = result_exp_wide - 9'd1;
        end else begin
            // Find MSB position
            for (int i = 47; i >= 0; i--) begin
                if (quotient[i] && msb_pos == 0) begin
                    msb_pos = 8'(i);  // Explicit cast to avoid width truncation
                end
            end
            
            if (msb_pos > 0) begin
                shift = 9'(MANT_MSB_POS) - 9'(msb_pos);  // Explicit width for calculation
                
                if (shift > 0) begin
                    normalized_quotient = quotient << shift;
                    result_exp_wide = result_exp_wide - 9'(shift);
                end else if (shift < 0) begin
                    normalized_quotient = quotient >> 9'(-shift);
                    result_exp_wide = result_exp_wide + 9'(-shift);
                end else begin
                    normalized_quotient = quotient;
                end
                
                result_mant = normalized_quotient[MANT_MSB_POS-1:1];
            end
        end
        
        // Handle underflow/overflow
        if (result_exp_wide[8] && result_exp_wide[7]) begin
            // Underflow -> zero
            result = {result_sign, 31'h0};
        end else if (result_exp_wide >= 255) begin
            // Overflow -> infinity
            flags[2] = 1'b1;  // Overflow flag
            result = {result_sign, 8'hFF, 23'h0};
        end else if (result_exp_wide == 0) begin
            // Denormal -> zero (simplified)
            result = {result_sign, 31'h0};
        end else begin
            result_exp = result_exp_wide[7:0];
            result = {result_sign, result_exp, result_mant};
        end
    end

endmodule
`default_nettype wire

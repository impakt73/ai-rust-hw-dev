`default_nettype none
// FPU Division Setup Module
// Handles special cases and sets up division parameters

module fpu_div_setup (
    input wire logic [31:0] a,
    input wire logic [31:0] b,
    output logic [47:0] dividend,
    output logic [47:0] divisor,
    output logic        needs_div,
    output logic [31:0] special_result,
    output logic [4:0]  flags
);

    localparam [31:0] QNAN = 32'h7FC00000;
    localparam [31:0] POS_INF = 32'h7F800000;
    localparam [31:0] NEG_INF = 32'hFF800000;
    
    logic a_is_nan, b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero;
    
    always_comb begin
        // Initialize outputs
        needs_div = 1'b0;
        dividend = 48'h0;
        divisor = 48'h0;
        flags = 5'b0;
        special_result = 32'h0;
        
        // Classification
        a_is_nan = (a[30:23] == 8'hFF) && (a[22:0] != 23'h0);
        b_is_nan = (b[30:23] == 8'hFF) && (b[22:0] != 23'h0);
        a_is_inf = (a[30:23] == 8'hFF) && (a[22:0] == 23'h0);
        b_is_inf = (b[30:23] == 8'hFF) && (b[22:0] == 23'h0);
        a_is_zero = (a[30:0] == 31'h0);
        b_is_zero = (b[30:0] == 31'h0);
        
        // Handle special cases
        if (a_is_nan || b_is_nan) begin
            special_result = QNAN;
        end else if (b_is_zero) begin
            flags[3] = 1'b1;  // Division by zero flag
            if (a_is_zero) begin
                special_result = QNAN;
            end else begin
                special_result = a[31] ? NEG_INF : POS_INF;
            end
        end else if (a_is_inf) begin
            if (b_is_inf) begin
                special_result = QNAN;
            end else begin
                special_result = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (b_is_inf) begin
            special_result = {a[31] ^ b[31], 31'h0};
        end else if (a_is_zero) begin
            special_result = {a[31] ^ b[31], 31'h0};
        end else begin
            // Normal case: need hardware division
            needs_div = 1'b1;
            // Prepare 48-bit operands for division
            // dividend = {1.mant_a, 24'h0} - shifts mantissa left by 24 bits
            // divisor = {24'h0, 1.mant_b} - mantissa in lower bits
            dividend = {1'b1, a[22:0], 24'h0};
            divisor = {24'h0, 1'b1, b[22:0]};
            special_result = 32'h0;  // Placeholder
        end
    end

endmodule
`default_nettype wire

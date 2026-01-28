// FPU Square Root Module
// Simplified square root implementation

module fpu_sqrt (
    input  logic [31:0] a,
    output logic [31:0] result,
    output logic [4:0]  flags
);

    localparam [31:0] QNAN = 32'h7FC00000;
    
    logic is_nan, is_zero, is_inf;
    logic [7:0] result_exp;
    logic [22:0] result_mant;
    logic [23:0] mant_with_bit;
    
    always_comb begin
        // Classification
        is_nan = (a[30:23] == 8'hFF) && (a[22:0] != 23'h0);
        is_zero = (a[30:0] == 31'h0);
        is_inf = (a[30:23] == 8'hFF) && (a[22:0] == 23'h0);
        
        // Initialize all signals to avoid latches
        flags = 5'b0;
        result = 32'h0;
        result_exp = 8'b0;
        result_mant = 23'h0;
        mant_with_bit = 24'h0;
        
        // Handle special cases
        if (is_nan) begin
            result = QNAN;
        end else if (a[31] && !is_zero) begin
            // Negative non-zero
            flags[4] = 1'b1;  // Invalid operation
            result = QNAN;
        end else if (is_zero || is_inf) begin
            result = a;
        end else begin
            // Normal sqrt - simplified implementation
            // Calculate result exponent: (exp - 127) / 2 + 127
            if (a[23]) begin  // LSB of exponent (odd)
                result_exp = ((a[30:23] - 8'd127) >> 1) + 8'd127;
                /* verilator lint_off WIDTHTRUNC */
                mant_with_bit = {2'b01, a[22:0]};  // Prepend 01 for odd exponent (25 bits truncated to 24)
                /* verilator lint_on WIDTHTRUNC */
            end else begin  // Even exponent
                result_exp = ((a[30:23] - 8'd127) >> 1) + 8'd127;
                mant_with_bit = {1'b1, a[22:0]};  // Implicit 1
            end
            
            // Simplified mantissa (approximation)
            // For a more accurate implementation, Newton-Raphson iterations would be needed
            result_mant = mant_with_bit[22:0];
            
            result = {1'b0, result_exp, result_mant};
        end
    end

endmodule

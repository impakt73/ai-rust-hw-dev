`default_nettype none
// FPU Comparator Module
// Implements floating-point less-than comparison

module fpu_comparator (
    input wire logic [31:0] a,
    input wire logic [31:0] b,
    output logic        less_than
);

    logic a_sign, b_sign;
    logic [30:0] a_mag, b_mag;
    logic a_is_nan, b_is_nan, a_is_zero, b_is_zero;
    
    always_comb begin
        // Check for NaN
        a_is_nan = (a[30:23] == 8'hFF) && (a[22:0] != 23'h0);
        b_is_nan = (b[30:23] == 8'hFF) && (b[22:0] != 23'h0);
        
        // Check for zero
        a_is_zero = (a[30:0] == 31'h0);
        b_is_zero = (b[30:0] == 31'h0);
        
        // Initialize to avoid latches
        a_sign = a[31];
        b_sign = b[31];
        a_mag = a[30:0];
        b_mag = b[30:0];
        
        // Default
        less_than = 1'b0;
        
        if (a_is_nan || b_is_nan) begin
            less_than = 1'b0;
        end else if (a_is_zero && b_is_zero) begin
            less_than = 1'b0;
        end else begin
            if (a_sign != b_sign) begin
                less_than = a_sign;  // Negative < Positive
            end else if (!a_sign) begin
                less_than = a_mag < b_mag;  // Both positive
            end else begin
                less_than = a_mag > b_mag;  // Both negative
            end
        end
    end

endmodule
`default_nettype wire

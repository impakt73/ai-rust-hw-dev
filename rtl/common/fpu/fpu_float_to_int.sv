`default_nettype none
// FPU Float to Integer Converter
// Converts IEEE 754 single-precision float to 32-bit integer

module fpu_float_to_int (
    input  logic [31:0] val,
    input  logic        is_signed,
    output logic [31:0] result,
    output logic        invalid
);

    logic sign;
    logic [7:0] exp;
    logic [23:0] mant;
    logic [31:0] temp_result;
    logic signed [8:0] shift;  // Explicit bit width for synthesis (exp range 0-255, shift = exp - 127)
    logic is_nan, is_inf, is_zero;
    
    always_comb begin
        // Classification
        is_nan = (val[30:23] == 8'hFF) && (val[22:0] != 23'h0);
        is_inf = (val[30:23] == 8'hFF) && (val[22:0] == 23'h0);
        is_zero = (val[30:0] == 31'h0);
        
        // Initialize all signals to avoid latches
        sign = val[31];
        exp = val[30:23];
        invalid = 1'b0;
        mant = 24'h0;
        shift = 0;
        temp_result = 32'h0;
        result = 32'h0;
        
        if (is_nan || is_inf) begin
            invalid = 1'b1;
            if (is_nan) begin
                result = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
            end else if (sign) begin
                result = is_signed ? 32'h80000000 : 32'h00000000;
            end else begin
                result = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
            end
        end else if (is_zero) begin
            result = 32'h0;
        end else begin
            mant = {1'b1, val[22:0]};
            shift = 9'(exp) - 9'd127;  // Explicit width for calculation
            
            if (shift < 0) begin
                result = 32'h0;
            end else if (shift > 31) begin
                invalid = 1'b1;
                result = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
            end else begin
                if (shift >= 23) begin
                    temp_result = {8'h0, mant} << (shift - 23);
                end else begin
                    temp_result = {8'h0, mant} >> (23 - shift);
                end
                
                if (sign) begin
                    if (!is_signed) begin
                        invalid = 1'b1;
                        result = 32'h0;
                    end else begin
                        result = -temp_result;
                    end
                end else begin
                    result = temp_result;
                end
            end
        end
    end

endmodule
`default_nettype wire

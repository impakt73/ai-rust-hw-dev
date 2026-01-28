// FPU Integer to Float Converter
// Converts 32-bit integer to IEEE 754 single-precision float

module fpu_int_to_float (
    input  logic [31:0] val,
    input  logic        is_signed,
    output logic [31:0] result
);

    localparam [31:0] POS_ZERO = 32'h00000000;
    
    logic sign;
    logic [31:0] abs_val;
    logic [7:0] exp;
    logic [22:0] mant;
    logic [7:0] msb_pos;  // 8 bits to hold values 0-31 and avoid bit selection warnings
    
    always_comb begin
        // Initialize all signals to avoid latches
        result = POS_ZERO;
        sign = 1'b0;
        abs_val = 32'h0;
        exp = 8'b0;
        mant = 23'h0;
        msb_pos = 0;
        
        if (val == 32'h0) begin
            result = POS_ZERO;
        end else begin
            // Determine sign and absolute value
            if (is_signed && val[31]) begin
                sign = 1'b1;
                abs_val = -val;
            end else begin
                sign = 1'b0;
                abs_val = val;
            end
            
            // Find MSB position (priority encoder)
            msb_pos = 0;
            for (int i = 31; i >= 0; i--) begin
                if (abs_val[i] && msb_pos == 0) begin
                    msb_pos = 8'(i);  // Explicit cast to avoid width truncation
                end
            end
            
            // Calculate exponent: 127 (bias) + position of MSB
            exp = 8'd127 + msb_pos;
            
            // Extract mantissa - get bits below MSB
            if (msb_pos >= 23) begin
                mant = abs_val[(msb_pos-1) -: 23];
            end else begin
                // Shift left to fill 23 bits
                mant = abs_val[22:0] << (23 - msb_pos);
            end
            
            result = {sign, exp, mant};
        end
    end

endmodule

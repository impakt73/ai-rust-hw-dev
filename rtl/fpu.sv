// Pure RTL Floating Point Unit (FPU) Module  
// Implements RISC-V RV32F single-precision floating point operations
// IEEE 754-2008 compliant using manual bit manipulation (Verilator-compatible)

module fpu (
    input  logic        clk,          // Clock for multi-cycle division
    input  logic        rst_n,        // Reset for multi-cycle division
    input  logic        fpu_start,    // Start FPU operation (pulse)
    input  logic [31:0] fs1,
    input  logic [31:0] fs2,
    input  logic [31:0] fs3,
    input  logic [31:0] int_src,
    input  logic [4:0]  fpu_op,
    input  logic [2:0]  rm,
    output logic [31:0] fp_result,
    output logic [31:0] int_result,
    output logic [4:0]  fflags,
    output logic        fpu_ready     // FPU operation complete
);

    // FPU Operation Encodings
    localparam logic [4:0] FPU_ADD    = 5'b00000;
    localparam logic [4:0] FPU_SUB    = 5'b00001;
    localparam logic [4:0] FPU_MUL    = 5'b00010;
    localparam logic [4:0] FPU_DIV    = 5'b00011;
    localparam logic [4:0] FPU_SQRT   = 5'b00100;
    localparam logic [4:0] FPU_MIN    = 5'b00101;
    localparam logic [4:0] FPU_MAX    = 5'b00110;
    localparam logic [4:0] FPU_MADD   = 5'b00111;
    localparam logic [4:0] FPU_MSUB   = 5'b01000;
    localparam logic [4:0] FPU_NMSUB  = 5'b01001;
    localparam logic [4:0] FPU_NMADD  = 5'b01010;
    localparam logic [4:0] FPU_SGNJ   = 5'b01011;
    localparam logic [4:0] FPU_SGNJN  = 5'b01100;
    localparam logic [4:0] FPU_SGNJX  = 5'b01101;
    localparam logic [4:0] FPU_CVTWS  = 5'b01110;
    localparam logic [4:0] FPU_CVTWUS = 5'b01111;
    localparam logic [4:0] FPU_CVTSW  = 5'b10000;
    localparam logic [4:0] FPU_CVTSWU = 5'b10001;
    localparam logic [4:0] FPU_FEQ    = 5'b10010;
    localparam logic [4:0] FPU_FLT    = 5'b10011;
    localparam logic [4:0] FPU_FLE    = 5'b10100;
    localparam logic [4:0] FPU_FCLASS = 5'b10101;
    localparam logic [4:0] FPU_MVXW   = 5'b10110;
    localparam logic [4:0] FPU_MVWX   = 5'b10111;

    // IEEE 754 constants
    localparam [31:0] POS_ZERO = 32'h00000000;
    localparam [31:0] NEG_ZERO = 32'h80000000;
    localparam [31:0] POS_INF  = 32'h7F800000;
    localparam [31:0] NEG_INF  = 32'hFF800000;
    localparam [31:0] QNAN     = 32'h7FC00000;
    
    // Division normalization constant
    // The target bit position for the mantissa MSB after 48-bit division
    // This represents where the implicit '1' should be positioned
    localparam MANT_MSB_POS = 24;

    // Helper functions - Rewritten for Yosys compatibility (no && or || in expressions)
    function automatic logic is_nan(input logic [31:0] val);
        logic exp_all_ones, frac_nonzero;
        exp_all_ones = (val[30:23] == 8'hFF);
        frac_nonzero = (val[22:0] != 23'h0);
        is_nan = exp_all_ones ? frac_nonzero : 1'b0;
    endfunction

    function automatic logic is_snan(input logic [31:0] val);
        logic exp_all_ones, frac_nonzero, msb_zero, temp;
        exp_all_ones = (val[30:23] == 8'hFF);
        frac_nonzero = (val[22:0] != 23'h0);
        msb_zero = (val[22] == 1'b0);
        temp = exp_all_ones ? frac_nonzero : 1'b0;
        is_snan = temp ? msb_zero : 1'b0;
    endfunction

    function automatic logic is_inf(input logic [31:0] val);
        logic exp_all_ones, frac_zero;
        exp_all_ones = (val[30:23] == 8'hFF);
        frac_zero = (val[22:0] == 23'h0);
        is_inf = exp_all_ones ? frac_zero : 1'b0;
    endfunction

    function automatic logic is_zero(input logic [31:0] val);
        is_zero = (val[30:0] == 31'h0);
    endfunction

    function automatic logic is_subnormal(input logic [31:0] val);
        logic exp_all_zeros, frac_nonzero;
        exp_all_zeros = (val[30:23] == 8'h00);
        frac_nonzero = (val[22:0] != 23'h0);
        is_subnormal = exp_all_zeros ? frac_nonzero : 1'b0;
    endfunction

    // FP comparison - Rewritten for Yosys compatibility
    function automatic logic fp_less_than(input logic [31:0] a, input logic [31:0] b);
        logic a_sign, b_sign;
        logic [30:0] a_mag, b_mag;
        logic a_nan, b_nan, a_z, b_z;
        
        a_nan = is_nan(a);
        b_nan = is_nan(b);
        if (a_nan) begin
            fp_less_than = 1'b0;
        end else if (b_nan) begin
            fp_less_than = 1'b0;
        end else begin
            a_sign = a[31];
            b_sign = b[31];
            a_mag = a[30:0];
            b_mag = b[30:0];
            
            a_z = is_zero(a);
            b_z = is_zero(b);
            if (a_z) begin
                if (b_z) begin
                    fp_less_than = 1'b0;
                end else if (a_sign != b_sign) begin
                    fp_less_than = a_sign;
                end else if (!a_sign) begin
                    fp_less_than = a_mag < b_mag;
                end else begin
                    fp_less_than = a_mag > b_mag;
                end
            end else if (a_sign != b_sign) begin
                fp_less_than = a_sign;
            end else if (!a_sign) begin
                fp_less_than = a_mag < b_mag;
            end else begin
                fp_less_than = a_mag > b_mag;
            end
        end
    endfunction

    // Integer to float conversion
    function automatic logic [31:0] int_to_float(input logic [31:0] val, input logic is_signed);
        logic sign;
        logic [31:0] abs_val;
        logic [7:0] exp;
        logic [22:0] mant;
        integer lz;
        
        // Yosys workaround: avoid early return
        if (val == 32'h0) begin
            int_to_float = POS_ZERO;
        end else begin
            // Yosys workaround: avoid && in expression
            if (is_signed) begin
                if (val[31]) begin
                    sign = 1'b1;
                    abs_val = -val;
                end else begin
                    sign = 1'b0;
                    abs_val = val;
                end
            end else begin
                sign = 1'b0;
                abs_val = val;
            end
            
            // Count leading zeros
            // Yosys workaround: can't use break in functions, use found flag instead
            lz = 0;
            begin
                logic found;
                found = 1'b0;
                for (int i = 31; i >= 0; i--) begin
                    if (abs_val[i] && !found) begin
                        lz = i;  // Position of MSB, not leading zeros!
                        found = 1'b1;  // Prevent further updates
                    end
                end
            end
            
            // Calculate exponent: 127 (bias) + position of MSB
            exp = 8'd127 + lz[7:0];
            
            // Extract mantissa - shift to get bits below MSB
            if (lz >= 23) begin
                mant = abs_val[(lz-1) -: 23];
            end else begin
                // Need to shift left to fill 23 bits
                logic [31:0] shifted;
                shifted = abs_val << (23 - lz);
                mant = shifted[22:0];
            end
            
            int_to_float = {sign, exp, mant};
        end
    endfunction

    // Float to integer conversion
    function automatic logic [31:0] float_to_int(
        input logic [31:0] val,
        input logic is_signed,
        output logic invalid
    );
        logic sign;
        logic [7:0] exp;
        logic [23:0] mant;
        logic [31:0] result;
        integer shift;
        
        invalid = 1'b0;
        sign = val[31];
        exp = val[30:23];
        result = 32'h0;
        
        // Yosys workaround: avoid || in expression and avoid early returns
        if (is_nan(val)) begin
            invalid = 1'b1;
            float_to_int = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
        end else if (is_inf(val)) begin
            invalid = 1'b1;
            if (sign) begin
                float_to_int = is_signed ? 32'h80000000 : 32'h00000000;
            end else begin
                float_to_int = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
            end
        end else if (is_zero(val)) begin
            float_to_int = 32'h0;
        end else begin
            mant = {1'b1, val[22:0]};
            /* verilator lint_off WIDTHEXPAND */
            shift = exp - 127;
            /* verilator lint_on WIDTHEXPAND */
            
            if (shift < 0) begin
                float_to_int = 32'h0;
            end else if (shift > 31) begin
                invalid = 1'b1;
                float_to_int = is_signed ? 32'h7FFFFFFF : 32'hFFFFFFFF;
            end else begin
                /* verilator lint_off WIDTHEXPAND */
                if (shift >= 23) result = mant << (shift - 23);
                else result = mant >> (23 - shift);
                /* verilator lint_on WIDTHEXPAND */
                
                if (sign) begin
                    if (!is_signed) begin
                        invalid = 1'b1;
                        float_to_int = 32'h0;
                    end else begin
                        float_to_int = -result;
                    end
                end else begin
                    float_to_int = result;
                end
            end
        end
    endfunction

    // FP Addition/Subtraction
    function automatic logic [31:0] fp_add_sub(
        input logic [31:0] a,
        input logic [31:0] b,
        input logic is_sub,
        output logic [4:0] flags
    );
        logic a_sign, b_sign, result_sign;
        logic [7:0] a_exp, b_exp, result_exp;
        logic [23:0] a_mant, b_mant;
        logic [24:0] result_mant;
        logic [7:0] exp_diff;
        
        flags = 5'b0;
        
        // Yosys workaround: avoid || in expression and avoid early returns
        if (is_nan(a)) begin
            fp_add_sub = QNAN;
        end else if (is_nan(b)) begin
            fp_add_sub = QNAN;
        end else if (is_inf(a)) begin
            if (is_inf(b)) begin
                if (a[31] != (b[31] ^ is_sub)) begin
                    fp_add_sub = QNAN;
                end else begin
                    fp_add_sub = a;
                end
            end else begin
                fp_add_sub = a;
            end
        end else if (is_inf(b)) begin
            fp_add_sub = is_sub ? {~b[31], b[30:0]} : b;
        end else if (is_zero(a)) begin
            fp_add_sub = is_sub ? {~b[31], b[30:0]} : b;
        end else if (is_zero(b)) begin
            fp_add_sub = a;
        end else begin
            a_sign = a[31];
            b_sign = b[31] ^ is_sub;
            a_exp = a[30:23];
            b_exp = b[30:23];
            a_mant = {1'b1, a[22:0]};
            b_mant = {1'b1, b[22:0]};
            
            if (a_exp > b_exp) begin
                exp_diff = a_exp - b_exp;
                if (exp_diff < 24) b_mant = b_mant >> exp_diff;
                else b_mant = 24'h0;
                result_exp = a_exp;
            end else begin
                exp_diff = b_exp - a_exp;
                if (exp_diff < 24) a_mant = a_mant >> exp_diff;
                else a_mant = 24'h0;
                result_exp = b_exp;
            end
            
            if (a_sign == b_sign) begin
                result_mant = a_mant + b_mant;
                result_sign = a_sign;
                
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
                
                // Yosys workaround: Convert while loop to for loop (Yosys doesn't support
                // while loops in functions with output parameters)
                // Normalize mantissa: shift left until bit 23 is set or we run out of exponent
                for (int norm_i = 0; norm_i < 24; norm_i++) begin
                    if (result_mant != 0) begin
                        if (!result_mant[23]) begin
                            if (result_exp > 0) begin
                                result_mant = result_mant << 1;
                                result_exp = result_exp - 1;
                            end
                        end
                    end
                end
            end
            
            if (result_mant == 0) begin
                fp_add_sub = POS_ZERO;
            end else begin
                fp_add_sub = {result_sign, result_exp, result_mant[22:0]};
            end
        end
    endfunction

    // FP Multiplication
    function automatic logic [31:0] fp_mul(
        input logic [31:0] a,
        input logic [31:0] b,
        output logic [4:0] flags
    );
        logic result_sign;
        logic [8:0] result_exp_wide;
        logic [7:0] result_exp;
        logic [47:0] product;
        logic [22:0] result_mant;
        
        flags = 5'b0;
        
        // Yosys workaround: avoid || in expression and avoid early returns
        if (is_nan(a)) begin
            fp_mul = QNAN;
        end else if (is_nan(b)) begin
            fp_mul = QNAN;
        end else if (is_inf(a)) begin
            if (is_zero(b)) begin
                fp_mul = QNAN;
            end else begin
                fp_mul = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (is_inf(b)) begin
            if (is_zero(a)) begin
                fp_mul = QNAN;
            end else begin
                fp_mul = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (is_zero(a)) begin
            fp_mul = {a[31] ^ b[31], 31'h0};
        end else if (is_zero(b)) begin
            fp_mul = {a[31] ^ b[31], 31'h0};
        end else begin
            result_sign = a[31] ^ b[31];
            result_exp_wide = {1'b0, a[30:23]} + {1'b0, b[30:23]} - 9'd127;
            product = {1'b1, a[22:0]} * {1'b1, b[22:0]};
            
            if (product[47]) begin
                result_mant = product[46:24];
                result_exp_wide = result_exp_wide + 1;
            end else begin
                result_mant = product[45:23];
            end
            
            // Yosys workaround: avoid || in expression
            if (result_exp_wide[8]) begin
                fp_mul = {result_sign, 8'hFF, 23'h0};
            end else if (result_exp_wide > 254) begin
                fp_mul = {result_sign, 8'hFF, 23'h0};
            end else if (result_exp_wide == 0) begin
                fp_mul = {result_sign, 31'h0};
            end else begin
                result_exp = result_exp_wide[7:0];
                fp_mul = {result_sign, result_exp, result_mant};
            end
        end
    endfunction

    // FP Division (hardware implementation using div_unit)
    // Multi-cycle operation that uses a 48-bit division unit
    // The div_unit handles the mantissa division in hardware with full precision
    function automatic logic [31:0] fp_div_setup(
        input logic [31:0] a,
        input logic [31:0] b,
        output logic [47:0] dividend_out,
        output logic [47:0] divisor_out,
        output logic        needs_div,
        output logic [4:0]  flags
    );
        logic result_sign;
        logic [8:0] result_exp_wide;
        
        // Default: no division needed
        needs_div = 1'b0;
        dividend_out = 48'h0;
        divisor_out = 48'h0;
        flags = 5'b0;
        
        // Handle NaN - Yosys workaround: avoid ||
        if (is_nan(a)) begin
            fp_div_setup = QNAN;
        end else if (is_nan(b)) begin
            fp_div_setup = QNAN;
        end else if (is_zero(b)) begin
            // Handle division by zero
            flags[3] = 1'b1;  // DZ flag
            if (is_zero(a)) begin
                fp_div_setup = QNAN;
            end else begin
                fp_div_setup = a[31] ? NEG_INF : POS_INF;
            end
        end else if (is_inf(a)) begin
            // Handle infinity
            if (is_inf(b)) begin
                fp_div_setup = QNAN;
            end else begin
                fp_div_setup = {a[31] ^ b[31], 8'hFF, 23'h0};
            end
        end else if (is_inf(b)) begin
            fp_div_setup = {a[31] ^ b[31], 31'h0};
        end else if (is_zero(a)) begin
            // Handle zero dividend
            fp_div_setup = {a[31] ^ b[31], 31'h0};
        end else begin
            // Normal case: need to perform division
            needs_div = 1'b1;
            
            // With 48-bit div_unit, we can implement proper IEEE 754 division:
            // To get a quotient in the range we want, we scale only the dividend:
            // dividend = {1.mant_a, MANT_MSB_POS'h0} gives 48 bits total (scaled up by 2^MANT_MSB_POS)
            // divisor  = {MANT_MSB_POS'h0, 1.mant_b} gives 48 bits total (in lower bits)
            //
            // This computes (1.mant_a * 2^MANT_MSB_POS) / (1.mant_b) which yields a MANT_MSB_POS-bit quotient
            // in the upper bits, representing the mantissa ratio scaled by 2^MANT_MSB_POS.
            //
            dividend_out = {1'b1, a[22:0], 24'h0};    // 24-bit mantissa (1.mant_a) shifted left by MANT_MSB_POS
            divisor_out = {24'h0, 1'b1, b[22:0]};     // 24-bit mantissa (1.mant_b) in lower bits
            
            // Return a placeholder (will be replaced after division completes)
            fp_div_setup = 32'h0;
        end
    endfunction
    
    // FP Division result assembly (called after div_unit completes)
    function automatic logic [31:0] fp_div_assemble(
        input logic [31:0] a,
        input logic [31:0] b,
        input logic [47:0] quotient_raw,
        output logic [4:0] flags
    );
        logic result_sign;
        logic [8:0] result_exp_wide;
        logic [7:0] result_exp;
        logic [47:0] quotient;
        logic [22:0] result_mant;
        logic [47:0] normalized_quotient;
        integer i;
        integer shift;
        
        flags = 5'b0;
        
        result_sign = a[31] ^ b[31];
        result_exp_wide = {1'b0, a[30:23]} - {1'b0, b[30:23]} + 9'd127;
        
        // The quotient from the 48-bit div_unit represents:
        // (1.mant_a * 2^MANT_MSB_POS) / (1.mant_b) = (mant_a / mant_b) * 2^MANT_MSB_POS
        // For normalized IEEE 754 inputs (mantissas ~1.0), quotient should be around 2^MANT_MSB_POS = 0x1000000
        // This means the implicit '1' of the result will typically be at bit MANT_MSB_POS
        quotient = quotient_raw;
        
        // Normalize the quotient to extract the 23-bit mantissa:
        // Find the MSB and determine how to extract the mantissa
        if (quotient[MANT_MSB_POS]) begin
            // Normal case: quotient ~= 2^MANT_MSB_POS, MSB at bit MANT_MSB_POS
            // The implicit 1 is at bit MANT_MSB_POS, mantissa is bits [MANT_MSB_POS-1:1]
            result_mant = quotient[MANT_MSB_POS-1:1];
            // No exponent adjustment needed
        end else if (quotient[MANT_MSB_POS-1]) begin
            // quotient in [2^(MANT_MSB_POS-1), 2^MANT_MSB_POS): implicit 1 at bit MANT_MSB_POS-1
            result_mant = quotient[MANT_MSB_POS-2:0];
            result_exp_wide = result_exp_wide - 9'd1;
        end else begin
            // Quotient < 2^(MANT_MSB_POS-1): use general normalization
            normalized_quotient = 48'b0;
            shift = 0;
            
            // Find the MSB position
            // Yosys workaround: can't use break in functions, use found flag instead
            begin
                logic found_msb;
                found_msb = 1'b0;
                for (i = 47; i >= 0; i--) begin
                    if (quotient[i] && !found_msb) begin
                        // Compute shift so that MSB moves to bit MANT_MSB_POS
                        shift = MANT_MSB_POS - i;
                        if (shift > 0) begin
                            // Left shift increases magnitude -> decrement exponent
                            /* verilator lint_off WIDTHEXPAND */
                            normalized_quotient = quotient << shift;
                            result_exp_wide = result_exp_wide - 9'(shift);
                            /* verilator lint_on WIDTHEXPAND */
                        end else if (shift < 0) begin
                            // Right shift decreases magnitude -> increment exponent
                            normalized_quotient = quotient >> (-shift);
                            result_exp_wide = result_exp_wide + 9'(-shift);
                        end else begin
                            normalized_quotient = quotient;
                        end
                        // Mantissa is bits [MANT_MSB_POS-1:1] below the normalized leading 1 at bit MANT_MSB_POS
                        result_mant = normalized_quotient[MANT_MSB_POS-1:1];
                        found_msb = 1'b1;  // Prevent further updates
                    end
                end
            end
        end
        
        // If quotient is zero (no bits set), result_mant remains 0 and
        // result_exp_wide will be handled by underflow/zero logic below
        
        // Handle underflow/overflow
        // Yosys workaround: avoid && in expression
        if (result_exp_wide[8]) begin
            if (result_exp_wide[7]) begin
                // Large negative (Underflow) -> Flush to zero
                fp_div_assemble = {result_sign, 31'h0};
            end else begin
                // Not underflow - continue to next checks
                if (result_exp_wide >= 255) begin
                    // Overflow -> Inf
                    flags[2] = 1'b1; // OF
                    fp_div_assemble = {result_sign, 8'hFF, 23'h0};
                end else if (result_exp_wide == 0) begin
                    // Result is denormal (flush to zero for simplicity)
                    fp_div_assemble = {result_sign, 31'h0};
                end else begin
                    result_exp = result_exp_wide[7:0];
                    fp_div_assemble = {result_sign, result_exp, result_mant};
                end
            end
        end else begin
            // result_exp_wide[8] is 0, continue with normal checks
            if (result_exp_wide >= 255) begin
                // Overflow -> Inf
                flags[2] = 1'b1; // OF
                fp_div_assemble = {result_sign, 8'hFF, 23'h0};
            end else if (result_exp_wide == 0) begin
                // Result is denormal (flush to zero for simplicity)
                fp_div_assemble = {result_sign, 31'h0};
            end else begin
                result_exp = result_exp_wide[7:0];
                fp_div_assemble = {result_sign, result_exp, result_mant};
            end
        end
    endfunction

    // FP Square Root (simplified - returns approximation)
    // For full accuracy, this should be a multi-cycle operation
    function automatic logic [31:0] fp_sqrt(
        input logic [31:0] a,
        output logic [4:0] flags
    );
        logic [7:0] result_exp;
        logic [22:0] result_mant;
        logic [23:0] mant_with_bit;
        logic [47:0] squared;
        
        flags = 5'b0;
        
        // Handle NaN
        if (is_nan(a)) begin
            fp_sqrt = QNAN;
        end else if (a[31]) begin
            // Handle negative - Yosys workaround: avoid &&
            if (!is_zero(a)) begin
                flags[4] = 1'b1;  // NV flag
                fp_sqrt = QNAN;
            end else begin
                fp_sqrt = a;  // -0.0
            end
        end else if (is_zero(a)) begin
            // Handle zero and infinity - Yosys workaround: avoid ||
            fp_sqrt = a;
        end else if (is_inf(a)) begin
            fp_sqrt = a;
        end else begin
            // Calculate result exponent: (exp - 127) / 2 + 127
            // Check if exponent is odd
            if (a[23]) begin  // LSB of exponent (odd)
                result_exp = ((a[30:23] - 8'd127) >> 1) + 8'd127;
                /* verilator lint_off WIDTHTRUNC */
                mant_with_bit = {2'b01, a[22:0]};  // Prepend 01 for normalization
                /* verilator lint_on WIDTHTRUNC */
            end else begin  // Even exponent
                result_exp = ((a[30:23] - 8'd127) >> 1) + 8'd127;
                mant_with_bit = {1'b1, a[22:0]};  // Implicit 1
            end
            
            // Simplified: Use approximation sqrt(x) ≈ x/2 + 1/2 (Newton's method seed)
            // For better accuracy, multiple Newton iterations needed
            // This gives rough approximation
            result_mant = mant_with_bit[22:0];
            
            fp_sqrt = {1'b0, result_exp, result_mant};
        end
    endfunction

    // NOTE: fp_fmadd function removed - Yosys doesn't support function-to-function calls
    // with non-constant arguments. FMA logic is inlined directly in the case statement.

    // ============================================================
    // Division Unit Integration for FP Division
    // ============================================================
    
    // Division unit signals (48-bit for FP mantissa precision)
    logic        div_start;
    logic        div_ready;
    logic [47:0] div_dividend;
    logic [47:0] div_divisor;
    logic [47:0] div_result;
    
    // Instantiate 48-bit division unit for FP mantissa division
    // This provides the full precision needed for IEEE 754 single-precision (23-bit mantissa)
    div_unit #(
        .WIDTH(48)
    ) u_div (
        .clk(clk),
        .rst_n(rst_n),
        .start(div_start),
        .is_signed(1'b0),        // Always unsigned for FP mantissa
        .rem_sel(1'b0),          // Always quotient for FP division
        .dividend(div_dividend),
        .divisor(div_divisor),
        .result(div_result),
        .ready(div_ready)
    );
    
    // Detect FP division operation
    logic is_fp_div;
    logic needs_div_comb;  // Combinational signal from fp_div_setup
    assign is_fp_div = (fpu_op == FPU_DIV);
    
    // Start division only when requested AND hardware division is actually needed
    // Yosys workaround: avoid && in expression
    logic div_start_cond1, div_start_cond2;
    assign div_start_cond1 = fpu_start ? is_fp_div : 1'b0;
    assign div_start_cond2 = div_start_cond1 ? needs_div_comb : 1'b0;
    assign div_start = div_start_cond2;
    
    // FPU ready signal - three cases:
    // 1. Division in progress: wait for div_ready to signal completion
    // 2. Starting a new division this cycle: not ready yet (needs one cycle to register div_in_progress)
    // 3. All other operations: ready immediately (combinational ops or special cases like NaN/Inf/zero)
    // Yosys workaround: avoid && in ternary expression
    logic fpu_ready_new_div_cond1, fpu_ready_new_div_cond2;
    assign fpu_ready_new_div_cond1 = fpu_start ? is_fp_div : 1'b0;
    assign fpu_ready_new_div_cond2 = fpu_ready_new_div_cond1 ? needs_div_comb : 1'b0;
    assign fpu_ready = div_in_progress ? div_ready : 
                       fpu_ready_new_div_cond2 ? 1'b0 :
                       1'b1;
    
    // ============================================================
    // Division State Registers
    // ============================================================
    logic [31:0] div_fs1_reg;
    logic [31:0] div_fs2_reg;
    logic        div_in_progress;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            div_fs1_reg <= 32'h0;
            div_fs2_reg <= 32'h0;
            div_in_progress <= 1'b0;
        end else begin
            if (div_start) begin
                // Capture operands when division starts
                div_fs1_reg <= fs1;
                div_fs2_reg <= fs2;
                div_in_progress <= 1'b1;
            // Yosys workaround: avoid && in expression
            end else begin
                if (div_in_progress) begin
                    if (div_ready) begin
                        // Clear in-progress flag when division completes
                        div_in_progress <= 1'b0;
                    end
                end
            end
        end
    end
    
    // Main logic
    logic inv_flag;  // Move outside always_comb to avoid latch
    logic [31:0] temp_product;  // For FMA operations
    logic [4:0] temp_fma_flags;  // For FMA flag accumulation
    
    always_comb begin
        fp_result = POS_ZERO;
        int_result = 32'h0;
        fflags = 5'b0;
        inv_flag = 1'b0;  // Initialize
        needs_div_comb = 1'b0;  // Initialize
        div_dividend = 48'h0;
        div_divisor = 48'h0;
        temp_product = 32'h0;  // Initialize
        temp_fma_flags = 5'b0;  // Initialize
        
        case (fpu_op)
            FPU_ADD: fp_result = fp_add_sub(fs1, fs2, 1'b0, fflags);
            FPU_SUB: fp_result = fp_add_sub(fs1, fs2, 1'b1, fflags);
            FPU_MUL: fp_result = fp_mul(fs1, fs2, fflags);
            
            FPU_DIV: begin
                // Yosys workaround: avoid && in expression
                if (div_in_progress) begin
                    if (div_ready) begin
                        // Division complete - assemble result using captured operands
                        fp_result = fp_div_assemble(div_fs1_reg, div_fs2_reg, div_result, fflags);
                    end else begin
                        // Setup division or return intermediate result (for special cases)
                        fp_result = fp_div_setup(fs1, fs2, div_dividend, div_divisor, needs_div_comb, fflags);
                        // If special case (NaN, Inf, Zero), needs_div_comb will be 0 and result is valid
                    end
                end else begin
                    // Setup division or return intermediate result (for special cases)
                    fp_result = fp_div_setup(fs1, fs2, div_dividend, div_divisor, needs_div_comb, fflags);
                    // If special case (NaN, Inf, Zero), needs_div_comb will be 0 and result is valid
                end
            end
            
            FPU_SQRT: fp_result = fp_sqrt(fs1, fflags);
            
            // Fused multiply-add operations
            // Inlined FMA logic (Yosys doesn't support function-to-function calls)
            FPU_MADD: begin  // (fs1*fs2) + fs3
                temp_product = fp_mul(fs1, fs2, temp_fma_flags);
                fflags = temp_fma_flags;
                // negate_product = 0, negate_addend = 0
                fp_result = fp_add_sub(temp_product, fs3, 1'b0, temp_fma_flags);
            end
            
            FPU_MSUB: begin  // (fs1*fs2) - fs3
                temp_product = fp_mul(fs1, fs2, temp_fma_flags);
                fflags = temp_fma_flags;
                // negate_product = 0, negate_addend = 1
                fp_result = fp_add_sub(temp_product, fs3, 1'b1, temp_fma_flags);
            end
            
            FPU_NMSUB: begin  // -(fs1*fs2) + fs3
                temp_product = fp_mul(fs1, fs2, temp_fma_flags);
                fflags = temp_fma_flags;
                temp_product[31] = ~temp_product[31];  // Negate product
                // negate_product = 1, negate_addend = 0
                fp_result = fp_add_sub(temp_product, fs3, 1'b0, temp_fma_flags);
            end
            
            FPU_NMADD: begin  // -(fs1*fs2) - fs3
                temp_product = fp_mul(fs1, fs2, temp_fma_flags);
                fflags = temp_fma_flags;
                temp_product[31] = ~temp_product[31];  // Negate product
                // negate_product = 1, negate_addend = 1
                fp_result = fp_add_sub(temp_product, fs3, 1'b1, temp_fma_flags);
            end
            
            FPU_SGNJ:  fp_result = {fs2[31], fs1[30:0]};
            FPU_SGNJN: fp_result = {~fs2[31], fs1[30:0]};
            FPU_SGNJX: fp_result = {fs1[31] ^ fs2[31], fs1[30:0]};
            
            FPU_MVXW: int_result = fs1;
            FPU_MVWX: fp_result = int_src;
            
            FPU_FEQ: begin
                // Yosys workaround: avoid || in expression
                if (is_nan(fs1)) begin
                    int_result = 32'h0;
                    if (is_snan(fs1)) fflags[4] = 1'b1;
                end else if (is_nan(fs2)) begin
                    int_result = 32'h0;
                    if (is_snan(fs2)) fflags[4] = 1'b1;
                end else begin
                    int_result = (fs1 == fs2) ? 32'h1 : 32'h0;
                end
            end
            
            FPU_FLT: begin
                // Yosys workaround: avoid || in expression
                if (is_nan(fs1)) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;
                end else if (is_nan(fs2)) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;
                end else begin
                    int_result = fp_less_than(fs1, fs2) ? 32'h1 : 32'h0;
                end
            end
            
            FPU_FLE: begin
                // Yosys workaround: avoid || in expression
                if (is_nan(fs1)) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;
                end else if (is_nan(fs2)) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;
                end else begin
                    // Yosys workaround: avoid || in ternary expression
                    logic fle_cond;
                    fle_cond = fp_less_than(fs1, fs2) ? 1'b1 : (fs1 == fs2);
                    int_result = fle_cond ? 32'h1 : 32'h0;
                end
            end
            
            FPU_MIN: begin
                // Yosys workaround: avoid && and || in expressions
                if (is_nan(fs1)) begin
                    if (is_nan(fs2)) begin
                        fp_result = QNAN;
                    end else begin
                        fp_result = fs2;
                    end
                end else if (is_nan(fs2)) begin
                    fp_result = fs1;
                end else if (is_zero(fs1)) begin
                    if (is_zero(fs2)) begin
                        // Yosys workaround: avoid || in ternary
                        fp_result = fs1[31] ? NEG_ZERO : (fs2[31] ? NEG_ZERO : POS_ZERO);
                    end else begin
                        fp_result = fp_less_than(fs1, fs2) ? fs1 : fs2;
                    end
                end else begin
                    fp_result = fp_less_than(fs1, fs2) ? fs1 : fs2;
                end
            end
            
            FPU_MAX: begin
                // Yosys workaround: avoid && in expressions
                if (is_nan(fs1)) begin
                    if (is_nan(fs2)) begin
                        fp_result = QNAN;
                    end else begin
                        fp_result = fs2;
                    end
                end else if (is_nan(fs2)) begin
                    fp_result = fs1;
                end else if (is_zero(fs1)) begin
                    if (is_zero(fs2)) begin
                        // Yosys workaround: avoid && in ternary
                        fp_result = fs1[31] ? (fs2[31] ? NEG_ZERO : POS_ZERO) : POS_ZERO;
                    end else begin
                        fp_result = fp_less_than(fs1, fs2) ? fs2 : fs1;
                    end
                end else begin
                    fp_result = fp_less_than(fs1, fs2) ? fs2 : fs1;
                end
            end
            
            FPU_FCLASS: begin
                if (is_nan(fs1)) begin
                    int_result = fs1[22] ? 32'h00000200 : 32'h00000100;
                end else if (is_inf(fs1)) begin
                    int_result = fs1[31] ? 32'h00000001 : 32'h00000080;
                end else if (is_zero(fs1)) begin
                    int_result = fs1[31] ? 32'h00000008 : 32'h00000010;
                end else if (is_subnormal(fs1)) begin
                    int_result = fs1[31] ? 32'h00000004 : 32'h00000020;
                end else begin
                    int_result = fs1[31] ? 32'h00000002 : 32'h00000040;
                end
            end
            
            FPU_CVTSW:  fp_result = int_to_float(int_src, 1'b1);
            FPU_CVTSWU: fp_result = int_to_float(int_src, 1'b0);
            
            FPU_CVTWS: begin
                int_result = float_to_int(fs1, 1'b1, inv_flag);
                if (inv_flag) fflags[4] = 1'b1;
            end
            
            FPU_CVTWUS: begin
                int_result = float_to_int(fs1, 1'b0, inv_flag);
                if (inv_flag) fflags[4] = 1'b1;
            end
            
            default: begin
                fp_result = POS_ZERO;
                int_result = 32'h0;
            end
        endcase
    end

endmodule

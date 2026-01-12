// Floating Point Unit (FPU) Module
// Implements RISC-V RV32F single-precision floating point operations
// IEEE 754-2008 compliant

module fpu (
    input  logic [31:0] fs1,         // FP source 1
    input  logic [31:0] fs2,         // FP source 2
    input  logic [31:0] fs3,         // FP source 3 (for fused ops)
    input  logic [31:0] int_src,     // Integer source (for conversions)
    input  logic [4:0]  fpu_op,      // FPU operation selector
    input  logic [2:0]  rm,          // Rounding mode
    output logic [31:0] fp_result,   // FP result
    output logic [31:0] int_result,  // Integer result (for conversions/compares)
    output logic [4:0]  fflags       // Exception flags (NV, DZ, OF, UF, NX)
);

    // FPU Operation Encodings
    localparam logic [4:0] FPU_ADD    = 5'b00000;  // FADD.S
    localparam logic [4:0] FPU_SUB    = 5'b00001;  // FSUB.S
    localparam logic [4:0] FPU_MUL    = 5'b00010;  // FMUL.S
    localparam logic [4:0] FPU_DIV    = 5'b00011;  // FDIV.S
    localparam logic [4:0] FPU_SQRT   = 5'b00100;  // FSQRT.S
    localparam logic [4:0] FPU_MIN    = 5'b00101;  // FMIN.S
    localparam logic [4:0] FPU_MAX    = 5'b00110;  // FMAX.S
    localparam logic [4:0] FPU_MADD   = 5'b00111;  // FMADD.S
    localparam logic [4:0] FPU_MSUB   = 5'b01000;  // FMSUB.S
    localparam logic [4:0] FPU_NMSUB  = 5'b01001;  // FNMSUB.S
    localparam logic [4:0] FPU_NMADD  = 5'b01010;  // FNMADD.S
    localparam logic [4:0] FPU_SGNJ   = 5'b01011;  // FSGNJ.S
    localparam logic [4:0] FPU_SGNJN  = 5'b01100;  // FSGNJN.S
    localparam logic [4:0] FPU_SGNJX  = 5'b01101;  // FSGNJX.S
    localparam logic [4:0] FPU_CVTWS  = 5'b01110;  // FCVT.W.S
    localparam logic [4:0] FPU_CVTWUS = 5'b01111;  // FCVT.WU.S
    localparam logic [4:0] FPU_CVTSW  = 5'b10000;  // FCVT.S.W
    localparam logic [4:0] FPU_CVTSWU = 5'b10001;  // FCVT.S.WU
    localparam logic [4:0] FPU_FEQ    = 5'b10010;  // FEQ.S
    localparam logic [4:0] FPU_FLT    = 5'b10011;  // FLT.S
    localparam logic [4:0] FPU_FLE    = 5'b10100;  // FLE.S
    localparam logic [4:0] FPU_FCLASS = 5'b10101;  // FCLASS.S
    localparam logic [4:0] FPU_MVXW   = 5'b10110;  // FMV.X.W
    localparam logic [4:0] FPU_MVWX   = 5'b10111;  // FMV.W.X

    // Exception flag bits
    // fflags[4]: NV - Invalid operation
    // fflags[3]: DZ - Divide by zero
    // fflags[2]: OF - Overflow
    // fflags[1]: UF - Underflow
    // fflags[0]: NX - Inexact

    // Convert inputs to shortreal (IEEE 754 single precision)
    /* verilator lint_off SHORTREAL */
    shortreal fs1_real, fs2_real, fs3_real;
    shortreal result_real;
    /* verilator lint_on SHORTREAL */
    integer int_temp;
    
    /* verilator lint_off SHORTREAL */
    /* verilator lint_off WIDTHEXPAND */
    /* verilator lint_off WIDTHTRUNC */
    assign fs1_real = $bitstoshortreal(fs1);
    assign fs2_real = $bitstoshortreal(fs2);
    assign fs3_real = $bitstoshortreal(fs3);
    /* verilator lint_on WIDTHTRUNC */
    /* verilator lint_on WIDTHEXPAND */
    /* verilator lint_on SHORTREAL */
    
    // Helper function to check if a float is NaN
    function automatic logic is_nan(input logic [31:0] val);
        return (val[30:23] == 8'hFF) && (val[22:0] != 23'h0);
    endfunction
    
    // Helper function to check if a float is infinity
    function automatic logic is_inf(input logic [31:0] val);
        return (val[30:23] == 8'hFF) && (val[22:0] == 23'h0);
    endfunction
    
    // Helper function to check if a float is zero
    function automatic logic is_zero(input logic [31:0] val);
        return (val[30:0] == 31'h0);
    endfunction
    
    // Helper variables for FCLASS
    logic is_neg_local, is_subnormal_local;
    
    always_comb begin
        // Default values
        fp_result = 32'h00000000;
        int_result = 32'h00000000;
        fflags = 5'b00000;
        result_real = 0.0;
        int_temp = 0;
        
        // Initialize FCLASS helper variables
        is_neg_local = 1'b0;
        is_subnormal_local = 1'b0;
        
        case (fpu_op)
            FPU_ADD: begin
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = fs1_real + fs2_real;
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_SUB: begin
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = fs1_real - fs2_real;
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_MUL: begin
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = fs1_real * fs2_real;
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_DIV: begin
                if (is_zero(fs2)) begin
                    // Division by zero
                    if (is_zero(fs1)) begin
                        fp_result = 32'h7FC00000;  // Canonical NaN
                        fflags[4] = 1'b1;  // Invalid
                    end else begin
                        // Return infinity with appropriate sign
                        fp_result = (fs1[31] ^ fs2[31]) ? 32'hFF800000 : 32'h7F800000;
                        fflags[3] = 1'b1;  // Divide by zero
                    end
                end else begin
                    /* verilator lint_off SHORTREAL */
                    /* verilator lint_off WIDTHTRUNC */
                    result_real = fs1_real / fs2_real;
                    fp_result = $shortrealtobits(result_real);
                    /* verilator lint_on WIDTHTRUNC */
                    /* verilator lint_on SHORTREAL */
                end
            end
            
            FPU_SQRT: begin
                if (fs1[31] && !is_zero(fs1)) begin
                    // Negative number (not -0.0)
                    fp_result = 32'h7FC00000;  // Canonical NaN
                    fflags[4] = 1'b1;  // Invalid
                end else begin
                    /* verilator lint_off SHORTREAL */
                    /* verilator lint_off WIDTHTRUNC */
                    result_real = $sqrt(fs1_real);
                    fp_result = $shortrealtobits(result_real);
                    /* verilator lint_on WIDTHTRUNC */
                    /* verilator lint_on SHORTREAL */
                end
            end
            
            FPU_MIN: begin
                // Handle NaN and signed zero cases
                if (is_nan(fs1) && is_nan(fs2)) begin
                    fp_result = 32'h7FC00000;  // Canonical NaN
                end else if (is_nan(fs1)) begin
                    fp_result = fs2;  // Return non-NaN
                end else if (is_nan(fs2)) begin
                    fp_result = fs1;  // Return non-NaN
                end else begin
                    // Handle -0.0 and +0.0 specially
                    if (is_zero(fs1) && is_zero(fs2)) begin
                        fp_result = (fs1[31] || fs2[31]) ? 32'h80000000 : 32'h00000000;  // Return -0.0 if either is -0.0
                    end else begin
                        /* verilator lint_off SHORTREAL */
                        /* verilator lint_off WIDTHTRUNC */
                        result_real = (fs1_real < fs2_real) ? fs1_real : fs2_real;
                        fp_result = $shortrealtobits(result_real);
                        /* verilator lint_on WIDTHTRUNC */
                        /* verilator lint_on SHORTREAL */
                    end
                end
            end
            
            FPU_MAX: begin
                // Handle NaN and signed zero cases
                if (is_nan(fs1) && is_nan(fs2)) begin
                    fp_result = 32'h7FC00000;  // Canonical NaN
                end else if (is_nan(fs1)) begin
                    fp_result = fs2;  // Return non-NaN
                end else if (is_nan(fs2)) begin
                    fp_result = fs1;  // Return non-NaN
                end else begin
                    // Handle -0.0 and +0.0 specially
                    if (is_zero(fs1) && is_zero(fs2)) begin
                        fp_result = (fs1[31] && fs2[31]) ? 32'h80000000 : 32'h00000000;  // Return +0.0 if either is +0.0
                    end else begin
                        /* verilator lint_off SHORTREAL */
                        /* verilator lint_off WIDTHTRUNC */
                        result_real = (fs1_real > fs2_real) ? fs1_real : fs2_real;
                        fp_result = $shortrealtobits(result_real);
                        /* verilator lint_on WIDTHTRUNC */
                        /* verilator lint_on SHORTREAL */
                    end
                end
            end
            
            FPU_MADD: begin
                // fd = fs1 * fs2 + fs3
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = (fs1_real * fs2_real) + fs3_real;
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_MSUB: begin
                // fd = fs1 * fs2 - fs3
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = (fs1_real * fs2_real) - fs3_real;
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_NMSUB: begin
                // fd = -(fs1 * fs2 - fs3) = fs3 - fs1 * fs2
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = fs3_real - (fs1_real * fs2_real);
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_NMADD: begin
                // fd = -(fs1 * fs2 + fs3)
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = -((fs1_real * fs2_real) + fs3_real);
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_SGNJ: begin
                // Copy sign of fs2 to magnitude of fs1
                fp_result = {fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJN: begin
                // Copy inverted sign of fs2 to magnitude of fs1
                fp_result = {~fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJX: begin
                // XOR signs of fs1 and fs2
                fp_result = {fs1[31] ^ fs2[31], fs1[30:0]};
            end
            
            FPU_CVTWS: begin
                // Float to signed int
                if (is_nan(fs1)) begin
                    int_result = 32'h7FFFFFFF;  // Maximum positive value for NaN
                    fflags[4] = 1'b1;  // Invalid
                end else if (fs1_real > 2147483647.0) begin
                    int_result = 32'h7FFFFFFF;  // Saturate to max
                    fflags[4] = 1'b1;  // Invalid
                end else if (fs1_real < -2147483648.0) begin
                    int_result = 32'h80000000;  // Saturate to min
                    fflags[4] = 1'b1;  // Invalid
                end else begin
                    int_temp = $rtoi(fs1_real);
                    int_result = int_temp[31:0];
                end
            end
            
            FPU_CVTWUS: begin
                // Float to unsigned int
                if (is_nan(fs1) || fs1_real < 0.0) begin
                    int_result = 32'h00000000;  // Saturate to 0 for negative or NaN
                    fflags[4] = 1'b1;  // Invalid
                end else if (fs1_real > 4294967295.0) begin
                    int_result = 32'hFFFFFFFF;  // Saturate to max unsigned
                    fflags[4] = 1'b1;  // Invalid
                end else begin
                    int_temp = $rtoi(fs1_real);
                    int_result = int_temp[31:0];
                end
            end
            
            FPU_CVTSW: begin
                // Signed int to float
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                int_temp = $signed(int_src);
                result_real = $itor(int_temp);
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_CVTSWU: begin
                // Unsigned int to float
                /* verilator lint_off SHORTREAL */
                /* verilator lint_off WIDTHTRUNC */
                result_real = $itor(int_src);
                fp_result = $shortrealtobits(result_real);
                /* verilator lint_on WIDTHTRUNC */
                /* verilator lint_on SHORTREAL */
            end
            
            FPU_FEQ: begin
                // Floating point equal
                // NaN is never equal to anything (including itself)
                if (is_nan(fs1) || is_nan(fs2)) begin
                    int_result = 32'h00000000;
                    if (is_nan(fs1) && fs1[22] == 1'b0) fflags[4] = 1'b1;  // Signaling NaN
                    if (is_nan(fs2) && fs2[22] == 1'b0) fflags[4] = 1'b1;  // Signaling NaN
                end else begin
                    int_result = (fs1 == fs2) ? 32'h00000001 : 32'h00000000;
                end
            end
            
            FPU_FLT: begin
                // Floating point less than
                if (is_nan(fs1) || is_nan(fs2)) begin
                    int_result = 32'h00000000;
                    fflags[4] = 1'b1;  // Invalid for any NaN
                end else begin
                    int_result = (fs1_real < fs2_real) ? 32'h00000001 : 32'h00000000;
                end
            end
            
            FPU_FLE: begin
                // Floating point less than or equal
                if (is_nan(fs1) || is_nan(fs2)) begin
                    int_result = 32'h00000000;
                    fflags[4] = 1'b1;  // Invalid for any NaN
                end else begin
                    int_result = (fs1_real <= fs2_real) ? 32'h00000001 : 32'h00000000;
                end
            end
            
            FPU_FCLASS: begin
                // Classify floating point number
                // Bit 0: negative infinity
                // Bit 1: negative normal
                // Bit 2: negative subnormal
                // Bit 3: negative zero
                // Bit 4: positive zero
                // Bit 5: positive subnormal
                // Bit 6: positive normal
                // Bit 7: positive infinity
                // Bit 8: signaling NaN
                // Bit 9: quiet NaN
                
                is_neg_local = fs1[31];
                is_subnormal_local = (fs1[30:23] == 8'h00) && (fs1[22:0] != 23'h0);
                
                int_result = 32'h00000000;  // Default to avoid latch
                
                if (is_nan(fs1)) begin
                    // Check if signaling or quiet NaN (bit 22 is MSB of mantissa)
                    int_result = fs1[22] ? 32'h00000200 : 32'h00000100;
                end else if (is_inf(fs1)) begin
                    int_result = is_neg_local ? 32'h00000001 : 32'h00000080;
                end else if (is_zero(fs1)) begin
                    int_result = is_neg_local ? 32'h00000008 : 32'h00000010;
                end else if (is_subnormal_local) begin
                    int_result = is_neg_local ? 32'h00000004 : 32'h00000020;
                end else begin
                    // Normal number
                    int_result = is_neg_local ? 32'h00000002 : 32'h00000040;
                end
            end
            
            FPU_MVXW: begin
                // Move FP register to integer register (bitwise)
                int_result = fs1;
            end
            
            FPU_MVWX: begin
                // Move integer register to FP register (bitwise)
                fp_result = int_src;
            end
            
            default: begin
                fp_result = 32'h00000000;
                int_result = 32'h00000000;
                fflags = 5'b00000;
            end
        endcase
    end

endmodule

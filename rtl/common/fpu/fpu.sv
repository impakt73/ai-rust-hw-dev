`default_nettype none
// Pure RTL Floating Point Unit (FPU) Module - Refactored for Yosys Compatibility
// Implements RISC-V RV32F single-precision floating point operations
// IEEE 754-2008 compliant using modular design (no functions)
//
// REFACTORED: All function logic moved to separate modules to ensure
// compatibility with Yosys synthesis (even v0.61 has function limitations)

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
    localparam [31:0] FP_ONE   = 32'h3F800000;  // 1.0f in IEEE 754

    // ============================================================
    // Submodule Wiring
    // ============================================================
    
    // Classifier outputs for fs1 and fs2
    logic fs1_is_nan, fs1_is_snan, fs1_is_inf, fs1_is_zero, fs1_is_subnormal;
    logic fs2_is_nan, fs2_is_snan, fs2_is_inf, fs2_is_zero, fs2_is_subnormal;
    
    // Comparator output
    logic fs1_less_than_fs2;
    
    // Shared FMA inputs and outputs (consolidated from 4+2+1 units to 1 unit)
    logic [31:0] fma_a, fma_b, fma_c;
    logic        fma_negate_product, fma_negate_addend;
    logic [31:0] fma_result;
    logic [4:0]  fma_flags;
    
    // Sqrt output
    logic [31:0] sqrt_result;
    logic [4:0]  sqrt_flags;
    
    // Conversion outputs
    logic [31:0] int_to_float_signed, int_to_float_unsigned;
    logic [31:0] float_to_int_signed, float_to_int_unsigned;
    logic        float_to_int_signed_invalid, float_to_int_unsigned_invalid;
    
    // Division signals
    logic [47:0] div_dividend, div_divisor;
    logic        div_needs_hw;
    logic [31:0] div_special_result;
    logic [4:0]  div_setup_flags;
    logic [31:0] div_assembled_result;
    logic [4:0]  div_assemble_flags;
    
    // ============================================================
    // Submodule Instantiations
    // ============================================================
    
    // Classifiers for fs1 and fs2
    fpu_classifier u_classifier_fs1 (
        .val(fs1),
        .is_nan(fs1_is_nan),
        .is_snan(fs1_is_snan),
        .is_inf(fs1_is_inf),
        .is_zero(fs1_is_zero),
        .is_subnormal(fs1_is_subnormal)
    );
    
    fpu_classifier u_classifier_fs2 (
        .val(fs2),
        .is_nan(fs2_is_nan),
        .is_snan(fs2_is_snan),
        .is_inf(fs2_is_inf),
        .is_zero(fs2_is_zero),
        .is_subnormal(fs2_is_subnormal)
    );
    
    // Comparator
    fpu_comparator u_comparator (
        .a(fs1),
        .b(fs2),
        .less_than(fs1_less_than_fs2)
    );
    
    // ============================================================
    // CONSOLIDATED FMA Unit (Replaces 4 FMA + 2 Adder + 1 Multiplier)
    // ============================================================
    // Routes all arithmetic operations through single FMA:
    // - ADD: (fs1 * 1.0) + fs2
    // - SUB: (fs1 * 1.0) - fs2
    // - MUL: (fs1 * fs2) + 0.0
    // - MADD/MSUB/NMSUB/NMADD: Direct FMA operations
    
    // FMA input multiplexing logic
    always_comb begin
        // Default: Pass-through for FMA operations
        fma_a = fs1;
        fma_b = fs2;
        fma_c = fs3;
        fma_negate_product = 1'b0;
        fma_negate_addend = 1'b0;
        
        case (fpu_op)
            FPU_ADD: begin
                // (fs1 * 1.0) + fs2
                fma_a = fs1;
                fma_b = FP_ONE;
                fma_c = fs2;
                fma_negate_product = 1'b0;
                fma_negate_addend = 1'b0;
            end
            
            FPU_SUB: begin
                // (fs1 * 1.0) - fs2
                fma_a = fs1;
                fma_b = FP_ONE;
                fma_c = fs2;
                fma_negate_product = 1'b0;
                fma_negate_addend = 1'b1;
            end
            
            FPU_MUL: begin
                // (fs1 * fs2) + 0.0
                fma_a = fs1;
                fma_b = fs2;
                fma_c = POS_ZERO;
                fma_negate_product = 1'b0;
                fma_negate_addend = 1'b0;
            end
            
            // FPU_MADD case omitted - uses default values (same as explicit case)
            
            FPU_MSUB: begin
                // (fs1 * fs2) - fs3
                fma_a = fs1;
                fma_b = fs2;
                fma_c = fs3;
                fma_negate_product = 1'b0;
                fma_negate_addend = 1'b1;
            end
            
            FPU_NMSUB: begin
                // -(fs1 * fs2) + fs3
                fma_a = fs1;
                fma_b = fs2;
                fma_c = fs3;
                fma_negate_product = 1'b1;
                fma_negate_addend = 1'b0;
            end
            
            FPU_NMADD: begin
                // -(fs1 * fs2) - fs3
                fma_a = fs1;
                fma_b = fs2;
                fma_c = fs3;
                fma_negate_product = 1'b1;
                fma_negate_addend = 1'b1;
            end
            
            default: begin
                // Default: use initialized values (handles FPU_MADD and other ops)
                // Values already set at lines 134-138
            end
        endcase
    end
    
    // Single shared FMA instance
    fpu_fma u_fma (
        .a(fma_a),
        .b(fma_b),
        .c(fma_c),
        .negate_product(fma_negate_product),
        .negate_addend(fma_negate_addend),
        .result(fma_result),
        .flags(fma_flags)
    );
    
    // Sqrt unit (cannot be emulated via FMA)
    fpu_sqrt u_sqrt (
        .a(fs1),
        .result(sqrt_result),
        .flags(sqrt_flags)
    );
    
    // Conversion operations
    fpu_int_to_float u_int_to_float_signed (
        .val(int_src),
        .is_signed(1'b1),
        .result(int_to_float_signed)
    );
    
    fpu_int_to_float u_int_to_float_unsigned (
        .val(int_src),
        .is_signed(1'b0),
        .result(int_to_float_unsigned)
    );
    
    fpu_float_to_int u_float_to_int_signed (
        .val(fs1),
        .is_signed(1'b1),
        .result(float_to_int_signed),
        .invalid(float_to_int_signed_invalid)
    );
    
    fpu_float_to_int u_float_to_int_unsigned (
        .val(fs1),
        .is_signed(1'b0),
        .result(float_to_int_unsigned),
        .invalid(float_to_int_unsigned_invalid)
    );
    
    // Division setup
    fpu_div_setup u_div_setup (
        .a(fs1),
        .b(fs2),
        .dividend(div_dividend),
        .divisor(div_divisor),
        .needs_div(div_needs_hw),
        .special_result(div_special_result),
        .flags(div_setup_flags)
    );

    
    // ============================================================
    // Division Unit Integration
    // ============================================================
    
    // Division unit signals (48-bit for FP mantissa precision)
    logic        div_start;
    logic        div_ready;
    logic [47:0] div_result;
    
    // Instantiate 48-bit division unit
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
    
    // Division state registers
    logic [31:0] div_fs1_reg;
    logic [31:0] div_fs2_reg;
    logic        div_in_progress;
    
    // Detect FP division operation
    logic is_fp_div;
    assign is_fp_div = (fpu_op == FPU_DIV);
    
    // Start division when requested and needed
    assign div_start = fpu_start && is_fp_div && div_needs_hw;
    
    // FPU ready signal
    assign fpu_ready = div_in_progress ? div_ready : 
                       (fpu_start && is_fp_div && div_needs_hw) ? 1'b0 :
                       1'b1;
    
    // Division state machine
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            div_fs1_reg <= 32'h0;
            div_fs2_reg <= 32'h0;
            div_in_progress <= 1'b0;
        end else begin
            if (div_start) begin
                div_fs1_reg <= fs1;
                div_fs2_reg <= fs2;
                div_in_progress <= 1'b1;
            end else if (div_ready) begin
                div_in_progress <= 1'b0;
            end
        end
    end
    
    // Division result assembly
    fpu_div_assemble u_div_assemble (
        .a(div_fs1_reg),
        .b(div_fs2_reg),
        .quotient_raw(div_result),
        .result(div_assembled_result),
        .flags(div_assemble_flags)
    );

    
    // ============================================================
    // Main Operation Selection Logic
    // ============================================================
    
    // Temporary signal for invalid flag
    logic inv_flag;
    
    always_comb begin
        // Default outputs
        fp_result = POS_ZERO;
        int_result = 32'h0;
        fflags = 5'b0;
        inv_flag = 1'b0;
        
        case (fpu_op)
            // ========== Arithmetic Operations (via Shared FMA) ==========
            FPU_ADD,
            FPU_SUB,
            FPU_MUL: begin
                fp_result = fma_result;
                fflags = fma_flags;
            end
            
            FPU_DIV: begin
                if (div_in_progress && div_ready) begin
                    fp_result = div_assembled_result;
                    fflags = div_assemble_flags;
                end else begin
                    fp_result = div_special_result;
                    fflags = div_setup_flags;
                end
            end
            
            FPU_SQRT: begin
                fp_result = sqrt_result;
                fflags = sqrt_flags;
            end
            
            // ========== FMA Operations (via Shared FMA) ==========
            FPU_MADD,
            FPU_MSUB,
            FPU_NMSUB,
            FPU_NMADD: begin
                fp_result = fma_result;
                fflags = fma_flags;
            end
            
            // ========== Sign Injection ==========
            FPU_SGNJ: begin
                fp_result = {fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJN: begin
                fp_result = {~fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJX: begin
                fp_result = {fs1[31] ^ fs2[31], fs1[30:0]};
            end
            
            // ========== Move Operations ==========
            FPU_MVXW: begin
                int_result = fs1;
            end
            
            FPU_MVWX: begin
                fp_result = int_src;
            end
            
            // ========== Comparison Operations ==========
            FPU_FEQ: begin
                if (fs1_is_nan || fs2_is_nan) begin
                    int_result = 32'h0;
                    if (fs1_is_snan || fs2_is_snan) begin
                        fflags[4] = 1'b1;  // Invalid operation
                    end
                end else begin
                    int_result = (fs1 == fs2) ? 32'h1 : 32'h0;
                end
            end
            
            FPU_FLT: begin
                if (fs1_is_nan || fs2_is_nan) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;  // Invalid operation
                end else begin
                    int_result = fs1_less_than_fs2 ? 32'h1 : 32'h0;
                end
            end
            
            FPU_FLE: begin
                if (fs1_is_nan || fs2_is_nan) begin
                    int_result = 32'h0;
                    fflags[4] = 1'b1;  // Invalid operation
                end else begin
                    int_result = (fs1_less_than_fs2 || (fs1 == fs2)) ? 32'h1 : 32'h0;
                end
            end
            
            FPU_MIN: begin
                if (fs1_is_nan && fs2_is_nan) begin
                    fp_result = QNAN;
                end else if (fs1_is_nan) begin
                    fp_result = fs2;
                end else if (fs2_is_nan) begin
                    fp_result = fs1;
                end else if (fs1_is_zero && fs2_is_zero) begin
                    fp_result = (fs1[31] || fs2[31]) ? NEG_ZERO : POS_ZERO;
                end else begin
                    fp_result = fs1_less_than_fs2 ? fs1 : fs2;
                end
            end
            
            FPU_MAX: begin
                if (fs1_is_nan && fs2_is_nan) begin
                    fp_result = QNAN;
                end else if (fs1_is_nan) begin
                    fp_result = fs2;
                end else if (fs2_is_nan) begin
                    fp_result = fs1;
                end else if (fs1_is_zero && fs2_is_zero) begin
                    fp_result = (fs1[31] && fs2[31]) ? NEG_ZERO : POS_ZERO;
                end else begin
                    fp_result = fs1_less_than_fs2 ? fs2 : fs1;
                end
            end
            
            // ========== Classification ==========
            FPU_FCLASS: begin
                if (fs1_is_nan) begin
                    int_result = fs1[22] ? 32'h00000200 : 32'h00000100;  // QNaN : SNaN
                end else if (fs1_is_inf) begin
                    int_result = fs1[31] ? 32'h00000001 : 32'h00000080;  // -Inf : +Inf
                end else if (fs1_is_zero) begin
                    int_result = fs1[31] ? 32'h00000008 : 32'h00000010;  // -0 : +0
                end else if (fs1_is_subnormal) begin
                    int_result = fs1[31] ? 32'h00000004 : 32'h00000020;  // -subnormal : +subnormal
                end else begin
                    int_result = fs1[31] ? 32'h00000002 : 32'h00000040;  // -normal : +normal
                end
            end
            
            // ========== Conversion Operations ==========
            FPU_CVTSW: begin
                fp_result = int_to_float_signed;
            end
            
            FPU_CVTSWU: begin
                fp_result = int_to_float_unsigned;
            end
            
            FPU_CVTWS: begin
                int_result = float_to_int_signed;
                inv_flag = float_to_int_signed_invalid;
                if (inv_flag) begin
                    fflags[4] = 1'b1;  // Invalid operation
                end
            end
            
            FPU_CVTWUS: begin
                int_result = float_to_int_unsigned;
                inv_flag = float_to_int_unsigned_invalid;
                if (inv_flag) begin
                    fflags[4] = 1'b1;  // Invalid operation
                end
            end
            
            default: begin
                fp_result = POS_ZERO;
                int_result = 32'h0;
            end
        endcase
    end

endmodule
`default_nettype wire

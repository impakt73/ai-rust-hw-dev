// Division Unit Module
// Hardware-synthesizable division using Non-Restoring Algorithm
// Implements 32-bit signed and unsigned division and remainder

module div_unit (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control interface
    input  logic        start,        // Start division (pulse)
    input  logic        is_signed,    // 1=signed (DIV/REM), 0=unsigned (DIVU/REMU)
    input  logic        rem_sel,      // 1=remainder, 0=quotient
    
    // Data interface
    input  logic [31:0] dividend,     // Dividend (A)
    input  logic [31:0] divisor,      // Divisor (B)
    output logic [31:0] result,       // Quotient or Remainder
    output logic        ready         // Result valid
);

    // ============================================================
    // State Machine Definition
    // ============================================================
    typedef enum logic [2:0] {
        DIV_IDLE     = 3'b000,  // Waiting for start
        DIV_INIT     = 3'b001,  // Initialize registers
        DIV_ITER     = 3'b010,  // Perform 32 iterations
        DIV_CORRECT  = 3'b011,  // Final correction if needed
        DIV_DONE     = 3'b100   // Result ready
    } div_state_t;
    
    div_state_t state, next_state;
    
    // ============================================================
    // Internal Registers
    // ============================================================
    
    // Division working registers
    logic [63:0] P;           // Partial remainder (64-bit)
    logic [63:0] D;           // Divisor aligned (64-bit)
    logic [31:0] Q;           // Quotient accumulator
    logic [5:0]  iter_count;  // Iteration counter (0-31)
    
    // Sign tracking
    logic        dividend_neg;
    logic        divisor_neg;
    
    // Special case flags
    logic        div_by_zero;
    logic        overflow;
    
    // Intermediate values (combinational)
    logic [31:0] abs_dividend;
    logic [31:0] abs_divisor;
    logic [31:0] final_quotient;
    logic [31:0] final_remainder;
    
    // ============================================================
    // State Register
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            state <= DIV_IDLE;
        else
            state <= next_state;
    end
    
    // ============================================================
    // Next State Logic
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            DIV_IDLE: begin
                if (start)
                    next_state = DIV_INIT;
            end
            
            DIV_INIT: begin
                if (div_by_zero || overflow)
                    next_state = DIV_DONE;  // Skip iterations for edge cases
                else
                    next_state = DIV_ITER;
            end
            
            DIV_ITER: begin
                if (iter_count == 6'd31)  // After 32 iterations (0-31)
                    next_state = DIV_CORRECT;
                else
                    next_state = DIV_ITER;
            end
            
            DIV_CORRECT: begin
                next_state = DIV_DONE;
            end
            
            DIV_DONE: begin
                // Return to IDLE unconditionally to avoid deadlock
                // (start pulse should be only 1 cycle from top-level FSM)
                next_state = DIV_IDLE;
            end
            
            default: next_state = DIV_IDLE;
        endcase
    end
    
    // ============================================================
    // Datapath Registers
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            P <= 64'h0;
            D <= 64'h0;
            Q <= 32'h0;
            iter_count <= 6'd0;
            dividend_neg <= 1'b0;
            divisor_neg <= 1'b0;
            div_by_zero <= 1'b0;
            overflow <= 1'b0;
        end else begin
            case (state)
                DIV_INIT: begin
                    // Check for special cases
                    div_by_zero <= (divisor == 32'd0);
                    overflow <= is_signed && 
                                (dividend == 32'h80000000) && 
                                (divisor == 32'hFFFFFFFF);
                    
                    if (divisor != 32'd0) begin
                        // Handle sign conversion for signed division
                        if (is_signed) begin
                            dividend_neg <= dividend[31];
                            divisor_neg <= divisor[31];
                            abs_dividend = dividend[31] ? (~dividend + 32'd1) : dividend;
                            abs_divisor  = divisor[31]  ? (~divisor  + 32'd1) : divisor;
                        end else begin
                            dividend_neg <= 1'b0;
                            divisor_neg <= 1'b0;
                            abs_dividend = dividend;
                            abs_divisor  = divisor;
                        end
                        
                        // Initialize division registers
                        P <= {32'h0, abs_dividend};  // {remainder, dividend}
                        D <= {abs_divisor, 32'h0};   // Divisor in upper 32 bits
                        Q <= 32'h0;
                        iter_count <= 6'd0;
                    end
                end
                
                DIV_ITER: begin
                    // Non-restoring division iteration
                    logic [63:0] P_shifted;
                    
                    P_shifted = P << 1;  // Shift left by 1
                    
                    if (!P_shifted[63]) begin
                        // Positive: subtract divisor, set quotient bit
                        P <= P_shifted - D;
                        Q <= {Q[30:0], 1'b1};
                    end else begin
                        // Negative: add divisor, clear quotient bit
                        P <= P_shifted + D;
                        Q <= {Q[30:0], 1'b0};
                    end
                    
                    iter_count <= iter_count + 6'd1;
                end
                
                DIV_CORRECT: begin
                    // Final non-restoring correction: if P is negative, add back D and decrement Q
                    if (P[63]) begin
                        P <= P + D;
                        Q <= Q - 32'd1;
                    end
                end
                
                default: begin
                    // Hold values in other states
                end
            endcase
        end
    end
    
    // ============================================================
    // Sign Correction (Combinational)
    // ============================================================
    always_comb begin
        // Apply signs to quotient and remainder based on RISC-V specification
        if (is_signed && !div_by_zero && !overflow) begin
            // Quotient sign: sign(dividend) XOR sign(divisor)
            if (dividend_neg ^ divisor_neg)
                final_quotient = ~Q + 32'd1;  // Two's complement negation
            else
                final_quotient = Q;
            
            // Remainder sign: same as dividend
            if (dividend_neg)
                final_remainder = ~P[63:32] + 32'd1;  // Two's complement negation
            else
                final_remainder = P[63:32];
        end else begin
            // Unsigned or edge cases: use values as-is
            final_quotient = Q;
            final_remainder = P[63:32];
        end
    end
    
    // ============================================================
    // Output Logic (Combinational)
    // ============================================================
    always_comb begin
        ready = (state == DIV_DONE);
        
        if (state == DIV_DONE) begin
            if (div_by_zero) begin
                // RISC-V spec: division by zero
                if (rem_sel)
                    result = dividend;  // REM/REMU: return dividend unchanged
                else
                    result = 32'hFFFFFFFF;  // DIV/DIVU: return all 1's
            end else if (overflow) begin
                // RISC-V spec: -2^31 / -1 overflow
                if (rem_sel)
                    result = 32'd0;  // REM: return 0
                else
                    result = 32'h80000000;  // DIV: return -2^31
            end else begin
                if (rem_sel)
                    result = final_remainder;  // Remainder
                else
                    result = final_quotient;   // Quotient
            end
        end else begin
            result = 32'h0;  // Default when not ready
        end
    end

endmodule

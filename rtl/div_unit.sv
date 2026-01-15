// Division Unit Module
// Hardware-synthesizable division using Non-Restoring Algorithm
// Parameterizable width for signed and unsigned division and remainder

module div_unit #(
    parameter int WIDTH = 32  // Bit width of operands (default 32-bit for RV32IM integer ops)
) (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control interface
    input  logic        start,        // Start division (pulse)
    input  logic        is_signed,    // 1=signed (DIV/REM), 0=unsigned (DIVU/REMU)
    input  logic        rem_sel,      // 1=remainder, 0=quotient
    
    // Data interface
    input  logic [WIDTH-1:0] dividend,     // Dividend (A)
    input  logic [WIDTH-1:0] divisor,      // Divisor (B)
    output logic [WIDTH-1:0] result,       // Quotient or Remainder
    output logic             ready         // Result valid
);

    // ============================================================
    // State Machine Definition
    // ============================================================
    typedef enum logic [2:0] {
        DIV_IDLE     = 3'b000,  // Waiting for start
        DIV_INIT     = 3'b001,  // Initialize registers
        DIV_ITER     = 3'b010,  // Perform WIDTH iterations
        DIV_CORRECT  = 3'b011,  // Final correction if needed
        DIV_DONE     = 3'b100   // Result ready
    } div_state_t;
    
    div_state_t state, next_state;
    
    // ============================================================
    // Internal Registers
    // ============================================================
    
    // Division working registers
    logic [2*WIDTH-1:0] P;           // Partial remainder (2*WIDTH-bit)
    logic [2*WIDTH-1:0] D;           // Divisor aligned (2*WIDTH-bit)
    logic [WIDTH-1:0]   Q;           // Quotient accumulator
    logic [7:0]         iter_count;  // Iteration counter (0 to WIDTH-1)
    
    // Sign tracking
    logic        dividend_neg;
    logic        divisor_neg;
    
    // Special case flags
    logic        div_by_zero;
    logic        overflow;
    
    // Intermediate values (combinational)
    logic [WIDTH-1:0] abs_dividend;
    logic [WIDTH-1:0] abs_divisor;
    logic [WIDTH-1:0] final_quotient;
    logic [WIDTH-1:0] final_remainder;
    
    // Temporary variables for division iteration
    logic [2*WIDTH-1:0] P_shifted;
    logic [2*WIDTH-1:0] P_add;      // P + D (for non-restoring when P is negative)
    logic [2*WIDTH-1:0] P_sub;      // P - D (for non-restoring when P is non-negative)
    
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
                /* verilator lint_off WIDTHEXPAND */
                if (iter_count == (WIDTH-1))  // After WIDTH iterations (0 to WIDTH-1)
                /* verilator lint_on WIDTHEXPAND */
                    next_state = DIV_CORRECT;  // Need correction for non-restoring
                else
                    next_state = DIV_ITER;
            end
            
            DIV_CORRECT: begin
                // Final correction step for non-restoring division
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
    // Combinational logic for absolute value conversion
    // ============================================================
    always_comb begin
        // Default values
        abs_dividend = dividend;
        abs_divisor = divisor;
        
        // Compute absolute values in INIT state for signed operations
        if (state == DIV_INIT && is_signed && divisor != '0) begin
            abs_dividend = dividend[WIDTH-1] ? (~dividend + 1'b1) : dividend;
            abs_divisor  = divisor[WIDTH-1]  ? (~divisor  + 1'b1) : divisor;
        end
        
        // Compute shifted, add, and subtract values for non-restoring division
        P_shifted = P << 1;
        P_sub = P_shifted - D;  // Subtract divisor from shifted P
        P_add = P_shifted + D;  // Add divisor to shifted P
    end
    
    // ============================================================
    // Datapath Registers
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            P <= '0;
            D <= '0;
            Q <= '0;
            iter_count <= '0;
            dividend_neg <= 1'b0;
            divisor_neg <= 1'b0;
            div_by_zero <= 1'b0;
            overflow <= 1'b0;
        end else begin
            case (state)
                DIV_INIT: begin
                    // Check for special cases
                    div_by_zero <= (divisor == '0);
                    // Overflow only for signed: most negative / -1
                    /* verilator lint_off WIDTHEXPAND */
                    overflow <= is_signed && 
                                (dividend == (1'b1 << (WIDTH-1))) && 
                                (divisor == '1);
                    /* verilator lint_on WIDTHEXPAND */
                    
                    if (divisor != '0) begin
                        // Handle sign tracking for signed division
                        if (is_signed) begin
                            dividend_neg <= dividend[WIDTH-1];
                            divisor_neg <= divisor[WIDTH-1];
                        end else begin
                            dividend_neg <= 1'b0;
                            divisor_neg <= 1'b0;
                        end
                        
                        // Initialize division registers (abs values computed combinationally)
                        P <= {{WIDTH{1'b0}}, abs_dividend};  // {remainder, dividend}
                        D <= {abs_divisor, {WIDTH{1'b0}}};   // Divisor in upper WIDTH bits
                        Q <= '0;
                        iter_count <= '0;
                    end
                end
                
                DIV_ITER: begin
                    // Non-restoring division iteration
                    // Decision based on current partial remainder sign:
                    // - If P >= 0: shift and subtract divisor
                    // - If P < 0: shift and add divisor
                    // Quotient bit is determined by the result's sign (1 if non-negative, 0 if negative)
                    
                    if (!P[2*WIDTH-1]) begin
                        // Partial remainder is non-negative: shift left and subtract divisor
                        P <= P_sub;
                        Q <= {Q[WIDTH-2:0], !P_sub[2*WIDTH-1] ? 1'b1 : 1'b0};
                    end else begin
                        // Partial remainder is negative: shift left and add divisor
                        P <= P_add;
                        Q <= {Q[WIDTH-2:0], !P_add[2*WIDTH-1] ? 1'b1 : 1'b0};
                    end
                    
                    iter_count <= iter_count + 1'b1;
                end
                
                DIV_CORRECT: begin
                    // Final correction for non-restoring division
                    // If the final remainder is negative, add divisor to make it positive
                    if (P[2*WIDTH-1]) begin
                        P <= P + D;
                    end
                    // Quotient is already correct from iteration loop
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
                final_quotient = ~Q + 1'b1;  // Two's complement negation
            else
                final_quotient = Q;
            
            // Remainder sign: same as dividend
            if (dividend_neg)
                final_remainder = ~P[2*WIDTH-1:WIDTH] + 1'b1;  // Two's complement negation
            else
                final_remainder = P[2*WIDTH-1:WIDTH];
        end else begin
            // Unsigned or edge cases: use values as-is
            final_quotient = Q;
            final_remainder = P[2*WIDTH-1:WIDTH];
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
                    result = '1;  // DIV/DIVU: return all 1's
            end else if (overflow) begin
                // RISC-V spec: most negative / -1 overflow
                if (rem_sel)
                    result = '0;  // REM: return 0
                else
                    result = (1'b1 << (WIDTH-1));  // DIV: return most negative number
            end else begin
                if (rem_sel)
                    result = final_remainder;  // Remainder
                else
                    result = final_quotient;   // Quotient
            end
        end else begin
            result = '0;  // Default when not ready
        end
    end

endmodule

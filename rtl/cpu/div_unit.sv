// Division Unit Module
// Hardware-synthesizable restoring division with WIDTH+1 remainder datapath
// Uses narrower remainder arithmetic than prior 2*WIDTH partial-remainder form to reduce LUT cost
// Parameterizable width for signed and unsigned division and remainder

module div_unit #(
    parameter int WIDTH = 32  // Bit width of operands (default 32-bit for RV32IM integer ops)
) (
    input  logic             clk,
    input  logic             rst_n,

    // Control interface
    input  logic             start,      // Start division (pulse)
    input  logic             is_signed,  // 1=signed (DIV/REM), 0=unsigned (DIVU/REMU)
    input  logic             rem_sel,    // 1=remainder, 0=quotient

    // Data interface
    input  logic [WIDTH-1:0] dividend,   // Dividend (A)
    input  logic [WIDTH-1:0] divisor,    // Divisor (B)
    output logic [WIDTH-1:0] result,     // Quotient or Remainder
    output logic             ready       // Result valid
);

    // ============================================================
    // State Machine Definition
    // ============================================================
    typedef enum logic [1:0] {
        DIV_IDLE = 2'b00,  // Waiting for start
        DIV_INIT = 2'b01,  // Initialize registers
        DIV_ITER = 2'b10,  // Perform WIDTH restoring iterations
        DIV_DONE = 2'b11   // Result ready
    } div_state_t;

    div_state_t state, next_state;

    // ============================================================
    // Internal Registers
    // ============================================================
    logic [WIDTH:0]   remainder_reg;  // WIDTH+1 partial remainder
    logic [WIDTH-1:0] quotient_reg;   // Quotient shift register
    logic [WIDTH-1:0] divisor_reg;    // Absolute divisor
    logic [$clog2(WIDTH)-1:0] iter_count;  // Counts 0..WIDTH-1; $clog2(WIDTH) is sufficient for terminal count WIDTH-1

    // Sign tracking
    logic dividend_neg;
    logic divisor_neg;

    // Special case flags
    logic div_by_zero;
    logic overflow;

    // Intermediate values (combinational)
    logic [WIDTH-1:0] abs_dividend;
    logic [WIDTH-1:0] abs_divisor;
    logic [WIDTH:0]   remainder_shifted;
    logic [WIDTH:0]   remainder_sub;
    logic [WIDTH-1:0] quotient_next;
    logic [WIDTH:0]   remainder_next;
    logic [WIDTH-1:0] final_quotient;
    logic [WIDTH-1:0] final_remainder;

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
                    next_state = DIV_DONE;
                else
                    next_state = DIV_ITER;
            end

            DIV_ITER: begin
                /* verilator lint_off WIDTHEXPAND */
                if (iter_count == (WIDTH-1))
                /* verilator lint_on WIDTHEXPAND */
                    next_state = DIV_DONE;
                else
                    next_state = DIV_ITER;
            end

            DIV_DONE: begin
                next_state = DIV_IDLE;
            end

            default: next_state = DIV_IDLE;
        endcase
    end

    // ============================================================
    // Combinational datapath helpers
    // ============================================================
    always_comb begin
        abs_dividend = dividend;
        abs_divisor = divisor;

        if (state == DIV_INIT && is_signed && divisor != '0) begin
            abs_dividend = dividend[WIDTH-1] ? (~dividend + 1'b1) : dividend;
            abs_divisor = divisor[WIDTH-1] ? (~divisor + 1'b1) : divisor;
        end

        // Restoring division iteration:
        // 1) Shift {remainder,quotient} left by 1
        // 2) Try subtracting divisor from remainder
        // 3) If subtraction is non-negative, keep it and set quotient bit to 1
        //    Otherwise restore shifted remainder and set quotient bit to 0
        remainder_shifted = {remainder_reg[WIDTH-1:0], quotient_reg[WIDTH-1]};
        remainder_sub = remainder_shifted - {1'b0, divisor_reg};

        if (!remainder_sub[WIDTH]) begin
            remainder_next = remainder_sub;
            quotient_next = {quotient_reg[WIDTH-2:0], 1'b1};
        end else begin
            remainder_next = remainder_shifted;
            quotient_next = {quotient_reg[WIDTH-2:0], 1'b0};
        end
    end

    // ============================================================
    // Datapath Registers
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            remainder_reg <= '0;
            quotient_reg <= '0;
            divisor_reg <= '0;
            iter_count <= '0;
            dividend_neg <= 1'b0;
            divisor_neg <= 1'b0;
            div_by_zero <= 1'b0;
            overflow <= 1'b0;
        end else begin
            case (state)
                DIV_INIT: begin
                    div_by_zero <= (divisor == '0);
                    /* verilator lint_off WIDTHEXPAND */
                    overflow <= is_signed &&
                                (dividend == (1'b1 << (WIDTH-1))) &&
                                (divisor == '1);
                    /* verilator lint_on WIDTHEXPAND */

                    if (divisor != '0) begin
                        if (is_signed) begin
                            dividend_neg <= dividend[WIDTH-1];
                            divisor_neg <= divisor[WIDTH-1];
                        end else begin
                            dividend_neg <= 1'b0;
                            divisor_neg <= 1'b0;
                        end

                        remainder_reg <= '0;
                        quotient_reg <= abs_dividend;
                        divisor_reg <= abs_divisor;
                        iter_count <= '0;
                    end
                end

                DIV_ITER: begin
                    remainder_reg <= remainder_next;
                    quotient_reg <= quotient_next;
                    iter_count <= iter_count + 1'b1;
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
        if (is_signed && !div_by_zero && !overflow) begin
            if (dividend_neg ^ divisor_neg)
                final_quotient = ~quotient_reg + 1'b1;
            else
                final_quotient = quotient_reg;

            if (dividend_neg)
                final_remainder = ~remainder_reg[WIDTH-1:0] + 1'b1;
            else
                final_remainder = remainder_reg[WIDTH-1:0];
        end else begin
            final_quotient = quotient_reg;
            final_remainder = remainder_reg[WIDTH-1:0];
        end
    end

    // ============================================================
    // Output Logic (Combinational)
    // ============================================================
    always_comb begin
        ready = (state == DIV_DONE);

        if (state == DIV_DONE) begin
            if (div_by_zero) begin
                if (rem_sel)
                    result = dividend;
                else
                    result = '1;
            end else if (overflow) begin
                if (rem_sel)
                    result = '0;
                else
                    result = (1'b1 << (WIDTH-1));
            end else begin
                if (rem_sel)
                    result = final_remainder;
                else
                    result = final_quotient;
            end
        end else begin
            result = '0;
        end
    end

endmodule

// M Unit Module
// Shared iterative unit for RV32M multiply/divide/remainder operations

module m_unit #(
    parameter int WIDTH = 32
) (
    input  logic             clk,
    input  logic             rst_n,
    input  logic             start,       // Start operation (pulse)
    input  logic [2:0]       op_type,     // 000=MUL 001=MULH 010=MULHSU 011=MULHU 100=DIV 101=DIVU 110=REM 111=REMU
    input  logic [WIDTH-1:0] a,
    input  logic [WIDTH-1:0] b,
    output logic [WIDTH-1:0] result,
    output logic             ready
);

    localparam logic [2:0] OP_MUL    = 3'b000;
    localparam logic [2:0] OP_MULH   = 3'b001;
    localparam logic [2:0] OP_MULHSU = 3'b010;
    localparam logic [2:0] OP_MULHU  = 3'b011;
    localparam logic [2:0] OP_DIV    = 3'b100;
    localparam logic [2:0] OP_DIVU   = 3'b101;
    localparam logic [2:0] OP_REM    = 3'b110;
    localparam logic [2:0] OP_REMU   = 3'b111;

    typedef enum logic [1:0] {
        M_IDLE = 2'b00,
        M_INIT = 2'b01,
        M_ITER = 2'b10,
        M_DONE = 2'b11
    } m_state_t;

    m_state_t state, next_state;

    logic                   is_div_reg;
    logic [2:0]             op_type_reg;
    logic [$clog2(WIDTH)-1:0] iter_count;  // Counts 0..WIDTH-1; $clog2(WIDTH) is sufficient for terminal count WIDTH-1

    // Multiply datapath
    logic [2*WIDTH-1:0] product_reg;
    logic [WIDTH-1:0]   mcand_reg;
    logic               mul_result_negative_reg;
    logic [WIDTH:0]     mul_add_result;
    logic [2*WIDTH-1:0] mul_final_product;
    logic [WIDTH-1:0]   mul_abs_a;
    logic [WIDTH-1:0]   mul_abs_b;

    // Divide datapath (WIDTH+1 remainder form)
    logic [WIDTH:0]     div_remainder_reg;
    logic [WIDTH-1:0]   div_quotient_reg;
    logic [WIDTH-1:0]   div_divisor_reg;
    logic               div_by_zero_reg;
    logic               div_overflow_reg;
    logic               div_dividend_neg_reg;
    logic               div_divisor_neg_reg;

    logic [WIDTH-1:0]   div_abs_dividend;
    logic [WIDTH-1:0]   div_abs_divisor;
    logic [WIDTH:0]     div_remainder_shifted;
    logic [WIDTH:0]     div_remainder_sub;
    logic [WIDTH:0]     div_remainder_next;
    logic [WIDTH-1:0]   div_quotient_next;
    logic [WIDTH-1:0]   div_final_quotient;
    logic [WIDTH-1:0]   div_final_remainder;

    logic div_is_signed;
    logic div_is_remainder;
    logic div_skip_iter;
    logic [WIDTH-1:0] neg_one;

    assign div_is_signed = (op_type == OP_DIV) || (op_type == OP_REM);
    assign div_is_remainder = (op_type == OP_REM) || (op_type == OP_REMU);
    assign neg_one = {WIDTH{1'b1}};
    /* verilator lint_off WIDTHEXPAND */
    assign div_skip_iter =
        op_type[2] &&
        ((b == '0) || (div_is_signed && (a == (1'b1 << (WIDTH-1))) && (b == neg_one)));
    /* verilator lint_on WIDTHEXPAND */

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            state <= M_IDLE;
        else
            state <= next_state;
    end

    always_comb begin
        next_state = state;

        case (state)
            M_IDLE: begin
                if (start)
                    next_state = M_INIT;
            end

            M_INIT: begin
                if (div_skip_iter)
                    next_state = M_DONE;
                else
                    next_state = M_ITER;
            end

            M_ITER: begin
                /* verilator lint_off WIDTHEXPAND */
                if (iter_count == (WIDTH-1))
                /* verilator lint_on WIDTHEXPAND */
                    next_state = M_DONE;
                else
                    next_state = M_ITER;
            end

            M_DONE: begin
                next_state = M_IDLE;
            end

            default: next_state = M_IDLE;
        endcase
    end

    always_comb begin
        mul_abs_a = a;
        mul_abs_b = b;

        if (state == M_INIT) begin
            if ((op_type == OP_MUL) || (op_type == OP_MULH) || (op_type == OP_MULHSU))
                mul_abs_a = a[WIDTH-1] ? (~a + 1'b1) : a;

            if ((op_type == OP_MUL) || (op_type == OP_MULH))
                mul_abs_b = b[WIDTH-1] ? (~b + 1'b1) : b;
        end

        mul_add_result = {1'b0, product_reg[2*WIDTH-1:WIDTH]} + {1'b0, mcand_reg};

        if (mul_result_negative_reg && (product_reg != '0))
            mul_final_product = ~product_reg + 1'b1;
        else
            mul_final_product = product_reg;

        div_abs_dividend = a;
        div_abs_divisor = b;
        if (state == M_INIT && div_is_signed && b != '0) begin
            div_abs_dividend = a[WIDTH-1] ? (~a + 1'b1) : a;
            div_abs_divisor = b[WIDTH-1] ? (~b + 1'b1) : b;
        end

        div_remainder_shifted = {div_remainder_reg[WIDTH-1:0], div_quotient_reg[WIDTH-1]};
        div_remainder_sub = div_remainder_shifted - {1'b0, div_divisor_reg};
        if (!div_remainder_sub[WIDTH]) begin
            div_remainder_next = div_remainder_sub;
            div_quotient_next = {div_quotient_reg[WIDTH-2:0], 1'b1};
        end else begin
            div_remainder_next = div_remainder_shifted;
            div_quotient_next = {div_quotient_reg[WIDTH-2:0], 1'b0};
        end

        if (div_dividend_neg_reg ^ div_divisor_neg_reg)
            div_final_quotient = ~div_quotient_reg + 1'b1;
        else
            div_final_quotient = div_quotient_reg;

        if (div_dividend_neg_reg)
            div_final_remainder = ~div_remainder_reg[WIDTH-1:0] + 1'b1;
        else
            div_final_remainder = div_remainder_reg[WIDTH-1:0];
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            is_div_reg <= 1'b0;
            op_type_reg <= '0;
            iter_count <= '0;

            product_reg <= '0;
            mcand_reg <= '0;
            mul_result_negative_reg <= 1'b0;

            div_remainder_reg <= '0;
            div_quotient_reg <= '0;
            div_divisor_reg <= '0;
            div_by_zero_reg <= 1'b0;
            div_overflow_reg <= 1'b0;
            div_dividend_neg_reg <= 1'b0;
            div_divisor_neg_reg <= 1'b0;
        end else begin
            case (state)
                M_INIT: begin
                    is_div_reg <= op_type[2];
                    op_type_reg <= op_type;
                    iter_count <= '0;

                    if (op_type[2]) begin
                        div_by_zero_reg <= (b == '0);
                        /* verilator lint_off WIDTHEXPAND */
                        div_overflow_reg <= div_is_signed && (a == (1'b1 << (WIDTH-1))) && (b == neg_one);
                        /* verilator lint_on WIDTHEXPAND */

                        if (b != '0) begin
                            if (div_is_signed) begin
                                div_dividend_neg_reg <= a[WIDTH-1];
                                div_divisor_neg_reg <= b[WIDTH-1];
                            end else begin
                                div_dividend_neg_reg <= 1'b0;
                                div_divisor_neg_reg <= 1'b0;
                            end

                            div_remainder_reg <= '0;
                            div_quotient_reg <= div_abs_dividend;
                            div_divisor_reg <= div_abs_divisor;
                        end
                    end else begin
                        if ((op_type == OP_MUL) || (op_type == OP_MULH))
                            mul_result_negative_reg <= a[WIDTH-1] ^ b[WIDTH-1];
                        else if (op_type == OP_MULHSU)
                            mul_result_negative_reg <= a[WIDTH-1];
                        else
                            mul_result_negative_reg <= 1'b0;

                        product_reg <= {{WIDTH{1'b0}}, mul_abs_b};
                        mcand_reg <= mul_abs_a;
                    end
                end

                M_ITER: begin
                    iter_count <= iter_count + 1'b1;
                    if (is_div_reg) begin
                        div_remainder_reg <= div_remainder_next;
                        div_quotient_reg <= div_quotient_next;
                    end else begin
                        if (product_reg[0])
                            product_reg <= {mul_add_result[WIDTH:0], product_reg[WIDTH-1:1]};
                        else
                            product_reg <= {1'b0, product_reg[2*WIDTH-1:1]};
                    end
                end

                default: begin
                    // Hold state in IDLE and DONE
                end
            endcase
        end
    end

    always_comb begin
        ready = (state == M_DONE);
        result = '0;

        if (state == M_DONE) begin
            if (is_div_reg) begin
                if (div_by_zero_reg) begin
                    if (div_is_remainder)
                        result = a;
                    else
                        result = '1;
                end else if (div_overflow_reg) begin
                    if (div_is_remainder)
                        result = '0;
                    else
                        result = (1'b1 << (WIDTH-1));
                end else begin
                    if (div_is_remainder)
                        result = div_final_remainder;
                    else
                        result = div_final_quotient;
                end
            end else begin
                if (op_type_reg == OP_MUL)
                    result = mul_final_product[WIDTH-1:0];
                else
                    result = mul_final_product[2*WIDTH-1:WIDTH];
            end
        end
    end

endmodule

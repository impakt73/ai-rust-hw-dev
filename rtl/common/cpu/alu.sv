`default_nettype none
// ALU Module - Arithmetic Logic Unit
// Implements RISC-V RV32I ALU operations
// Multi-cycle support for multiplication and division operations
// Configurable M extension support for resource-constrained FPGAs

module alu #(
    parameter bit ENABLE_M_EXT = 1'b1  // RV32M extension: Multiply/Divide (default: enabled)
) (
    input wire logic        clk,
    input wire logic        rst,
    input wire logic [31:0] a,
    input wire logic [31:0] b,
    input wire logic [4:0]  alu_op,
    input wire logic        in_valid,
    output logic        in_ready,
    output logic [31:0] out_data,
    output logic        out_valid
);

    // ALU Operation Encodings (RV32I)
    localparam logic [4:0] ALU_ADD  = 5'b00000;
    localparam logic [4:0] ALU_SUB  = 5'b00001;
    localparam logic [4:0] ALU_AND  = 5'b00010;
    localparam logic [4:0] ALU_OR   = 5'b00011;
    localparam logic [4:0] ALU_XOR  = 5'b00100;
    localparam logic [4:0] ALU_SLL  = 5'b00101;
    localparam logic [4:0] ALU_SRL  = 5'b00110;
    localparam logic [4:0] ALU_SRA  = 5'b00111;
    localparam logic [4:0] ALU_SLT  = 5'b01000;
    localparam logic [4:0] ALU_SLTU = 5'b01001;
    
    // M Extension Operation Encodings (RV32IM)
    localparam logic [4:0] ALU_MUL    = 5'b01010;  // Multiply (lower 32 bits)
    localparam logic [4:0] ALU_MULH   = 5'b01011;  // Multiply High (signed×signed)
    localparam logic [4:0] ALU_MULHSU = 5'b01100;  // Multiply High (signed×unsigned)
    localparam logic [4:0] ALU_MULHU  = 5'b01101;  // Multiply High (unsigned×unsigned)
    localparam logic [4:0] ALU_DIV    = 5'b01110;  // Divide (signed)
    localparam logic [4:0] ALU_DIVU   = 5'b01111;  // Divide (unsigned)
    localparam logic [4:0] ALU_REM    = 5'b10000;  // Remainder (signed)
    localparam logic [4:0] ALU_REMU   = 5'b10001;  // Remainder (unsigned)
    
    // A Extension Operation Encodings (RV32A - Atomic MIN/MAX)
    localparam logic [4:0] ALU_MIN    = 5'b10010;  // Minimum (signed)
    localparam logic [4:0] ALU_MAX    = 5'b10011;  // Maximum (signed)
    localparam logic [4:0] ALU_MINU   = 5'b10100;  // Minimum (unsigned)
    localparam logic [4:0] ALU_MAXU   = 5'b10101;  // Maximum (unsigned)

    // ============================================================
    // M Extension: Division Unit (Conditional Generation)
    // ============================================================
    // Division unit signals
    logic        div_start;
    logic        div_is_signed;
    logic        div_rem_sel;
    logic [31:0] div_result;
    logic        div_ready;
    logic        launch_is_div_op;
    logic        launch_op;
    
    generate
        if (ENABLE_M_EXT) begin : gen_m_ext
            // Instantiate division unit with default 32-bit width for integer operations
            div_unit #(
                .WIDTH(32)
            ) u_div (
                .clk(clk),
                .rst(rst),
                .start(div_start),
                .is_signed(div_is_signed),
                .rem_sel(div_rem_sel),
                .dividend(req_a_reg),
                .divisor(req_b_reg),
                .result(div_result),
                .ready(div_ready)
            );
            
            // Start division when requested
            assign div_start = launch_op && launch_is_div_op;
        end else begin : gen_no_m_ext
            // M extension disabled: No division unit
            assign div_result = 32'd0;
            assign div_ready = 1'b1;
            assign div_start = 1'b0;
        end
    endgenerate
    
    // ============================================================
    // M Extension: Multiplication Unit (Conditional Generation)
    // ============================================================
    // Multi-cycle shift-and-add multiplier for MUL, MULH, MULHSU, MULHU
    // Replaces single-cycle 64x64 multiplier for better FPGA resource usage
    logic        mul_start;
    logic [1:0]  mul_op_type;
    logic [31:0] mul_result;
    logic        mul_ready;
    logic        launch_is_mul_op;
    
    generate
        if (ENABLE_M_EXT) begin : gen_multiplier
            // Instantiate multiplication unit with default 32-bit width for integer operations
            mul_unit #(
                .WIDTH(32)
            ) u_mul (
                .clk(clk),
                .rst(rst),
                .start(mul_start),
                .op_type(mul_op_type),
                .multiplicand(req_a_reg),
                .multiplier(req_b_reg),
                .result(mul_result),
                .ready(mul_ready)
            );
            
            // Start multiplication when requested
            assign mul_start = launch_op && launch_is_mul_op;
        end else begin : gen_no_multiplier
            // M extension disabled: No multiplier
            assign mul_result = 32'd0;
            assign mul_ready = 1'b1;
            assign mul_start = 1'b0;
        end
    endgenerate
    
    logic [31:0] arith_result;
    logic [31:0] bitwise_result;
    logic [31:0] shift_result;
    logic [31:0] muldiv_result;
    logic        launch_is_arith_op;
    logic        launch_is_bitwise_op;
    logic        launch_is_shift_op;
    logic        launch_is_minmax_op;
    logic        minmax_signed_lt;
    logic        minmax_unsigned_lt;
    logic        minmax_signed_lt_reg;
    logic        minmax_unsigned_lt_reg;
    logic        minmax_use_signed_compare_reg;
    logic        minmax_is_min_op_reg;
    logic        minmax_less_than_reg;
    logic [31:0] minmax_min_result_reg;
    logic [31:0] minmax_max_result_reg;
    logic        pending_operation_reg;
    logic [31:0] req_a_reg;
    logic [31:0] req_b_reg;
    logic [4:0]  req_op_reg;
    logic        req_is_arith_reg;
    logic        req_is_bitwise_reg;
    logic        req_is_shift_reg;
    logic        req_is_minmax_reg;
    logic        req_is_mul_reg;
    logic        req_is_div_reg;
    logic [31:0] result_next;
    typedef enum logic [2:0] {
        MINMAX_STAGE_IDLE            = 3'd0,
        MINMAX_STAGE_COMPARE_CAPTURE = 3'd1,
        MINMAX_STAGE_COMPARE_SELECT  = 3'd2,
        MINMAX_STAGE_RESULT_CAPTURE  = 3'd3,
        MINMAX_STAGE_OUTPUT_SELECT   = 3'd4
    } minmax_state_t;
    minmax_state_t minmax_state_reg;

    assign launch_is_arith_op = (alu_op == ALU_ADD)  ||
                                (alu_op == ALU_SUB)  ||
                                (alu_op == ALU_SLT)  ||
                                (alu_op == ALU_SLTU);

    assign launch_is_bitwise_op = (alu_op == ALU_AND) ||
                                  (alu_op == ALU_OR)  ||
                                  (alu_op == ALU_XOR);

    assign launch_is_shift_op = (alu_op == ALU_SLL) ||
                                (alu_op == ALU_SRL) ||
                                (alu_op == ALU_SRA);

    assign launch_is_minmax_op = (alu_op == ALU_MIN)  ||
                                 (alu_op == ALU_MAX)  ||
                                 (alu_op == ALU_MINU) ||
                                 (alu_op == ALU_MAXU);

    assign launch_is_mul_op = ENABLE_M_EXT &&
                              ((alu_op == ALU_MUL)    ||
                               (alu_op == ALU_MULH)   ||
                               (alu_op == ALU_MULHSU) ||
                               (alu_op == ALU_MULHU));

    assign launch_is_div_op = ENABLE_M_EXT &&
                              ((alu_op == ALU_DIV)  ||
                               (alu_op == ALU_DIVU) ||
                               (alu_op == ALU_REM)  ||
                               (alu_op == ALU_REMU));

    assign minmax_signed_lt = $signed(req_a_reg) < $signed(req_b_reg);
    assign minmax_unsigned_lt = req_a_reg < req_b_reg;

    // Backpressure new requests while the ALU holds a latched request that has not
    // yet produced a registered response.
    assign in_ready = !pending_operation_reg;
    assign launch_op = in_valid && in_ready;

    always_comb begin
        arith_result = 32'd0;

        case (req_op_reg)
            ALU_ADD:  arith_result = req_a_reg + req_b_reg;
            ALU_SUB:  arith_result = req_a_reg - req_b_reg;
            ALU_SLT:  arith_result = ($signed(req_a_reg) < $signed(req_b_reg)) ? 32'd1 : 32'd0;
            ALU_SLTU: arith_result = (req_a_reg < req_b_reg) ? 32'd1 : 32'd0;
            default:  arith_result = 32'd0;
        endcase
    end

    always_comb begin
        bitwise_result = 32'd0;

        case (req_op_reg)
            ALU_AND: bitwise_result = req_a_reg & req_b_reg;
            ALU_OR:  bitwise_result = req_a_reg | req_b_reg;
            ALU_XOR: bitwise_result = req_a_reg ^ req_b_reg;
            default: bitwise_result = 32'd0;
        endcase
    end

    always_comb begin
        shift_result = 32'd0;

        case (req_op_reg)
            ALU_SLL: shift_result = req_a_reg << req_b_reg[4:0];
            ALU_SRL: shift_result = req_a_reg >> req_b_reg[4:0];
            ALU_SRA: shift_result = $signed(req_a_reg) >>> req_b_reg[4:0];
            default: shift_result = 32'd0;
        endcase
    end

    always_comb begin
        if (!ENABLE_M_EXT) begin
            muldiv_result = 32'd0;
        end else if (req_is_mul_reg) begin
            muldiv_result = mul_result;
        end else if (req_is_div_reg) begin
            muldiv_result = div_result;
        end else begin
            muldiv_result = 32'd0;
        end
    end

    always_comb begin
        result_next = 32'd0;

        if (req_is_arith_reg) begin
            result_next = arith_result;
        end else if (req_is_shift_reg) begin
            result_next = shift_result;
        end else if (req_is_bitwise_reg) begin
            result_next = bitwise_result;
        end else if (req_is_mul_reg || req_is_div_reg) begin
            result_next = muldiv_result;
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            pending_operation_reg <= 1'b0;
            req_a_reg             <= 32'd0;
            req_b_reg             <= 32'd0;
            req_op_reg            <= 5'd0;
            req_is_arith_reg      <= 1'b0;
            req_is_bitwise_reg    <= 1'b0;
            req_is_shift_reg      <= 1'b0;
            req_is_minmax_reg     <= 1'b0;
            req_is_mul_reg        <= 1'b0;
            req_is_div_reg        <= 1'b0;
            minmax_signed_lt_reg  <= 1'b0;
            minmax_unsigned_lt_reg <= 1'b0;
            minmax_use_signed_compare_reg <= 1'b0;
            minmax_is_min_op_reg  <= 1'b0;
            minmax_less_than_reg  <= 1'b0;
            minmax_min_result_reg <= 32'd0;
            minmax_max_result_reg <= 32'd0;
            minmax_state_reg      <= MINMAX_STAGE_IDLE;
            div_is_signed         <= 1'b0;
            div_rem_sel           <= 1'b0;
            mul_op_type           <= 2'b00;
            out_valid             <= 1'b0;
        end else begin
            if (launch_op) begin
                pending_operation_reg <= 1'b1;
                req_a_reg             <= a;
                req_b_reg             <= b;
                req_op_reg            <= alu_op;
                req_is_arith_reg      <= launch_is_arith_op;
                req_is_bitwise_reg    <= launch_is_bitwise_op;
                req_is_shift_reg      <= launch_is_shift_op;
                req_is_minmax_reg     <= launch_is_minmax_op;
                req_is_mul_reg        <= launch_is_mul_op;
                req_is_div_reg        <= launch_is_div_op;
                minmax_state_reg      <= launch_is_minmax_op ? MINMAX_STAGE_COMPARE_CAPTURE : MINMAX_STAGE_IDLE;
                out_valid             <= 1'b0;
                case (alu_op)
                    ALU_DIV: begin
                        div_is_signed <= 1'b1;
                        div_rem_sel   <= 1'b0;
                    end
                    ALU_DIVU: begin
                        div_is_signed <= 1'b0;
                        div_rem_sel   <= 1'b0;
                    end
                    ALU_REM: begin
                        div_is_signed <= 1'b1;
                        div_rem_sel   <= 1'b1;
                    end
                    ALU_REMU: begin
                        div_is_signed <= 1'b0;
                        div_rem_sel   <= 1'b1;
                    end
                    default: begin
                        div_is_signed <= 1'b0;
                        div_rem_sel   <= 1'b0;
                    end
                endcase

                case (alu_op)
                    ALU_MUL:    mul_op_type <= 2'b00;
                    ALU_MULH:   mul_op_type <= 2'b01;
                    ALU_MULHSU: mul_op_type <= 2'b10;
                    ALU_MULHU:  mul_op_type <= 2'b11;
                    default:    mul_op_type <= 2'b00;
                endcase
            end else if (pending_operation_reg) begin
                if (req_is_minmax_reg) begin
                    case (minmax_state_reg)
                        MINMAX_STAGE_COMPARE_CAPTURE: begin
                            minmax_signed_lt_reg          <= minmax_signed_lt;
                            minmax_unsigned_lt_reg        <= minmax_unsigned_lt;
                            minmax_use_signed_compare_reg <= (req_op_reg == ALU_MIN) || (req_op_reg == ALU_MAX);
                            minmax_is_min_op_reg          <= (req_op_reg == ALU_MIN) || (req_op_reg == ALU_MINU);
                            minmax_state_reg              <= MINMAX_STAGE_COMPARE_SELECT;
                        end
                        MINMAX_STAGE_COMPARE_SELECT: begin
                            minmax_less_than_reg <= minmax_use_signed_compare_reg ?
                                                   minmax_signed_lt_reg :
                                                   minmax_unsigned_lt_reg;
                            minmax_state_reg <= MINMAX_STAGE_RESULT_CAPTURE;
                        end
                        MINMAX_STAGE_RESULT_CAPTURE: begin
                            minmax_min_result_reg <= minmax_less_than_reg ? req_a_reg : req_b_reg;
                            minmax_max_result_reg <= minmax_less_than_reg ? req_b_reg : req_a_reg;
                            minmax_state_reg      <= MINMAX_STAGE_OUTPUT_SELECT;
                        end
                        MINMAX_STAGE_OUTPUT_SELECT: begin
                            pending_operation_reg <= 1'b0;
                            out_data              <= minmax_is_min_op_reg ? minmax_min_result_reg : minmax_max_result_reg;
                            out_valid             <= 1'b1;
                            minmax_state_reg      <= MINMAX_STAGE_IDLE;
                        end
                        default: begin
                            pending_operation_reg <= 1'b0;
                            minmax_state_reg      <= MINMAX_STAGE_IDLE;
                        end
                    endcase
                end else if ((req_is_mul_reg && mul_ready) || (req_is_div_reg && div_ready) ||
                             (!req_is_mul_reg && !req_is_div_reg && !req_is_minmax_reg)) begin
                    pending_operation_reg <= 1'b0;
                    out_data              <= result_next;
                    out_valid             <= 1'b1;
                end
            end
        end
    end

endmodule
`default_nettype wire

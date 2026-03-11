// ALU Module - Arithmetic Logic Unit
// Implements RISC-V RV32I ALU operations
// Multi-cycle support for multiplication and division operations
// Configurable M extension support for resource-constrained FPGAs

module alu #(
    parameter bit ENABLE_M_EXT = 1'b1  // RV32M extension: Multiply/Divide (default: enabled)
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    input  logic        in_valid,
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
    logic        is_div_op;
    logic        launch_op;
    
    generate
        if (ENABLE_M_EXT) begin : gen_m_ext
            // Instantiate division unit with default 32-bit width for integer operations
            div_unit #(
                .WIDTH(32)
            ) u_div (
                .clk(clk),
                .rst_n(rst_n),
                .start(div_start),
                .is_signed(div_is_signed),
                .rem_sel(div_rem_sel),
                .dividend(a),
                .divisor(b),
                .result(div_result),
                .ready(div_ready)
            );
            
            // Detect division operations
            assign is_div_op = (alu_op == ALU_DIV)  || 
                               (alu_op == ALU_DIVU) || 
                               (alu_op == ALU_REM)  || 
                               (alu_op == ALU_REMU);
            
            // Start division when requested
            assign div_start = launch_op && is_div_op;
            
            // Configure division unit based on operation
            always_comb begin
                case (alu_op)
                    ALU_DIV: begin
                        div_is_signed = 1'b1;
                        div_rem_sel = 1'b0;  // Quotient
                    end
                    ALU_DIVU: begin
                        div_is_signed = 1'b0;
                        div_rem_sel = 1'b0;  // Quotient
                    end
                    ALU_REM: begin
                        div_is_signed = 1'b1;
                        div_rem_sel = 1'b1;  // Remainder
                    end
                    ALU_REMU: begin
                        div_is_signed = 1'b0;
                        div_rem_sel = 1'b1;  // Remainder
                    end
                    default: begin
                        div_is_signed = 1'b0;
                        div_rem_sel = 1'b0;
                    end
                endcase
            end
        end else begin : gen_no_m_ext
            // M extension disabled: No division unit
            assign div_result = 32'd0;
            assign div_ready = 1'b1;
            assign is_div_op = 1'b0;
            assign div_start = 1'b0;
            assign div_is_signed = 1'b0;
            assign div_rem_sel = 1'b0;
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
    logic        is_mul_op;
    
    generate
        if (ENABLE_M_EXT) begin : gen_multiplier
            // Instantiate multiplication unit with default 32-bit width for integer operations
            mul_unit #(
                .WIDTH(32)
            ) u_mul (
                .clk(clk),
                .rst_n(rst_n),
                .start(mul_start),
                .op_type(mul_op_type),
                .multiplicand(a),
                .multiplier(b),
                .result(mul_result),
                .ready(mul_ready)
            );
            
            // Detect multiplication operations
            assign is_mul_op = (alu_op == ALU_MUL)    ||
                               (alu_op == ALU_MULH)   ||
                               (alu_op == ALU_MULHSU) ||
                               (alu_op == ALU_MULHU);
            
            // Start multiplication when requested
            assign mul_start = launch_op && is_mul_op;
            
            // Map ALU operation to mul_unit op_type
            // op_type: 00=MUL, 01=MULH, 10=MULHSU, 11=MULHU
            always_comb begin
                case (alu_op)
                    ALU_MUL:    mul_op_type = 2'b00;
                    ALU_MULH:   mul_op_type = 2'b01;
                    ALU_MULHSU: mul_op_type = 2'b10;
                    ALU_MULHU:  mul_op_type = 2'b11;
                    default:    mul_op_type = 2'b00;
                endcase
            end
            
        end else begin : gen_no_multiplier
            // M extension disabled: No multiplier
            assign mul_result = 32'd0;
            assign mul_ready = 1'b1;
            assign is_mul_op = 1'b0;
            assign mul_start = 1'b0;
            assign mul_op_type = 2'b00;
        end
    endgenerate
    
    logic [31:0] arith_result;
    logic [31:0] bitwise_result;
    logic [31:0] shift_result;
    logic [31:0] minmax_result;
    logic [31:0] muldiv_result;
    logic        is_arith_op;
    logic        is_bitwise_op;
    logic        is_shift_op;
    logic        is_minmax_op;
    logic        minmax_compare_lt;
    logic        minmax_select_a_reg;
    logic        minmax_pending_reg;
    logic [31:0] minmax_a_reg;
    logic [31:0] minmax_b_reg;
    logic        mul_inflight_reg;
    logic        div_inflight_reg;
    logic [31:0] result_next;

    assign is_arith_op = (alu_op == ALU_ADD)  ||
                         (alu_op == ALU_SUB)  ||
                         (alu_op == ALU_SLT)  ||
                         (alu_op == ALU_SLTU);

    assign is_bitwise_op = (alu_op == ALU_AND) ||
                           (alu_op == ALU_OR)  ||
                           (alu_op == ALU_XOR);

    assign is_shift_op = (alu_op == ALU_SLL) ||
                         (alu_op == ALU_SRL) ||
                         (alu_op == ALU_SRA);

    assign is_minmax_op = (alu_op == ALU_MIN)  ||
                          (alu_op == ALU_MAX)  ||
                          (alu_op == ALU_MINU) ||
                          (alu_op == ALU_MAXU);

    // Signed MIN/MAX use signed compare; MINU/MAXU use plain unsigned compare.
    assign minmax_compare_lt = ((alu_op == ALU_MIN) || (alu_op == ALU_MAX)) ?
                               ($signed(a) < $signed(b)) :
                               (a < b);

    // Split MIN/MAX across two cycles to shorten the compare/select critical path.
    // Cycle 1 registers the comparison result and operands.
    // Cycle 2 selects the winning operand through a simple 32-bit mux.
    // Backpressure new requests while a multi-cycle operation still owns the output path.
    // Single-cycle ops do not need in-flight tracking because they accept the request and
    // register out_data/out_valid on the same clock edge.
    assign in_ready = !(div_inflight_reg || mul_inflight_reg || minmax_pending_reg);
    assign launch_op = in_valid && in_ready;

    always_comb begin
        arith_result = 32'd0;

        case (alu_op)
            ALU_ADD:  arith_result = a + b;
            ALU_SUB:  arith_result = a - b;
            ALU_SLT:  arith_result = ($signed(a) < $signed(b)) ? 32'd1 : 32'd0;
            ALU_SLTU: arith_result = (a < b) ? 32'd1 : 32'd0;
            default:  arith_result = 32'd0;
        endcase
    end

    always_comb begin
        bitwise_result = 32'd0;

        case (alu_op)
            ALU_AND: bitwise_result = a & b;
            ALU_OR:  bitwise_result = a | b;
            ALU_XOR: bitwise_result = a ^ b;
            default: bitwise_result = 32'd0;
        endcase
    end

    always_comb begin
        shift_result = 32'd0;

        case (alu_op)
            ALU_SLL: shift_result = a << b[4:0];
            ALU_SRL: shift_result = a >> b[4:0];
            ALU_SRA: shift_result = $signed(a) >>> b[4:0];
            default: shift_result = 32'd0;
        endcase
    end

    always_comb begin
        minmax_result = minmax_select_a_reg ? minmax_a_reg : minmax_b_reg;
    end

    always_comb begin
        if (!ENABLE_M_EXT) begin
            muldiv_result = 32'd0;
        end else if (is_mul_op) begin
            muldiv_result = mul_result;
        end else if (is_div_op) begin
            muldiv_result = div_result;
        end else begin
            muldiv_result = 32'd0;
        end
    end

    always_comb begin
        result_next = 32'd0;

        if (is_arith_op) begin
            result_next = arith_result;
        end else if (is_shift_op) begin
            result_next = shift_result;
        end else if (is_bitwise_op) begin
            result_next = bitwise_result;
        end else if (is_mul_op || is_div_op) begin
            result_next = muldiv_result;
        end else if (is_minmax_op) begin
            result_next = minmax_result;
        end
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            minmax_select_a_reg <= 1'b0;
            minmax_pending_reg  <= 1'b0;
            minmax_a_reg        <= 32'd0;
            minmax_b_reg        <= 32'd0;
            mul_inflight_reg    <= 1'b0;
            div_inflight_reg    <= 1'b0;
            out_data            <= 32'd0;
            out_valid           <= 1'b0;
        end else begin
            if (launch_op) begin
                out_valid <= 1'b0;

                if (is_minmax_op) begin
                    // MIN/MINU choose operand A when A < B.
                    // MAX/MAXU choose operand A when A is not less than B (greater-or-equal).
                    minmax_select_a_reg <= ((alu_op == ALU_MIN) || (alu_op == ALU_MINU)) ?
                                           minmax_compare_lt :
                                           !minmax_compare_lt;
                    minmax_pending_reg <= 1'b1;
                    minmax_a_reg       <= a;
                    minmax_b_reg       <= b;
                end else if (is_mul_op) begin
                    mul_inflight_reg <= 1'b1;
                end else if (is_div_op) begin
                    div_inflight_reg <= 1'b1;
                end else begin
                    out_data  <= result_next;
                    out_valid <= 1'b1;
                end
            end else if (minmax_pending_reg) begin
                minmax_pending_reg <= 1'b0;
                out_data           <= minmax_result;
                out_valid          <= 1'b1;
            end else if (mul_inflight_reg && mul_ready) begin
                mul_inflight_reg <= 1'b0;
                out_data         <= mul_result;
                out_valid        <= 1'b1;
            end else if (div_inflight_reg && div_ready) begin
                div_inflight_reg <= 1'b0;
                out_data         <= div_result;
                out_valid        <= 1'b1;
            end
        end
    end

endmodule

// ALU Module - Arithmetic Logic Unit
// Implements RISC-V RV32I ALU operations
// Multi-cycle support for multiplication and division operations
// Configurable M extension support for resource-constrained FPGAs

module alu #(
    parameter bit ENABLE_M_EXT = 1'b1  // RV32M extension: Multiply/Divide (default: enabled)
) (
    input  logic        clk,          // Clock for division unit
    input  logic        rst_n,        // Reset for division unit
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    input  logic        alu_start,    // Start operation (pulse)
    output logic [31:0] result,
    output logic        zero,
    output logic        alu_ready     // Operation complete
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
            assign div_start = alu_start && is_div_op;
            
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
            assign mul_start = alu_start && is_mul_op;
            
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
    
    // ============================================================
    // Combinational Result and Ready Computation
    // ============================================================
    logic [31:0] result_comb;
    logic        zero_comb;
    logic        ready_comb;
    
    // Combinational ready: waits for multi-cycle operations (div or mul)
    // For single-cycle ops, ready is asserted when alu_start is pulsed.
    // For multi-cycle ops, ready comes from the sub-unit (div_ready/mul_ready).
    assign ready_comb = is_div_op ? div_ready : (is_mul_op ? mul_ready : alu_start);

    always_comb begin
        // Default initialization to avoid latches
        result_comb = 32'd0;
        
        case (alu_op)
            // RV32I operations
            ALU_ADD:  result_comb = a + b;
            ALU_SUB:  result_comb = a - b;
            ALU_AND:  result_comb = a & b;
            ALU_OR:   result_comb = a | b;
            ALU_XOR:  result_comb = a ^ b;
            ALU_SLL:  result_comb = a << b[4:0];
            ALU_SRL:  result_comb = a >> b[4:0];
            ALU_SRA:  result_comb = $signed(a) >>> b[4:0];
            ALU_SLT:  result_comb = ($signed(a) < $signed(b)) ? 32'd1 : 32'd0;
            ALU_SLTU: result_comb = (a < b) ? 32'd1 : 32'd0;
            
            // M Extension - Multiplication operations (using multi-cycle mul_unit)
            ALU_MUL,
            ALU_MULH,
            ALU_MULHSU,
            ALU_MULHU: begin
                if (ENABLE_M_EXT) begin
                    result_comb = mul_result;  // Comes from multiplication unit
                end else begin
                    result_comb = 32'd0;  // M extension disabled
                end
            end
            
            // M Extension - Division operations (multi-cycle via division unit)
            ALU_DIV,
            ALU_DIVU,
            ALU_REM,
            ALU_REMU: begin
                if (ENABLE_M_EXT) begin
                    result_comb = div_result;  // Comes from division unit
                end else begin
                    result_comb = 32'd0;  // M extension disabled
                end
            end
            
            // A Extension - MIN/MAX operations (for atomic instructions)
            ALU_MIN:  result_comb = ($signed(a) < $signed(b)) ? a : b;  // Signed minimum
            ALU_MAX:  result_comb = ($signed(a) > $signed(b)) ? a : b;  // Signed maximum
            ALU_MINU: result_comb = (a < b) ? a : b;  // Unsigned minimum
            ALU_MAXU: result_comb = (a > b) ? a : b;  // Unsigned maximum
            
            default:  result_comb = 32'd0;
        endcase
    end

    assign zero_comb = (result_comb == 32'd0);
    
    // ============================================================
    // Registered Output Stage
    // ============================================================
    // Register result, zero, and alu_ready for better FPGA timing closure.
    // All outputs become valid on the clock edge following computation,
    // ensuring result and alu_ready are always synchronized.
    //
    // alu_ready is "sticky": once asserted, it stays high until a new
    // alu_start pulse clears it. This ensures downstream logic (which may
    // need multiple cycles to respond, e.g., memory through a bus arbiter)
    // can observe alu_ready for as long as needed.
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            result    <= 32'd0;
            zero      <= 1'b0;
            alu_ready <= 1'b0;
        end else begin
            result <= result_comb;
            zero   <= zero_comb;
            if (alu_start) begin
                // New operation starting: set ready based on whether it's
                // a single-cycle op (ready_comb=alu_start=1) or multi-cycle (ready_comb=0)
                alu_ready <= ready_comb;
            end else if (ready_comb) begin
                // Multi-cycle op completed: assert ready
                alu_ready <= 1'b1;
            end
            // else: hold current alu_ready value (sticky)
        end
    end

endmodule

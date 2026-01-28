// ALU Module - Arithmetic Logic Unit
// Implements RISC-V RV32I ALU operations
// Multi-cycle support for division operations
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
    
    // ALU ready signal: immediate for non-div ops, waits for div_ready for division
    assign alu_ready = is_div_op ? div_ready : 1'b1;
    
    // ============================================================
    // M Extension: Shared Multiplier (Conditional Generation)
    // ============================================================
    // Consolidated multiplier for MUL, MULH, MULHSU, MULHU
    // Uses single 64x64 signed multiplication with proper operand extension
    logic signed [63:0] mul_a_ext;
    logic signed [63:0] mul_b_ext;
    logic signed [63:0] mul_result;
    
    generate
        if (ENABLE_M_EXT) begin : gen_multiplier
            // Operand preparation: Extend operands based on operation type
            always_comb begin
                case (alu_op)
                    ALU_MUL, ALU_MULH: begin
                        // Signed × Signed
                        mul_a_ext = {{32{a[31]}}, a};  // Sign-extend
                        mul_b_ext = {{32{b[31]}}, b};  // Sign-extend
                    end
                    ALU_MULHSU: begin
                        // Signed × Unsigned
                        mul_a_ext = {{32{a[31]}}, a};  // Sign-extend
                        mul_b_ext = {32'b0, b};        // Zero-extend
                    end
                    ALU_MULHU: begin
                        // Unsigned × Unsigned
                        mul_a_ext = {32'b0, a};        // Zero-extend
                        mul_b_ext = {32'b0, b};        // Zero-extend
                    end
                    default: begin
                        mul_a_ext = 64'sd0;
                        mul_b_ext = 64'sd0;
                    end
                endcase
            end
            
            // Single shared 64x64 multiplier
            assign mul_result = mul_a_ext * mul_b_ext;
            
        end else begin : gen_no_multiplier
            // M extension disabled: No multiplier
            assign mul_a_ext = 64'sd0;
            assign mul_b_ext = 64'sd0;
            assign mul_result = 64'sd0;
        end
    endgenerate

    always_comb begin
        // Default initialization to avoid latches
        result = 32'd0;
        
        case (alu_op)
            // RV32I operations
            ALU_ADD:  result = a + b;
            ALU_SUB:  result = a - b;
            ALU_AND:  result = a & b;
            ALU_OR:   result = a | b;
            ALU_XOR:  result = a ^ b;
            ALU_SLL:  result = a << b[4:0];
            ALU_SRL:  result = a >> b[4:0];
            ALU_SRA:  result = $signed(a) >>> b[4:0];
            ALU_SLT:  result = ($signed(a) < $signed(b)) ? 32'd1 : 32'd0;
            ALU_SLTU: result = (a < b) ? 32'd1 : 32'd0;
            
            // M Extension - Multiplication operations (using shared multiplier)
            ALU_MUL: begin
                if (ENABLE_M_EXT) begin
                    result = mul_result[31:0];  // Lower 32 bits
                end else begin
                    result = 32'd0;  // M extension disabled
                end
            end
            ALU_MULH,
            ALU_MULHSU,
            ALU_MULHU: begin
                if (ENABLE_M_EXT) begin
                    result = mul_result[63:32];  // Upper 32 bits
                end else begin
                    result = 32'd0;  // M extension disabled
                end
            end
            
            // M Extension - Division operations (multi-cycle via division unit)
            ALU_DIV,
            ALU_DIVU,
            ALU_REM,
            ALU_REMU: begin
                if (ENABLE_M_EXT) begin
                    result = div_result;  // Comes from division unit
                end else begin
                    result = 32'd0;  // M extension disabled
                end
            end
            
            // A Extension - MIN/MAX operations (for atomic instructions)
            ALU_MIN:  result = ($signed(a) < $signed(b)) ? a : b;  // Signed minimum
            ALU_MAX:  result = ($signed(a) > $signed(b)) ? a : b;  // Signed maximum
            ALU_MINU: result = (a < b) ? a : b;  // Unsigned minimum
            ALU_MAXU: result = (a > b) ? a : b;  // Unsigned maximum
            
            default:  result = 32'd0;
        endcase
    end

    assign zero = (result == 32'd0);

endmodule

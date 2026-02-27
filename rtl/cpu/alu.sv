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
    // M Extension: Shared Iterative M Unit (MUL/DIV)
    // ============================================================
    logic        is_div_op;
    logic        is_mul_op;
    logic        is_m_op;
    logic        m_start;
    logic [2:0]  m_op_type;
    logic [31:0] m_result;
    logic        m_ready;

    assign is_div_op = (alu_op == ALU_DIV)  ||
                       (alu_op == ALU_DIVU) ||
                       (alu_op == ALU_REM)  ||
                       (alu_op == ALU_REMU);

    assign is_mul_op = (alu_op == ALU_MUL)    ||
                       (alu_op == ALU_MULH)   ||
                       (alu_op == ALU_MULHSU) ||
                       (alu_op == ALU_MULHU);

    assign is_m_op = is_div_op || is_mul_op;
    assign m_start = alu_start && is_m_op;

    // m_op_type encoding:
    // 000=MUL 001=MULH 010=MULHSU 011=MULHU 100=DIV 101=DIVU 110=REM 111=REMU
    always_comb begin
        case (alu_op)
            ALU_MUL:    m_op_type = 3'b000;
            ALU_MULH:   m_op_type = 3'b001;
            ALU_MULHSU: m_op_type = 3'b010;
            ALU_MULHU:  m_op_type = 3'b011;
            ALU_DIV:    m_op_type = 3'b100;
            ALU_DIVU:   m_op_type = 3'b101;
            ALU_REM:    m_op_type = 3'b110;
            ALU_REMU:   m_op_type = 3'b111;
            default:    m_op_type = 3'b000;
        endcase
    end

    generate
        if (ENABLE_M_EXT) begin : gen_m_ext
            m_unit #(
                .WIDTH(32)
            ) u_m_unit (
                .clk(clk),
                .rst_n(rst_n),
                .start(m_start),
                .op_type(m_op_type),
                .a(a),
                .b(b),
                .result(m_result),
                .ready(m_ready)
            );
        end else begin : gen_no_m_ext
            assign m_result = 32'd0;
            assign m_ready = 1'b1;
        end
    endgenerate

    // ALU ready signal: waits for shared multi-cycle M operations
    assign alu_ready = is_m_op ? m_ready : 1'b1;

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
            
            // M Extension - Multiplication operations (shared m_unit)
            ALU_MUL,
            ALU_MULH,
            ALU_MULHSU,
            ALU_MULHU: begin
                if (ENABLE_M_EXT) begin
                    result = m_result;
                end else begin
                    result = 32'd0;
                end
            end
            
            // M Extension - Division operations (shared m_unit)
            ALU_DIV,
            ALU_DIVU,
            ALU_REM,
            ALU_REMU: begin
                if (ENABLE_M_EXT) begin
                    result = m_result;
                end else begin
                    result = 32'd0;
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

// ALU Module - Arithmetic Logic Unit
// Implements RISC-V RV32I ALU operations

module alu (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    output logic [31:0] result,
    output logic        zero
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

    // Multiplication intermediate results (64-bit)
    logic [63:0] mul_result;
    logic signed [63:0] mulhsu_a_ext;
    logic [63:0] mulhsu_b_ext;

    always_comb begin
        // Default initialization to avoid latches
        mul_result = 64'd0;
        result = 32'd0;
        mulhsu_a_ext = 64'sd0;
        mulhsu_b_ext = 64'd0;
        
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
            
            // M Extension - Multiplication operations
            ALU_MUL: begin
                mul_result = $signed(a) * $signed(b);
                result = mul_result[31:0];  // Lower 32 bits
            end
            ALU_MULH: begin
                mul_result = $signed(a) * $signed(b);
                result = mul_result[63:32];  // Upper 32 bits (signed×signed)
            end
            ALU_MULHSU: begin
                // MULHSU: signed(rs1) × unsigned(rs2), upper 32 bits
                // Sign-extend a to 64-bit, zero-extend b to 64-bit, then multiply
                mulhsu_a_ext = {{32{a[31]}}, a};  // Sign-extend 32-bit to 64-bit
                mulhsu_b_ext = {32'b0, b};  // Zero-extend 32-bit to 64-bit
                mul_result = $signed(mulhsu_a_ext) * $signed(mulhsu_b_ext);  // Multiply as signed
                result = mul_result[63:32];  // Upper 32 bits
            end
            ALU_MULHU: begin
                mul_result = $unsigned(a) * $unsigned(b);
                result = mul_result[63:32];  // Upper 32 bits (unsigned×unsigned)
            end
            
            // M Extension - Division operations
            ALU_DIV: begin
                // Signed division with special cases
                if (b == 32'd0) begin
                    result = 32'hFFFFFFFF;  // Division by zero
                end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
                    result = 32'h80000000;  // Overflow case: -2^31 ÷ -1 = -2^31
                end else begin
                    result = $signed(a) / $signed(b);
                end
            end
            ALU_DIVU: begin
                // Unsigned division
                if (b == 32'd0) begin
                    result = 32'hFFFFFFFF;  // Division by zero
                end else begin
                    result = $unsigned(a) / $unsigned(b);
                end
            end
            
            // M Extension - Remainder operations
            ALU_REM: begin
                // Signed remainder
                if (b == 32'd0) begin
                    result = a;  // Division by zero: return dividend
                end else if (a == 32'h80000000 && b == 32'hFFFFFFFF) begin
                    result = 32'd0;  // Overflow case: -2^31 % -1 = 0
                end else begin
                    result = $signed(a) % $signed(b);
                end
            end
            ALU_REMU: begin
                // Unsigned remainder
                if (b == 32'd0) begin
                    result = a;  // Division by zero: return dividend
                end else begin
                    result = $unsigned(a) % $unsigned(b);
                end
            end
            
            default:  result = 32'd0;
        endcase
    end

    assign zero = (result == 32'd0);

endmodule

// Multiplication Unit Module
// Hardware-synthesizable multiplication using Shift-and-Add Algorithm
// Parameterizable width for all RISC-V M-extension multiply operations
// Supports: MUL, MULH, MULHSU, MULHU

module mul_unit #(
    parameter int WIDTH = 32,                // Bit width of operands (default 32-bit for RV32IM integer ops)
    parameter bit USE_DIRECT_MULTIPLY = 1'b0 // When set, infer hardware multiplier via Verilog * operator
) (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control interface
    input  logic        start,        // Start multiplication (pulse)
    input  logic [1:0]  op_type,      // 00=MUL, 01=MULH, 10=MULHSU, 11=MULHU
    
    // Data interface
    input  logic [WIDTH-1:0] multiplicand,  // First operand (rs1)
    input  logic [WIDTH-1:0] multiplier,    // Second operand (rs2)
    output logic [WIDTH-1:0] result,        // Lower 32-bits for MUL, upper 32-bits for MULH*
    output logic             ready          // Result valid
);

    generate
        if (USE_DIRECT_MULTIPLY) begin : gen_direct_multiply
            logic [WIDTH-1:0] direct_abs_multiplicand;
            logic [WIDTH-1:0] direct_abs_multiplier;
            logic [2*WIDTH-1:0] direct_product_abs;
            logic [2*WIDTH-1:0] direct_final_product;
            logic               direct_result_negative;
            logic [WIDTH-1:0]   multiplicand_reg;
            logic [WIDTH-1:0]   multiplier_reg;
            logic               result_negative_reg;
            logic [1:0]         product_op_type_reg;
            logic               input_valid_reg;
            logic [2*WIDTH-1:0] product_reg;
            logic [1:0]         op_type_reg;
            logic               product_valid_reg;
            logic [WIDTH-1:0]   result_reg;
            logic               ready_reg;

            always_comb begin
                direct_abs_multiplicand = multiplicand;
                direct_abs_multiplier = multiplier;
                direct_result_negative = 1'b0;

                if ((op_type == 2'b00) || (op_type == 2'b01) || (op_type == 2'b10)) begin
                    direct_abs_multiplicand = multiplicand[WIDTH-1] ? (~multiplicand + 1'b1) : multiplicand;
                end

                if ((op_type == 2'b00) || (op_type == 2'b01)) begin
                    direct_abs_multiplier = multiplier[WIDTH-1] ? (~multiplier + 1'b1) : multiplier;
                    direct_result_negative = multiplicand[WIDTH-1] ^ multiplier[WIDTH-1];
                end else if (op_type == 2'b10) begin
                    direct_result_negative = multiplicand[WIDTH-1];
                end

                direct_product_abs = multiplicand_reg * multiplier_reg;

                if (result_negative_reg && (direct_product_abs != '0)) begin
                    direct_final_product = ~direct_product_abs + 1'b1;
                end else begin
                    direct_final_product = direct_product_abs;
                end
            end

            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    multiplicand_reg <= '0;
                    multiplier_reg <= '0;
                    result_negative_reg <= 1'b0;
                    product_op_type_reg <= '0;
                    input_valid_reg <= 1'b0;
                    product_reg <= '0;
                    op_type_reg <= '0;
                    product_valid_reg <= 1'b0;
                    result_reg <= '0;
                    ready_reg <= 1'b0;
                end else begin
                    input_valid_reg <= start;
                    product_valid_reg <= input_valid_reg;
                    ready_reg <= product_valid_reg;

                    if (start) begin
                        multiplicand_reg <= direct_abs_multiplicand;
                        multiplier_reg <= direct_abs_multiplier;
                        result_negative_reg <= direct_result_negative;
                        product_op_type_reg <= op_type;
                    end

                    if (input_valid_reg) begin
                        product_reg <= direct_final_product;
                        op_type_reg <= product_op_type_reg;
                    end

                    if (product_valid_reg) begin
                        if (op_type_reg == 2'b00)
                            result_reg <= product_reg[WIDTH-1:0];
                        else
                            result_reg <= product_reg[2*WIDTH-1:WIDTH];
                    end
                end
            end

            assign ready = ready_reg;
            assign result = result_reg;
        end else begin : gen_iterative_multiply
            // ============================================================
            // State Machine Definition
            // ============================================================
            typedef enum logic [1:0] {
                MUL_IDLE = 2'b00,  // Waiting for start
                MUL_INIT = 2'b01,  // Initialize registers and handle signs
                MUL_ITER = 2'b10,  // Perform WIDTH iterations (shift-and-add)
                MUL_DONE = 2'b11   // Result ready
            } mul_state_t;
            
            mul_state_t state, next_state;
            
            // ============================================================
            // Internal Registers
            // ============================================================
            
            // Multiplication working registers
            // product[2*WIDTH-1:WIDTH] = partial product (accumulator)
            // product[WIDTH-1:0] = multiplier (shifted right each iteration)
            logic [2*WIDTH-1:0] product;              // Combined accumulator and multiplier
            logic [WIDTH-1:0]   mcand;                // Multiplicand (constant during iteration)
            logic [$clog2(WIDTH)-1:0] iter_count;     // Iteration counter (0 to WIDTH-1)
            
            // Sign tracking for result correction
            logic        result_negative;   // Final result should be negated
            logic [1:0]  op_type_reg;       // Registered operation type
            
            // Intermediate values (combinational)
            logic [WIDTH-1:0] abs_multiplicand;
            logic [WIDTH-1:0] abs_multiplier;
            logic [2*WIDTH-1:0] final_product;
            
            // For shift-and-add: add multiplicand to upper half if LSB of lower half is 1
            logic [WIDTH:0] add_result;  // WIDTH+1 bits to capture carry
            
            // ============================================================
            // State Register
            // ============================================================
            always_ff @(posedge clk) begin
                if (!rst_n)
                    state <= MUL_IDLE;
                else
                    state <= next_state;
            end
            
            // ============================================================
            // Next State Logic
            // ============================================================
            always_comb begin
                next_state = state;
                
                case (state)
                    MUL_IDLE: begin
                        if (start)
                            next_state = MUL_INIT;
                    end
                    
                    MUL_INIT: begin
                        next_state = MUL_ITER;
                    end
                    
                    MUL_ITER: begin
                        /* verilator lint_off WIDTHEXPAND */
                        if (iter_count == (WIDTH-1))  // After WIDTH iterations (0 to WIDTH-1)
                        /* verilator lint_on WIDTHEXPAND */
                            next_state = MUL_DONE;
                        else
                            next_state = MUL_ITER;
                    end
                    
                    MUL_DONE: begin
                        // Return to IDLE unconditionally
                        next_state = MUL_IDLE;
                    end
                    
                    default: next_state = MUL_IDLE;
                endcase
            end
            
            // ============================================================
            // Combinational logic for absolute value conversion
            // ============================================================
            always_comb begin
                // Default: pass through unchanged (for unsigned operations)
                abs_multiplicand = multiplicand;
                abs_multiplier = multiplier;
                
                // Compute absolute values when in INIT state for signed operations
                if (state == MUL_INIT) begin
                    // Handle multiplicand (rs1) - signed for MUL, MULH, MULHSU
                    // op_type: 00=MUL, 01=MULH, 10=MULHSU, 11=MULHU
                    if ((op_type == 2'b00) || (op_type == 2'b01) || (op_type == 2'b10)) begin
                        abs_multiplicand = multiplicand[WIDTH-1] ? (~multiplicand + 1'b1) : multiplicand;
                    end
                    
                    // Handle multiplier (rs2) - signed for MUL, MULH only (NOT for MULHSU)
                    if ((op_type == 2'b00) || (op_type == 2'b01)) begin
                        abs_multiplier = multiplier[WIDTH-1] ? (~multiplier + 1'b1) : multiplier;
                    end
                end
                
                // Compute add result: upper half of product + multiplicand (with carry)
                add_result = {1'b0, product[2*WIDTH-1:WIDTH]} + {1'b0, mcand};
            end
            
            // ============================================================
            // Datapath Registers
            // ============================================================
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    product <= '0;
                    mcand <= '0;
                    iter_count <= '0;
                    result_negative <= 1'b0;
                    op_type_reg <= '0;
                end else begin
                    case (state)
                        MUL_INIT: begin
                            // Register the operation type for later use
                            op_type_reg <= op_type;
                            
                            // Determine if final result needs to be negated
                            // op_type: 00=MUL, 01=MULH, 10=MULHSU, 11=MULHU
                            if ((op_type == 2'b00) || (op_type == 2'b01)) begin  // MUL or MULH
                                result_negative <= multiplicand[WIDTH-1] ^ multiplier[WIDTH-1];
                            end else if (op_type == 2'b10) begin  // MULHSU
                                result_negative <= multiplicand[WIDTH-1];
                            end else begin  // MULHU
                                result_negative <= 1'b0;
                            end
                            
                            // Initialize: product = {0, abs_multiplier}
                            // Upper half is accumulator (starts at 0), lower half is multiplier
                            product <= {{WIDTH{1'b0}}, abs_multiplier};
                            mcand <= abs_multiplicand;
                            iter_count <= '0;
                        end
                        
                        MUL_ITER: begin
                            // Shift-and-Add Algorithm:
                            // 1. If LSB of product (multiplier bit) is 1, add multiplicand to upper half
                            // 2. Shift entire product right by 1 (carrying any add result)
                            
                            if (product[0]) begin
                                // Add multiplicand to upper half, then shift right
                                // add_result[WIDTH] is the carry bit, which becomes the new MSB
                                product <= {add_result[WIDTH:0], product[WIDTH-1:1]};
                            end else begin
                                // Just shift right (no add needed)
                                product <= {1'b0, product[2*WIDTH-1:1]};
                            end
                            
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
                // Apply sign to product if necessary
                if (result_negative && (product != '0)) begin
                    final_product = ~product + 1'b1;  // Two's complement negation
                end else begin
                    final_product = product;
                end
            end
            
            // ============================================================
            // Output Logic (Combinational)
            // ============================================================
            always_comb begin
                ready = (state == MUL_DONE);
                
                if (state == MUL_DONE) begin
                    // Select lower or upper WIDTH bits based on operation type
                    // op_type: 00=MUL, 01=MULH, 10=MULHSU, 11=MULHU
                    if (op_type_reg == 2'b00)  // MUL
                        result = final_product[WIDTH-1:0];        // MUL: lower WIDTH bits
                    else
                        result = final_product[2*WIDTH-1:WIDTH];  // MULH/MULHSU/MULHU: upper WIDTH bits
                end else begin
                    result = '0;  // Default when not ready
                end
            end
        end
    endgenerate

endmodule

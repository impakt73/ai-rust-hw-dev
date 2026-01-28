// FPU Fused Multiply-Add Module
// Implements (a * b) +/- c with sign control
// This chains the multiplier and adder modules

module fpu_fma (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [31:0] c,
    input  logic        negate_product,   // If 1, negate (a*b)
    input  logic        negate_addend,    // If 1, subtract c instead of add
    output logic [31:0] result,
    output logic [4:0]  flags
);

    // Intermediate signals
    logic [31:0] product;
    logic [4:0]  mul_flags;
    logic [31:0] product_signed;
    logic [4:0]  add_flags;
    
    // Instantiate multiplier
    fpu_multiplier u_mul (
        .a(a),
        .b(b),
        .result(product),
        .flags(mul_flags)
    );
    
    // Apply product sign negation if needed
    assign product_signed = negate_product ? {~product[31], product[30:0]} : product;
    
    // Instantiate adder (with is_sub control for addend)
    fpu_adder u_add (
        .a(product_signed),
        .b(c),
        .is_sub(negate_addend),
        .result(result),
        .flags(add_flags)
    );
    
    // Combine flags (prioritize mul flags, then add flags)
    assign flags = mul_flags | add_flags;

endmodule

// Instruction Fetch Unit (Simplified - No Buffering)
// Simply passes raw memory data based on PC alignment
// When PC is half-word aligned (after compressed instruction), 
// we need to read from the NEXT word to get the upper half of a potential 32-bit instruction

module ifetch (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] pc,              // Current PC (2-byte aligned)
    input  logic [31:0] imem_data,       // 32-bit word from memory
    output logic [31:0] imem_addr,       // Word-aligned address for memory
    output logic [31:0] instruction,     // Raw memory data output
    output logic        valid,           // Always valid when not in reset
    output logic [1:0]  imem_size        // Always word size
);

    // Memory address calculation
    // When PC is half-word aligned, read from next word (PC+2 word-aligned)
    // so we can get the upper half of a potential 32-bit instruction
    assign imem_addr = pc[1] ? {pc[31:2], 2'b00} + 32'd4 : {pc[31:2], 2'b00};
    
    // Pass through raw memory data
    assign instruction = imem_data;
    
    // Always valid (no buffering, no complex state)
    assign valid = 1'b1;
    
    // Always read full words
    assign imem_size = 2'b10;  // Word read

endmodule

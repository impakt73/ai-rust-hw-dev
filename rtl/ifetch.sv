// Instruction Fetch Unit
// Manages instruction fetching with 16/32-bit width awareness
// Handles PC at both word and half-word alignment

module ifetch (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] pc,              // Current PC (2-byte aligned)
    input  logic [31:0] imem_data,       // 32-bit word from memory
    output logic [31:0] imem_addr,       // Word-aligned address for memory
    output logic [31:0] instruction,     // Full 32-bit instruction output
    output logic        valid            // Instruction is valid
);

    // Buffer for upper 16 bits when PC is word-aligned
    logic [15:0] buffered_half;
    logic        buffer_valid;
    
    // Word-align memory address (mask off lower 2 bits)
    assign imem_addr = {pc[31:2], 2'b00};
    
    // Current 16-bit instruction being fetched
    logic [15:0] current_half;
    
    // Determine which half of the word to use
    always_comb begin
        if (!pc[1]) begin
            // PC is word-aligned: use lower 16 bits
            current_half = imem_data[15:0];
        end else begin
            // PC is half-word aligned: use buffered upper 16 bits
            current_half = buffered_half;
        end
    end
    
    // Check if current instruction is compressed or standard
    logic is_compressed_insn;
    assign is_compressed_insn = (current_half[1:0] != 2'b11);
    
    // Build full instruction
    always_comb begin
        if (is_compressed_insn) begin
            // 16-bit compressed instruction
            instruction = {16'h0000, current_half};
            valid = 1'b1;
        end else begin
            // 32-bit standard instruction
            if (!pc[1]) begin
                // PC is word-aligned: both halves are in imem_data
                instruction = imem_data;
                valid = 1'b1;
            end else begin
                // PC is half-word aligned: lower half is buffered, upper half is in imem_data
                instruction = {imem_data[15:0], current_half};
                valid = 1'b1;
            end
        end
    end
    
    // Buffer management
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            buffered_half <= 16'h0000;
            buffer_valid <= 1'b0;
        end else begin
            if (!pc[1]) begin
                // PC is word-aligned: buffer the upper 16 bits for potential next use
                buffered_half <= imem_data[31:16];
                buffer_valid <= 1'b1;
            end else begin
                // PC is half-word aligned: buffer the lower 16 bits for next fetch
                buffered_half <= imem_data[15:0];
                buffer_valid <= 1'b1;
            end
        end
    end

endmodule

// RV32C Instruction Fetch Unit
// Handles fetching of both 16-bit compressed and 32-bit standard instructions
//
// This module manages the complexity of:
// - PC that can be 2-byte aligned (not just 4-byte aligned)
// - Buffering instruction halves when PC crosses word boundaries
// - Assembling complete 32-bit instructions from two memory fetches
// - Buffer invalidation on jumps/branches to prevent stale data usage
//
// CRITICAL DESIGN NOTES (from PR #40 learnings):
// - When PC[1]==1 and instruction is 32-bit, must fetch from PC+4 to get upper 16 bits
// - Buffer management is the most error-prone aspect - must track which bytes come from which address
// - Always verify byte selection in VCD dumps during debugging

module ifetch (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] pc,              // Current PC (must be 2-byte aligned: PC[0] must be 0)
    input  logic [31:0] imem_data,       // 32-bit word from memory
    input  logic        pc_valid,        // PC changed due to branch/jump (invalidates buffer)
    output logic [31:0] imem_addr,       // Word-aligned address for memory
    output logic [31:0] instruction,     // Complete instruction (16 or 32-bit, in lower bits)
    output logic        is_compressed,   // 1 if fetched instruction is compressed (16-bit)
    output logic        fetch_valid      // Instruction is valid and ready
);

    // Internal buffering state
    logic [15:0] buffered_half;    // Buffered upper 16 bits from previous fetch
    logic        buffer_valid;      // Buffer contains valid data
    logic [15:0] lower_half;        // Current lower 16 bits to examine
    logic [15:0] upper_half;        // Current upper 16 bits
    logic        is_32bit;          // Current instruction is 32-bit (not compressed)
    
    // Memory address calculation
    // CRITICAL: When PC[1]==1 AND instruction is 32-bit, need to fetch from PC+4 for upper half
    // However, we don't know if it's 32-bit until we examine the buffered data
    // So we always fetch from word containing PC first, then handle assembly
    assign imem_addr = {pc[31:2], 2'b00};
    
    // Extract halves from current memory fetch
    assign lower_half = imem_data[15:0];
    assign upper_half = imem_data[31:16];
    
    // Detect if current instruction is 32-bit
    // Compressed instructions have bits [1:0] != 2'b11
    always_comb begin
        if (!pc[1]) begin
            // PC is word-aligned: check lower half of current fetch
            is_32bit = (lower_half[1:0] == 2'b11);
        end else begin
            // PC is half-word aligned: check buffered half from previous fetch
            is_32bit = buffer_valid && (buffered_half[1:0] == 2'b11);
        end
    end
    
    // Set compressed flag (output)
    assign is_compressed = !is_32bit;
    
    // Main fetch logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            buffered_half <= 16'h0;
            buffer_valid <= 1'b0;
            instruction <= 32'h0;
            fetch_valid <= 1'b0;
        end else begin
            // CRITICAL: Invalidate buffer on jumps/branches to prevent stale data usage
            if (pc_valid) begin
                buffer_valid <= 1'b0;
                fetch_valid <= 1'b0;
            end else begin
                fetch_valid <= 1'b1;  // Instruction will be ready this cycle
            end
            
            if (!pc[1]) begin
                // ===== PC is WORD-ALIGNED (PC[1] == 0) =====
                // Fetch from word containing PC
                if (is_32bit) begin
                    // 32-bit instruction starting at word boundary
                    // Both halves are in current fetch: lower_half is bits [15:0], upper_half is bits [31:16]
                    instruction <= imem_data;
                    buffer_valid <= 1'b0;  // No buffering needed, used complete word
                end else begin
                    // 16-bit compressed instruction at word boundary
                    instruction <= {16'h0, lower_half};
                    // Buffer upper half in case next PC is half-word aligned
                    buffered_half <= upper_half;
                    buffer_valid <= 1'b1;
                end
            end else begin
                // ===== PC is HALF-WORD ALIGNED (PC[1] == 1) =====
                // Need to use buffered data from previous fetch
                if (buffer_valid) begin
                    if (is_32bit) begin
                        // 32-bit instruction starting at half-word boundary
                        // CRITICAL CASE: Instruction spans two memory words
                        // Lower 16 bits: buffered_half (from previous fetch at PC-2)
                        // Upper 16 bits: lower_half (from current fetch at PC+2)
                        // 
                        // Memory layout example:
                        //   PC-2: [compressed][buffered_half]  <- previous fetch
                        //   PC:   [lower_half][next_insn]      <- current fetch
                        // 
                        // We assemble: {lower_half, buffered_half}
                        instruction <= {lower_half, buffered_half};
                        // Update buffer with upper half for potential next use
                        buffered_half <= upper_half;
                        buffer_valid <= 1'b1;
                    end else begin
                        // 16-bit compressed instruction at half-word boundary
                        instruction <= {16'h0, buffered_half};
                        // Update buffer with lower half of current fetch
                        buffered_half <= lower_half;
                        buffer_valid <= 1'b1;
                    end
                end else begin
                    // Buffer invalid (e.g., after jump to half-word aligned address)
                    // This is the first fetch after a jump
                    // PC points to half-word boundary, so instruction is in upper half of fetched word
                    // Note: We can't know if it's 16 or 32-bit yet without examining it
                    // For now, fetch the upper half and mark it as potentially incomplete
                    instruction <= {16'h0, upper_half};
                    // Check if this is compressed or needs another fetch
                    // If upper_half[1:0] == 2'b11, it's a 32-bit instruction and we need more data
                    // But we'll handle that in the next cycle
                    buffer_valid <= 1'b0;  // Still need to establish proper buffer state
                    // Set fetch_valid based on whether we have a complete instruction
                    if (upper_half[1:0] != 2'b11) begin
                        // Compressed instruction - complete in 16 bits
                        fetch_valid <= 1'b1;
                    end else begin
                        // 32-bit instruction - incomplete, need next fetch
                        // This case is tricky; for simplicity, assume we'll get it next cycle
                        fetch_valid <= 1'b0;
                    end
                end
            end
        end
    end

endmodule

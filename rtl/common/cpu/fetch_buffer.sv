`default_nettype none
// RV32C Fetch Buffer Module
// Manages instruction fetch buffering for compressed (16-bit) and standard (32-bit) instructions
// Handles half-word alignment and buffer state across instruction boundaries

module fetch_buffer (
    input wire logic        clk,
    input wire logic        rst_n,
    
    // Memory interface
    input wire logic [31:0] imem_data,       // Data from instruction memory
    input wire logic        imem_ready,      // Memory has valid data
    
    // Program counter
    input wire logic [31:0] pc,              // Current PC value
    
    // Control signals
    input wire logic        ir_write,          // Write instruction to IR
    input wire logic        invalidate_buffer, // Clear buffered half-word after control flow changes
    
    // Instruction output
    output logic [31:0] instruction, // 32-bit instruction (decompressed if needed)
    output logic        valid,       // Instruction is valid
    
    // Instruction tracking
    output logic        pc_inc_2 // High for 2-byte instruction, low for 4-byte instruction
);

    // ============================================================
    // Internal Signals
    // ============================================================
    
    // Fetch buffer for handling compressed instructions at half-word boundaries
    logic [15:0] buffered_half;      // Buffered upper half-word from previous fetch
    logic        buffer_valid;       // Buffer contains valid half-word
    logic        buffer_valid_next;  // Next value for buffer_valid
    logic [15:0] buffered_half_next; // Next value for buffered_half
    
    // Assembled instruction (16-bit or 32-bit)
    logic [31:0] assembled_insn;             // Assembled instruction before decompression
    logic [15:0] current_half;               // Current half-word from memory
    logic        insn_is_compressed;         // Current assembled instruction is compressed
    logic        current_insn_compressed;    // Current executing instruction is compressed
    logic        decomp_is_compressed_internal; // Decompressor compression detect
    
    // ============================================================
    // Buffer Registers
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            buffer_valid <= 1'b0;
            current_insn_compressed <= 1'b0;
        end else begin
            buffered_half <= buffered_half_next;
            buffer_valid <= buffer_valid_next;
            // Track whether current instruction being executed is compressed
            if (ir_write)
                current_insn_compressed <= decomp_is_compressed_internal;
        end
    end
    
    // ============================================================
    // Instruction Assembly Logic
    // ============================================================
    
    // Instruction assembly: combine buffered half-word with current fetch
    // to form a complete 16-bit or 32-bit instruction
    always_comb begin
        // Determine which half-word to examine first
        if (buffer_valid) begin
            // We have a buffered half-word from previous fetch
            current_half = buffered_half;
        end else begin
            // Use lower half-word from current fetch
            current_half = imem_data[15:0];
        end
        
        // Check if current half-word is compressed (bits [1:0] != 2'b11)
        insn_is_compressed = (current_half[1:0] != 2'b11);
        
        if (insn_is_compressed) begin
            // 16-bit compressed instruction
            assembled_insn = {16'h0, current_half};
        end else begin
            // 32-bit instruction: need full word
            if (buffer_valid) begin
                // Lower half is in buffer, upper half is in current fetch
                // buffered_half contains the lower 16 bits, imem_data[31:16] contains upper 16 bits
                assembled_insn = {imem_data[31:16], buffered_half};
            end else begin
                // Both halves in current fetch (word-aligned)
                assembled_insn = imem_data;
            end
        end
        
    end

    // ============================================================
    // Decompressor
    // ============================================================
    decompress u_decompress (
        .insn_16(assembled_insn[15:0]),
        .insn_32_in(assembled_insn),
        .insn_32(instruction),
        .is_compressed(decomp_is_compressed_internal),
        .is_valid(valid)
    );
    
    // ============================================================
    // Fetch Buffer State Machine
    // ============================================================
    
    // Determine next buffer state
    always_comb begin
        buffered_half_next = buffered_half;
        buffer_valid_next = buffer_valid;
        
        if (ir_write && imem_ready) begin
            // Writing instruction to IR
            if (insn_is_compressed) begin
                // Consumed a compressed instruction (16-bit)
                if (buffer_valid) begin
                    // Used buffered half from previous fetch
                    // The current fetch contains: [15:0] = data we just used (redundant)
                    //                              [31:16] = new data we haven't processed
                    // Buffer the new data for next instruction
                    buffered_half_next = imem_data[31:16];
                    buffer_valid_next = 1'b1;
                end else if (pc[1] == 1'b0) begin
                    // PC is word-aligned, consumed lower half [15:0], buffer upper half [31:16]
                    buffered_half_next = imem_data[31:16];
                    buffer_valid_next = 1'b1;
                end else begin
                    // PC points to upper half-word (odd address), consumed upper half [31:16]
                    // No more data in this fetch to buffer
                    buffer_valid_next = 1'b0;
                end
            end else begin
                // Consumed a 32-bit instruction - used full word
                buffer_valid_next = 1'b0;  // Buffer is empty after consuming full word
            end
        end
        
        // Invalidate buffer on control flow changes (jumps/branches)
        // This happens when the CPU asserts invalidate_buffer on a control-flow change
        if (invalidate_buffer) begin
            buffer_valid_next = 1'b0;
        end
    end
    
    // ============================================================
    // PC Increment Calculation
    // ============================================================
    
    // PC increment flag based on instruction width
    always_comb begin
        pc_inc_2 = current_insn_compressed;
    end

endmodule
`default_nettype wire

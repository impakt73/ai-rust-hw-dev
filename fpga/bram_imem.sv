// Block RAM Instruction Memory
// Single-port read-only memory for instruction storage
// Synthesizes to on-chip BRAM on iCE40 FPGA

module bram_imem #(
    parameter ADDR_WIDTH = 10,  // 2^10 = 1024 words
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    req,
    output logic                    ready
);

    // Memory array
    logic [DATA_WIDTH-1:0] mem [0:(2**ADDR_WIDTH)-1];
    
    // Initialize memory with LED rotation program
    // This program rotates LED pattern left by 1 bit every second (25M cycles at 25MHz)
    initial begin
        // LED Rotation Program
        // Register allocation:
        //   x10 (a0): LED controller base address (0x50000000)
        //   x11 (a1): LED pattern register (rotating pattern)
        //   x12 (a2): Delay counter
        //   x13 (a3): Delay target (25,000,000 for 1 second at 25 MHz)
        
        // Address 0x80000000 (instruction memory base)
        
        // === Initialization ===
        // 0: lui x10, 0x50000  // Load LED base address upper (0x50000000)
        mem[0] = 32'h50000537;
        
        // 1: addi x11, x0, 0xAA  // Load initial pattern 0xAA (10101010)
        mem[1] = 32'h0AA00593;
        
        // 2: lui x13, 0x017D8  // Load upper 20 bits of 25M (0x017D7840)
        mem[2] = 32'h017D86B7;
        
        // 3: addi x13, x13, 0x7840  // Add lower 12 bits to get 25,000,000
        // Note: 0x7840 is -1984 in signed, so we need to add 0x840 (2112) carefully
        // Actually: 0x017D7840 = 25,000,000 decimal
        // lui loads 0x017D8 << 12 = 0x017D8000
        // We need to subtract 0x7C0 (1984) to get 0x017D7840
        // addi x13, x13, -1984 = addi x13, x13, 0x840
        mem[3] = 32'h84068693;
        
        // === Main Loop ===
        // 4: sw x11, 0(x10)  // Write LED pattern to controller
        mem[4] = 32'h00B52023;
        
        // 5: addi x12, x0, 0  // Initialize counter to 0
        mem[5] = 32'h00000613;
        
        // === Delay Loop (count to 25M) ===
        // 6: addi x12, x12, 1  // counter++
        mem[6] = 32'h00160613;
        
        // 7: bne x12, x13, -4  // if (counter != 25M) goto delay_loop
        mem[7] = 32'hFED61EE3;
        
        // === Rotate LED Pattern ===
        // 8: slli x14, x11, 1  // Shift left by 1: x14 = pattern << 1
        mem[8] = 32'h00159713;
        
        // 9: srli x15, x11, 7  // Shift right by 7: x15 = pattern >> 7 (get MSB)
        mem[9] = 32'h0075D793;
        
        // 10: or x11, x14, x15  // Combine: pattern = (pattern << 1) | (pattern >> 7)
        mem[10] = 32'h00F76593;
        
        // 11: andi x11, x11, 0xFF  // Mask to 8 bits
        mem[11] = 32'h0FF5F593;
        
        // 12: jal x0, -32  // Jump back to main loop (offset = -32 bytes = -8 instructions)
        mem[12] = 32'hFE1FF06F;
        
        // Fill rest with NOPs
        for (int i = 13; i < (2**ADDR_WIDTH); i++) begin
            mem[i] = 32'h00000013;  // addi x0, x0, 0 (NOP)
        end
    end
    
    // Read logic with 1-cycle latency
    logic [DATA_WIDTH-1:0] rdata_reg;
    logic ready_reg;
    
    always_ff @(posedge clk) begin
        if (req) begin
            rdata_reg <= mem[addr];
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

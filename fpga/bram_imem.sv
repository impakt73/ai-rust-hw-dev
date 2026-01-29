// Block RAM Instruction Memory
// Single-port read-only memory for instruction storage
// Synthesizes to on-chip BRAM on iCE40 FPGA
// Byte-addressed to support compressed (2-byte aligned) instructions

module bram_imem #(
    parameter ADDR_WIDTH = 12,  // 2^12 = 4096 bytes = 4 KB
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,   // Byte address
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    req,
    output logic                    ready
);

    // Memory array - stored as 32-bit words, addressed by word index
    localparam WORD_COUNT = 2**(ADDR_WIDTH-2);  // 4 KB / 4 bytes = 1024 words
    logic [DATA_WIDTH-1:0] mem [0:WORD_COUNT-1];
    
    // Word address is byte address >> 2
    logic [ADDR_WIDTH-3:0] word_addr;
    assign word_addr = addr[ADDR_WIDTH-1:2];
    
    // Initialize memory with UART-to-LED echo program
    // This program polls the UART for incoming data and displays received bytes on LEDs
    initial begin
        // UART-to-LED Echo Program
        // Register allocation:
        //   x10 (a0): LED controller base address (0x50000000)
        //   x11 (a1): UART controller base address (0x52000000)
        //   x12 (a2): UART status register value
        //   x13 (a3): Received byte / scratch
        //   x14 (a4): RX_EMPTY mask (bit 5 = 0x20)
        //
        // Memory map:
        //   LED_OUT:     0x50000000 (write to display on LEDs)
        //   UART_RXDATA: 0x52000004 (read received byte)
        //   UART_STATUS: 0x52000008 (bit 5 = RX_EMPTY)
        
        // === Initialization ===
        // 0: lui x10, 0x50000  // Load LED base address (0x50000000)
        mem[0] = 32'h50000537;
        
        // 1: lui x11, 0x52000  // Load UART base address (0x52000000)
        mem[1] = 32'h520005B7;
        
        // 2: addi x14, x0, 0x20  // Load RX_EMPTY mask (bit 5)
        mem[2] = 32'h02000713;
        
        // === Main Loop (poll_loop at address 0x0C = instruction 3) ===
        // 3: lw x12, 8(x11)  // Read UART STATUS register (offset 0x08)
        mem[3] = 32'h0085A603;
        
        // 4: and x13, x12, x14  // Mask RX_EMPTY bit
        mem[4] = 32'h00E676B3;
        
        // 5: bne x13, x0, -8  // If RX_EMPTY != 0 (FIFO empty), loop back to poll
        mem[5] = 32'hFE069CE3;
        
        // === Data Available - Read, Display, and Echo ===
        // 6: lw x13, 4(x11)  // Read byte from UART RXDATA (offset 0x04)
        mem[6] = 32'h0045A683;
        
        // 7: sw x13, 0(x10)  // Write byte to LED_OUT register
        mem[7] = 32'h00D52023;
        
        // 8: sw x13, 0(x11)  // Echo byte back to UART TXDATA (offset 0x00)
        mem[8] = 32'h00D5A023;
        
        // 9: jal x0, -24  // Jump back to poll_loop (offset = -24 bytes = -6 instructions)
        mem[9] = 32'hFD3FF06F;
        
        // Fill rest with NOPs
        for (int i = 10; i < WORD_COUNT; i++) begin
            mem[i] = 32'h00000013;  // addi x0, x0, 0 (NOP)
        end
    end
    
    // Read logic with 1-cycle latency
    logic [DATA_WIDTH-1:0] rdata_reg;
    logic ready_reg;
    
    always_ff @(posedge clk) begin
        if (req) begin
            rdata_reg <= mem[word_addr];  // Use word address for memory lookup
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

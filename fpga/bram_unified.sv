// Block RAM Unified Memory
// Single-port read/write memory for both instruction and data storage
// Synthesizes to on-chip BRAM on iCE40 FPGA
// Byte-addressed to support compressed (2-byte aligned) instructions and sub-word accesses
//
// This unified BRAM serves both instruction fetch and data access through
// a single interface. The CPU's multi-cycle FSM ensures only one type of
// access is active at a time.

module bram_unified #(
    parameter ADDR_WIDTH = 12,  // Address width in bits (size = 2^ADDR_WIDTH bytes)
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,   // Byte address
    input  logic [DATA_WIDTH-1:0]   wdata,
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    we,     // Write enable
    input  logic                    re,     // Read enable
    input  logic [1:0]              size,   // 00=byte, 01=half, 10=word
    input  logic                    req,    // Memory request
    output logic                    ready   // Memory operation complete
);

    // Memory array - stored as 32-bit words, addressed by word index
    // Size = 2^ADDR_WIDTH bytes = 2^(ADDR_WIDTH-2) words
    localparam WORD_COUNT = 2**(ADDR_WIDTH-2);
    logic [DATA_WIDTH-1:0] mem [0:WORD_COUNT-1];
    
    // Word address is byte address >> 2
    logic [ADDR_WIDTH-3:0] word_addr;
    assign word_addr = addr[ADDR_WIDTH-1:2];
    
    // Byte offset within word
    logic [1:0] byte_offset;
    assign byte_offset = addr[1:0];
    
    // Initialize memory with UART-to-LED echo program (same as bram_imem.sv)
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
    
    // Read/write logic with 1-cycle latency
    logic [DATA_WIDTH-1:0] rdata_reg;
    logic ready_reg;
    
    always_ff @(posedge clk) begin
        if (req) begin
            if (we) begin
                // Write logic with byte lane support using byte offset
                case (size)
                    2'b00: begin  // Byte write
                        case (byte_offset)
                            2'b00: mem[word_addr][7:0]   <= wdata[7:0];
                            2'b01: mem[word_addr][15:8]  <= wdata[7:0];
                            2'b10: mem[word_addr][23:16] <= wdata[7:0];
                            2'b11: mem[word_addr][31:24] <= wdata[7:0];
                        endcase
                    end
                    2'b01: begin  // Halfword write
                        if (byte_offset[1] == 1'b0) begin
                            mem[word_addr][15:0] <= wdata[15:0];
                        end else begin
                            mem[word_addr][31:16] <= wdata[15:0];
                        end
                    end
                    2'b10: begin  // Word write
                        mem[word_addr] <= wdata;
                    end
                    default: ;
                endcase
                rdata_reg <= 32'h0;
            end else if (re) begin
                // Read logic - use word address for memory lookup
                rdata_reg <= mem[word_addr];
            end else begin
                // Memory request without read or write (instruction fetch)
                // Treat as read operation
                rdata_reg <= mem[word_addr];
            end
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

// Block RAM Data Memory
// Single-port read/write memory for data storage
// Synthesizes to on-chip BRAM on iCE40 FPGA
// Byte-addressed to support sub-word (byte/halfword) accesses

module bram_dmem #(
    parameter ADDR_WIDTH = 12,  // 2^12 = 4096 bytes = 4 KB
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,   // Byte address
    input  logic [DATA_WIDTH-1:0]   wdata,
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    we,
    input  logic                    re,
    input  logic [1:0]              size,  // 00=byte, 01=half, 10=word
    input  logic                    req,
    output logic                    ready
);

    // Memory array - stored as 32-bit words, addressed by word index
    localparam WORD_COUNT = 2**(ADDR_WIDTH-2);  // 4 KB / 4 bytes = 1024 words
    logic [DATA_WIDTH-1:0] mem [0:WORD_COUNT-1];
    
    // Word address is byte address >> 2
    logic [ADDR_WIDTH-3:0] word_addr;
    assign word_addr = addr[ADDR_WIDTH-1:2];
    
    // Byte offset within word
    logic [1:0] byte_offset;
    assign byte_offset = addr[1:0];
    
    // Initialize memory to zero
    initial begin
        for (int i = 0; i < WORD_COUNT; i++) begin
            mem[i] = 32'h0;
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
                rdata_reg <= 32'h0;
            end
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

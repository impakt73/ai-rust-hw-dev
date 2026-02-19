// SRAM Peripheral
// 8KB memory-mapped SRAM peripheral with subword write masking
// Memory-mapped at 0x52000000 in RTL peripheral address space
//
// Size: 8KB (0x2000 bytes, 2048 words)
// Address Range: 0x52000000 - 0x52001FFF
//
// Features:
// - 8KB total memory (2048 x 32-bit words)
// - Subword write masking based on bus size and address alignment
// - Registered read output (1-cycle latency)

module sram_peripheral (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit)
    input  logic [31:0] wdata,     // Write data
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable
    input  logic        req,       // Memory request
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word)
    output logic        ready      // Operation complete
);

    // ============================================================
    // SRAM Configuration
    // ============================================================
    // 8KB = 8192 bytes = 2048 words (32-bit)
    // Address width: 11 bits for word addressing (2^11 = 2048)
    localparam ADDR_WIDTH = 11;
    
    // ============================================================
    // Internal Signals
    // ============================================================
    logic [ADDR_WIDTH-1:0] word_addr;
    logic [3:0]            wmask;
    logic                  sram_we;
    
    // ============================================================
    // Address Calculation
    // ============================================================
    // Extract word address (drop lower 2 bits since we're word-addressed)
    // addr[12:2] gives us 11 bits for 2048 words
    assign word_addr = addr[ADDR_WIDTH+1:2];
    
    // ============================================================
    // Write Data Alignment
    // ============================================================
    // The CPU provides write data in wdata[7:0] for all byte writes,
    // wdata[15:0] for halfword writes, regardless of address alignment.
    // We need to shift the data to the correct byte lanes based on addr[1:0].
    logic [31:0] aligned_wdata;
    
    always_comb begin
        aligned_wdata = 32'h0;
        
        case (size)
            2'b00: begin  // Byte access
                case (addr[1:0])
                    2'b00: aligned_wdata = {24'h0, wdata[7:0]};        // Byte 0
                    2'b01: aligned_wdata = {16'h0, wdata[7:0], 8'h0};  // Byte 1
                    2'b10: aligned_wdata = {8'h0, wdata[7:0], 16'h0};  // Byte 2
                    2'b11: aligned_wdata = {wdata[7:0], 24'h0};        // Byte 3
                endcase
            end
            2'b01: begin  // Halfword access
                case (addr[1])
                    1'b0: aligned_wdata = {16'h0, wdata[15:0]};        // Bytes 0-1
                    1'b1: aligned_wdata = {wdata[15:0], 16'h0};        // Bytes 2-3
                endcase
            end
            2'b10: begin  // Word access
                aligned_wdata = wdata;  // All bytes
            end
            default: aligned_wdata = 32'h0;
        endcase
    end
    
    // ============================================================
    // Write Mask Generation
    // ============================================================
    // Generate byte-level write mask based on access size and address alignment
    always_comb begin
        wmask = 4'b0000;
        
        if (we && req) begin
            case (size)
                2'b00: begin  // Byte access
                    case (addr[1:0])
                        2'b00: wmask = 4'b0001;  // Byte 0
                        2'b01: wmask = 4'b0010;  // Byte 1
                        2'b10: wmask = 4'b0100;  // Byte 2
                        2'b11: wmask = 4'b1000;  // Byte 3
                    endcase
                end
                2'b01: begin  // Halfword access
                    case (addr[1])
                        1'b0: wmask = 4'b0011;  // Bytes 0-1
                        1'b1: wmask = 4'b1100;  // Bytes 2-3
                    endcase
                end
                2'b10: begin  // Word access
                    wmask = 4'b1111;  // All bytes
                end
                default: wmask = 4'b0000;
            endcase
        end
    end
    
    // SRAM write enable: active when we have a write request
    assign sram_we = we && req;
    
    // ============================================================
    // SRAM Instantiation
    // ============================================================
    // The sram module has registered read output, providing 1-cycle latency
    // Two read ports: rdata for the word at word_addr, rdata2 for word_addr+1
    // Used together to support unaligned word reads (addr[1]=1)
    logic [31:0] sram_rdata;
    logic [31:0] sram_rdata2;
    
    sram #(
        .ADDR_WIDTH(ADDR_WIDTH)
    ) sram_inst (
        .clk(clk),
        .we(sram_we),
        .wmask(wmask),
        .waddr(word_addr),
        .wdata(aligned_wdata),  // Use aligned data
        .raddr(word_addr),
        .rdata(sram_rdata),
        .raddr2(word_addr + 1'b1),
        .rdata2(sram_rdata2)
    );
    
    // ============================================================
    // Read Data Extraction
    // ============================================================
    // Extract the requested bytes from the SRAM word based on address and size
    logic [31:0] extracted_rdata;
    
    always_comb begin
        extracted_rdata = 32'h0;
        
        case (size)
            2'b00: begin  // Byte access
                case (addr[1:0])
                    2'b00: extracted_rdata = {24'h0, sram_rdata[7:0]};
                    2'b01: extracted_rdata = {24'h0, sram_rdata[15:8]};
                    2'b10: extracted_rdata = {24'h0, sram_rdata[23:16]};
                    2'b11: extracted_rdata = {24'h0, sram_rdata[31:24]};
                endcase
            end
            2'b01: begin  // Halfword access
                case (addr[1])
                    1'b0: extracted_rdata = {16'h0, sram_rdata[15:0]};
                    1'b1: extracted_rdata = {16'h0, sram_rdata[31:16]};
                endcase
            end
            2'b10: begin  // Word access
                // For half-word-aligned addresses (addr[1]=1), the 32-bit value
                // spans two consecutive SRAM words.  Combine the upper half of the
                // current word with the lower half of the next word so that the
                // caller sees data starting at the requested address — matching
                // the byte-addressable DRAM behaviour expected by the CPU fetch
                // buffer for unaligned instruction fetches.
                if (addr[1])
                    extracted_rdata = {sram_rdata2[15:0], sram_rdata[31:16]};
                else
                    extracted_rdata = sram_rdata;
            end
            default: extracted_rdata = 32'h0;
        endcase
    end
    
    // ============================================================
    // Ready Signal and Read Data
    // ============================================================
    // SRAM has 1-cycle read latency due to registered output.
    // The CPU holds req high until ready is asserted.
    // 
    // Timing for reads:
    // Cycle N:   req=1, we=0, ready=0  (CPU requests read, SRAM latches address)
    // Cycle N+1: req=1, we=0, ready=1  (SRAM output ready, complete read)
    //
    // Timing for writes:
    // Cycle N:   req=1, we=1, ready=1  (Write completes in same cycle)
    
    logic read_pending;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            read_pending <= 1'b0;
        end else begin
            read_pending <= req && !we;
        end
    end
    
    // Ready logic:
    // - Writes: Ready immediately (same cycle as req && we)
    // - Reads: Ready one cycle after read request while request is still active
    assign ready = (req && we) || (req && read_pending);
    
    // Read data output - use extracted data
    assign rdata = extracted_rdata;

endmodule

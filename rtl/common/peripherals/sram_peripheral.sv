// SRAM Peripheral
// 12KB memory-mapped SRAM peripheral with subword write masking
// Memory-mapped at 0x52000000 in RTL peripheral address space
//
// Size: 12KB (0x3000 bytes, 3072 words)
// Address Range: 0x52000000 - 0x52002FFF
//
// Features:
// - 12KB total memory (3072 x 32-bit words)
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
    // 12KB = 12288 bytes = 3072 words (32-bit)
    // Address width: 12 bits for word addressing (2^12 = 4096)
    localparam ADDR_WIDTH = 12;
    localparam DEPTH_WORDS = 3072;
    
    // ============================================================
    // Internal Signals
    // ============================================================
    logic [ADDR_WIDTH-1:0] word_addr;
    logic [ADDR_WIDTH-1:0] sram_waddr;
    logic [ADDR_WIDTH-1:0] sram_raddr;
    logic [3:0]            wmask;
    logic [31:0]           sram_wdata;
    logic                  sram_we;
    logic                  req_unaligned;
    
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
    logic [1:0]  addr_offset;

    assign addr_offset = addr[1:0];
    assign req_unaligned = req && (
        ((size == 2'b01) && (addr_offset == 2'b11)) || // halfword crossing boundary
        ((size == 2'b10) && (addr_offset != 2'b00))    // word crossing boundary
    );
    
    always_comb begin
        aligned_wdata = 32'h0;
        
        case (size)
            2'b00: begin  // Byte access
                    case (addr_offset)
                        2'b00: aligned_wdata = {24'h0, wdata[7:0]};        // Byte 0
                        2'b01: aligned_wdata = {16'h0, wdata[7:0], 8'h0};  // Byte 1
                        2'b10: aligned_wdata = {8'h0, wdata[7:0], 16'h0};  // Byte 2
                        2'b11: aligned_wdata = {wdata[7:0], 24'h0};        // Byte 3
                    endcase
                end
            2'b01: aligned_wdata = wdata << ({addr_offset, 3'b000}); // Halfword
            2'b10: aligned_wdata = wdata << ({addr_offset, 3'b000}); // Word
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
                2'b00: wmask = 4'b0001 << addr_offset; // Byte access
                2'b01: wmask = 4'b0011 << addr_offset; // Halfword access
                2'b10: wmask = 4'b1111 << addr_offset; // Word access
                default: wmask = 4'b0000;
            endcase
        end
    end

    logic                  split_write_pending;
    logic                  split_read_phase1_pending;
    logic                  split_read_phase2_pending;
    logic [ADDR_WIDTH-1:0] split_word_addr;
    logic [1:0]            split_size;
    logic [1:0]            split_offset;
    logic [31:0]           split_wdata;
    logic [31:0]           split_first_rdata;
    logic [3:0]            split_second_wmask;
    logic [31:0]           split_second_wdata;

    always_comb begin
        split_second_wmask = 4'b0000;
        split_second_wdata = 32'h0;

        case (split_size)
            2'b01: begin // Halfword, only crossing case is offset 3
                split_second_wmask = 4'b0001;
                split_second_wdata = {24'h0, split_wdata[15:8]};
            end
            2'b10: begin // Word
                case (split_offset)
                    2'b01: begin
                        split_second_wmask = 4'b0001;
                        split_second_wdata = {24'h0, split_wdata[31:24]};
                    end
                    2'b10: begin
                        split_second_wmask = 4'b0011;
                        split_second_wdata = {16'h0, split_wdata[31:16]};
                    end
                    2'b11: begin
                        split_second_wmask = 4'b0111;
                        split_second_wdata = {8'h0, split_wdata[31:8]};
                    end
                    default: begin
                        split_second_wmask = 4'b0000;
                        split_second_wdata = 32'h0;
                    end
                endcase
            end
            default: begin
                split_second_wmask = 4'b0000;
                split_second_wdata = 32'h0;
            end
        endcase
    end

    always_comb begin
        sram_we    = req && we;
        sram_waddr = word_addr;
        sram_wdata = aligned_wdata;
        sram_raddr = word_addr;

        if (split_write_pending) begin
            sram_we    = req;
            sram_waddr = split_word_addr + 1'b1;
            sram_wdata = split_second_wdata;
        end else if (split_read_phase1_pending) begin
            sram_we    = 1'b0;
            sram_raddr = split_word_addr + 1'b1;
        end
    end
    
    // ============================================================
    // SRAM Instantiation
    // ============================================================
    // The sram module has registered read output, providing 1-cycle latency
    logic [31:0] sram_rdata;
    
    sram #(
        .ADDR_WIDTH(ADDR_WIDTH),
        .DEPTH(DEPTH_WORDS)
    ) sram_inst (
        .clk(clk),
        .we(sram_we),
        .wmask(split_write_pending ? split_second_wmask : wmask),
        .waddr(sram_waddr),
        .wdata(sram_wdata),
        .raddr(sram_raddr),
        .rdata(sram_rdata)
    );
    
    // ============================================================
    // Read Data Extraction
    // ============================================================
    // Extract the requested bytes from the SRAM word based on address and size
    logic [31:0] extracted_rdata;
    logic [31:0] shifted_rdata;
    logic [63:0] split_concat_rdata;
    logic [63:0] split_shifted_rdata;

    assign shifted_rdata      = sram_rdata >> ({addr_offset, 3'b000});
    assign split_concat_rdata  = {sram_rdata, split_first_rdata};
    assign split_shifted_rdata = split_concat_rdata >> ({split_offset, 3'b000});
    
    always_comb begin
        extracted_rdata = 32'h0;
        
        case (size)
            2'b00: begin  // Byte access
                case (addr_offset)
                    2'b00: extracted_rdata = {24'h0, sram_rdata[7:0]};
                    2'b01: extracted_rdata = {24'h0, sram_rdata[15:8]};
                    2'b10: extracted_rdata = {24'h0, sram_rdata[23:16]};
                    2'b11: extracted_rdata = {24'h0, sram_rdata[31:24]};
                endcase
            end
            2'b01: extracted_rdata = {16'h0, shifted_rdata[15:0]};
            2'b10: begin  // Word access
                extracted_rdata = sram_rdata;
            end
            default: extracted_rdata = 32'h0;
        endcase
    end

    logic [31:0] split_extracted_rdata;

    always_comb begin
        split_extracted_rdata = 32'h0;

        case (split_size)
            2'b01: split_extracted_rdata = {16'h0, split_shifted_rdata[15:0]};
            2'b10: split_extracted_rdata = split_shifted_rdata[31:0];
            default: split_extracted_rdata = 32'h0;
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
    
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            read_pending              <= 1'b0;
            split_write_pending       <= 1'b0;
            split_read_phase1_pending <= 1'b0;
            split_read_phase2_pending <= 1'b0;
            split_word_addr           <= '0;
            split_size                <= 2'b00;
            split_offset              <= 2'b00;
            split_wdata               <= 32'h0;
            split_first_rdata         <= 32'h0;
        end else begin
            read_pending <= req && !we && !req_unaligned &&
                            !split_write_pending && !split_read_phase1_pending && !split_read_phase2_pending;

            if (split_write_pending) begin
                split_write_pending <= 1'b0;
            end

            if (split_read_phase1_pending) begin
                split_first_rdata         <= sram_rdata;
                split_read_phase1_pending <= 1'b0;
                split_read_phase2_pending <= 1'b1;
            end else if (split_read_phase2_pending) begin
                split_read_phase2_pending <= 1'b0;
            end

            if (req_unaligned && !split_write_pending && !split_read_phase1_pending && !split_read_phase2_pending) begin
                split_word_addr <= word_addr;
                split_size      <= size;
                split_offset    <= addr_offset;
                split_wdata     <= wdata;

                if (we) begin
                    split_write_pending <= 1'b1;
                end else begin
                    split_read_phase1_pending <= 1'b1;
                end
            end
        end
    end
    
    // Ready logic:
    // - Writes: Ready immediately (same cycle as req && we)
    // - Reads: Ready one cycle after read request while request is still active
    assign ready =
        (req && we && !req_unaligned) ||      // aligned writes: single cycle
        (req && read_pending) ||              // aligned reads: one cycle latency
        (req && split_write_pending) ||       // split writes: second cycle
        (req && split_read_phase2_pending);   // split reads: second read cycle
    
    // Read data output - use extracted data
    assign rdata = split_read_phase2_pending ? split_extracted_rdata : extracted_rdata;

endmodule

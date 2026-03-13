`default_nettype none
// SRAM Peripheral
// 12KB memory-mapped SRAM peripheral with subword write masking
// Memory-mapped at 0x70000000 in RTL peripheral address space
//
// Size: 12KB (0x3000 bytes, 3072 words)
// Address Range: 0x70000000 - 0x70002FFF
//
// Features:
// - 12KB total memory (3072 x 32-bit words)
// - Subword write masking based on bus size and address alignment
// - Registered read output (2-cycle latency)

module sram_peripheral (
    // Clock and reset
    input wire logic        clk,
    input wire logic        rst_n,

    // Address channel
    input wire logic [31:0] mem_a_addr,
    input wire logic [31:0] mem_a_wdata,
    input wire logic        mem_a_we,
    input wire logic [1:0]  mem_a_size,
    input wire logic        mem_a_valid,
    output logic        mem_a_ready,

    // Data channel
    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input wire logic        mem_d_ready
);

    // ============================================================
    // SRAM Configuration
    // ============================================================
    // 12KB = 12288 bytes = 3072 words (32-bit)
    // Address width: 12 bits for word addressing (2^12 = 4096)
    localparam ADDR_WIDTH = 12;
    localparam DEPTH_WORDS = 3072;

    typedef enum logic [2:0] {
        S_IDLE,
        S_WRITE_RESP,
        S_WRITE_SPLIT_SECOND,
        S_READ_WAIT,
        S_READ_RESP,
        S_READ_SPLIT_SECOND,
        S_READ_SPLIT_CAPTURE,
        S_READ_SPLIT_RESP
    } state_t;

    // ============================================================
    // Internal Signals
    // ============================================================
    logic [ADDR_WIDTH-1:0] incoming_word_addr;
    logic [1:0]            incoming_offset;
    logic [31:0]           incoming_aligned_wdata;
    logic [3:0]            incoming_wmask;
    logic                  incoming_unaligned;

    logic [ADDR_WIDTH-1:0] req_word_addr;
    logic [1:0]            req_size;
    logic [1:0]            req_offset;
    logic [31:0]           req_wdata;
    logic [31:0]           split_first_rdata;

    logic [3:0]            split_second_wmask;
    logic [31:0]           split_second_wdata;

    logic [ADDR_WIDTH-1:0] sram_waddr;
    logic [ADDR_WIDTH-1:0] sram_raddr;
    logic [3:0]            sram_wmask;
    logic [31:0]           sram_wdata;
    logic                  sram_we;
    logic [31:0]           sram_rdata;

    logic [31:0]           shifted_rdata;
    logic [31:0]           extracted_rdata;
    logic [63:0]           split_concat_rdata;
    logic [63:0]           split_shifted_rdata;
    logic [31:0]           split_extracted_rdata;
    logic [31:0]           mem_d_rdata_r;
    logic                  mem_d_valid_r;

    logic                  mem_a_handshake;
    logic                  mem_d_handshake;

    state_t state;

    assign incoming_word_addr = mem_a_addr[ADDR_WIDTH+1:2];
    assign incoming_offset = mem_a_addr[1:0];
    assign incoming_unaligned =
        ((mem_a_size == 2'b01) && (incoming_offset == 2'b11)) ||
        ((mem_a_size == 2'b10) && (incoming_offset != 2'b00));

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;

    assign mem_a_ready = (state == S_IDLE);
    assign mem_d_rdata = mem_d_rdata_r;
    assign mem_d_valid = mem_d_valid_r;

    // ============================================================
    // Write Data Alignment
    // ============================================================
    always_comb begin
        incoming_aligned_wdata = 32'h0;

        case (mem_a_size)
            2'b00: begin  // Byte access
                case (incoming_offset)
                    2'b00: incoming_aligned_wdata = {24'h0, mem_a_wdata[7:0]};
                    2'b01: incoming_aligned_wdata = {16'h0, mem_a_wdata[7:0], 8'h0};
                    2'b10: incoming_aligned_wdata = {8'h0, mem_a_wdata[7:0], 16'h0};
                    2'b11: incoming_aligned_wdata = {mem_a_wdata[7:0], 24'h0};
                endcase
            end
            2'b01: incoming_aligned_wdata = mem_a_wdata << ({incoming_offset, 3'b000});
            2'b10: incoming_aligned_wdata = mem_a_wdata << ({incoming_offset, 3'b000});
            default: incoming_aligned_wdata = 32'h0;
        endcase
    end

    always_comb begin
        incoming_wmask = 4'b0000;

        if (mem_a_valid && mem_a_we) begin
            case (mem_a_size)
                2'b00: incoming_wmask = 4'b0001 << incoming_offset;
                2'b01: incoming_wmask = 4'b0011 << incoming_offset;
                2'b10: incoming_wmask = 4'b1111 << incoming_offset;
                default: incoming_wmask = 4'b0000;
            endcase
        end
    end

    always_comb begin
        split_second_wmask = 4'b0000;
        split_second_wdata = 32'h0;

        case (req_size)
            2'b01: begin
                split_second_wmask = 4'b0001;
                split_second_wdata = {24'h0, req_wdata[15:8]};
            end

            2'b10: begin
                case (req_offset)
                    2'b01: begin
                        split_second_wmask = 4'b0001;
                        split_second_wdata = {24'h0, req_wdata[31:24]};
                    end
                    2'b10: begin
                        split_second_wmask = 4'b0011;
                        split_second_wdata = {16'h0, req_wdata[31:16]};
                    end
                    2'b11: begin
                        split_second_wmask = 4'b0111;
                        split_second_wdata = {8'h0, req_wdata[31:8]};
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

    // ============================================================
    // SRAM Drive Signals
    // ============================================================
    always_comb begin
        sram_we = 1'b0;
        sram_waddr = req_word_addr;
        sram_wmask = 4'b0000;
        sram_wdata = 32'h0;
        sram_raddr = req_word_addr;

        case (state)
            S_IDLE: begin
                sram_raddr = incoming_word_addr;

                if (mem_a_handshake) begin
                    if (mem_a_we) begin
                        sram_we = 1'b1;
                        sram_waddr = incoming_word_addr;
                        sram_wmask = incoming_wmask;
                        sram_wdata = incoming_aligned_wdata;
                    end
                end
            end

            S_WRITE_SPLIT_SECOND: begin
                sram_we = 1'b1;
                sram_waddr = req_word_addr + 1'b1;
                sram_wmask = split_second_wmask;
                sram_wdata = split_second_wdata;
            end

            S_READ_SPLIT_SECOND: begin
                sram_raddr = req_word_addr + 1'b1;
            end

            S_READ_SPLIT_CAPTURE: begin
                sram_raddr = req_word_addr + 1'b1;
            end

            S_READ_SPLIT_RESP: begin
                sram_raddr = req_word_addr + 1'b1;
            end

            default: begin
                sram_raddr = req_word_addr;
            end
        endcase
    end

    // ============================================================
    // SRAM Instantiation
    // ============================================================
    sram #(
        .ADDR_WIDTH(ADDR_WIDTH),
        .DEPTH(DEPTH_WORDS)
    ) sram_inst (
        .clk(clk),
        .we(sram_we),
        .wmask(sram_wmask),
        .waddr(sram_waddr),
        .wdata(sram_wdata),
        .raddr(sram_raddr),
        .rdata(sram_rdata)
    );

    // ============================================================
    // Read Data Extraction
    // ============================================================
    assign shifted_rdata = sram_rdata >> ({req_offset, 3'b000});
    assign split_concat_rdata = {sram_rdata, split_first_rdata};
    assign split_shifted_rdata = split_concat_rdata >> ({req_offset, 3'b000});

    always_comb begin
        extracted_rdata = 32'h0;

        case (req_size)
            2'b00: begin
                case (req_offset)
                    2'b00: extracted_rdata = {24'h0, sram_rdata[7:0]};
                    2'b01: extracted_rdata = {24'h0, sram_rdata[15:8]};
                    2'b10: extracted_rdata = {24'h0, sram_rdata[23:16]};
                    2'b11: extracted_rdata = {24'h0, sram_rdata[31:24]};
                endcase
            end
            2'b01: extracted_rdata = {16'h0, shifted_rdata[15:0]};
            2'b10: extracted_rdata = sram_rdata;
            default: extracted_rdata = 32'h0;
        endcase
    end

    always_comb begin
        split_extracted_rdata = 32'h0;

        case (req_size)
            2'b01: split_extracted_rdata = {16'h0, split_shifted_rdata[15:0]};
            2'b10: split_extracted_rdata = split_shifted_rdata[31:0];
            default: split_extracted_rdata = 32'h0;
        endcase
    end

    // ============================================================
    // State / Request Tracking / Registered D Channel
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            state <= S_IDLE;
            mem_d_valid_r <= 1'b0;
        end else begin
            case (state)
                S_WRITE_SPLIT_SECOND: begin
                    state <= S_WRITE_RESP;
                    mem_d_rdata_r <= 32'h0;
                    mem_d_valid_r <= 1'b1;
                end

                S_READ_WAIT: begin
                    state <= S_READ_RESP;
                end

                S_READ_SPLIT_SECOND: begin
                    state <= S_READ_SPLIT_CAPTURE;
                end

                S_READ_SPLIT_CAPTURE: begin
                    split_first_rdata <= sram_rdata;
                    state <= S_READ_SPLIT_RESP;
                end

                S_WRITE_RESP: begin
                    if (mem_d_handshake) begin
                        mem_d_valid_r <= 1'b0;
                        state <= S_IDLE;
                    end
                end

                S_READ_RESP: begin
                    // Latch the SRAM read result into the registered D-channel
                    // outputs, then hold it stable until the response is accepted.
                    if (!mem_d_valid_r) begin
                        mem_d_rdata_r <= extracted_rdata;
                        mem_d_valid_r <= 1'b1;
                    end else if (mem_d_handshake) begin
                        mem_d_valid_r <= 1'b0;
                        state <= S_IDLE;
                    end
                end

                S_READ_SPLIT_RESP: begin
                    if (!mem_d_valid_r) begin
                        mem_d_rdata_r <= split_extracted_rdata;
                        mem_d_valid_r <= 1'b1;
                    end else if (mem_d_handshake) begin
                        mem_d_valid_r <= 1'b0;
                        state <= S_IDLE;
                    end
                end

                S_IDLE: begin
                    if (mem_a_handshake) begin
                        req_word_addr <= incoming_word_addr;
                        req_size <= mem_a_size;
                        req_offset <= incoming_offset;
                        req_wdata <= mem_a_wdata;

                        if (mem_a_we) begin
                            if (incoming_unaligned) begin
                                state <= S_WRITE_SPLIT_SECOND;
                            end else begin
                                state <= S_WRITE_RESP;
                                mem_d_rdata_r <= 32'h0;
                                mem_d_valid_r <= 1'b1;
                            end
                        end else if (incoming_unaligned) begin
                            state <= S_READ_SPLIT_SECOND;
                        end else begin
                            state <= S_READ_WAIT;
                        end
                    end
                end

                default: begin
                    state <= S_IDLE;
                end
            endcase
        end
    end

endmodule
`default_nettype wire

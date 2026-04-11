`default_nettype none

module sdram_peripheral #(
    parameter int unsigned BASE_ADDR = 32'h1000_0000,
    parameter int unsigned ADDR_SIZE = 32'h0400_0000,
    parameter int unsigned WORD_ADDR_WIDTH = 24,
    parameter int unsigned BUS_CDC_SYNC_STAGES = 3
) (
    input  wire logic                         sys_clk,
    input  wire logic                         sdram_clk,
    input  wire logic                         rst,
    input  wire logic [31:0]                  mem_a_addr,
    input  wire logic [31:0]                  mem_a_wdata,
    input  wire logic                         mem_a_we,
    input  wire logic [1:0]                   mem_a_size,
    input  wire logic                         mem_a_valid,
    output logic                              mem_a_ready,
    output logic [31:0]                       mem_d_rdata,
    output logic                              mem_d_valid,
    input  wire logic                         mem_d_ready,
    output logic                              word_rd,
    output logic                              word_wr,
    output logic [WORD_ADDR_WIDTH-1:0]        word_addr,
    output logic [31:0]                       word_data,
    input  wire logic [31:0]                  word_q,
    input  wire logic                         word_busy
);

    typedef enum logic [3:0] {
        S_IDLE,
        S_READ_CMD,
        S_READ_WAIT_BUSY,
        S_READ_WAIT_DONE,
        S_READ_PROCESS,
        S_WRITE_CMD,
        S_WRITE_WAIT_BUSY,
        S_WRITE_WAIT_DONE,
        S_RESPOND
    } state_t;

    // registered_bus only decodes addr[31:28], so one slave window can span at most 256 MiB.
    localparam logic [31:0] DECODE_LIMIT_BYTES = 32'h1000_0000;
    localparam longint unsigned INTERFACE_LIMIT_BYTES_WIDE = (64'd1 << (WORD_ADDR_WIDTH + 2));
    localparam logic [31:0] INTERFACE_LIMIT_BYTES = INTERFACE_LIMIT_BYTES_WIDE[31:0];
    localparam logic [31:0] LAST_ADDR = BASE_ADDR + ADDR_SIZE - 32'd1;

    logic reset_n_sdram_sync;
    logic sdram_rst;

    logic [31:0] periph_mem_a_addr;
    logic [31:0] periph_mem_a_wdata;
    logic        periph_mem_a_we;
    logic [1:0]  periph_mem_a_size;
    logic        periph_mem_a_valid;
    logic        periph_mem_a_ready;
    logic [31:0] periph_mem_d_rdata;
    logic        periph_mem_d_valid;
    logic        periph_mem_d_ready;

    logic        periph_mem_a_handshake;
    logic        periph_mem_d_handshake;

    state_t      state;

    logic [31:0] req_addr_reg;
    logic [31:0] req_wdata_reg;
    logic        req_we_reg;
    logic [1:0]  req_size_reg;
    logic        req_split_reg;
    logic [WORD_ADDR_WIDTH-1:0] req_word_addr_reg;

    logic        read_second_word_pending_reg;
    logic [31:0] read_word0_reg;
    logic [31:0] read_word1_reg;

    logic [31:0] write_word0_reg;
    logic [31:0] write_word1_reg;
    logic        write_second_word_pending_reg;
    logic        active_write_word_is_second_reg;

    logic [31:0] response_data;
    logic        response_pending;

    logic        incoming_addr_ge_base;
    logic [31:0] incoming_byte_offset;
    logic [31:0] incoming_word0_byte_offset;
    logic [2:0]  incoming_access_bytes;
    logic        incoming_split;
    logic [32:0] incoming_last_offset_ext;
    logic        incoming_in_range;
    logic [WORD_ADDR_WIDTH-1:0] incoming_word_addr;

    logic [1:0]  req_offset;
    logic [63:0] req_read_concat;
    logic [31:0] req_extracted_rdata;
    logic [63:0] req_merged_write_concat;
    logic [WORD_ADDR_WIDTH-1:0] active_read_word_addr;
    logic [WORD_ADDR_WIDTH-1:0] active_write_word_addr;
    logic [31:0] active_write_word_data;

    function automatic logic [2:0] access_byte_count(input logic [1:0] size);
        case (size)
            2'b00: access_byte_count = 3'd1;
            2'b01: access_byte_count = 3'd2;
            2'b10: access_byte_count = 3'd4;
            default: access_byte_count = 3'd0;
        endcase
    endfunction

    function automatic logic [7:0] access_byte_mask(
        input logic [1:0] size,
        input logic [1:0] offset
    );
        logic [7:0] byte_mask;
        logic [2:0] byte_count;
        logic [3:0] offset_limit;
        begin
            byte_mask = 8'h00;
            byte_count = access_byte_count(size);
            offset_limit = {2'b00, offset} + {1'b0, byte_count};

            for (int unsigned byte_idx = 0; byte_idx < 8; byte_idx++) begin
                if ((byte_idx >= {2'b00, offset}) && (byte_idx < offset_limit)) begin
                    byte_mask[byte_idx] = 1'b1;
                end
            end

            access_byte_mask = byte_mask;
        end
    endfunction

    function automatic logic [63:0] expand_byte_mask(input logic [7:0] byte_mask);
        logic [63:0] expanded_mask;
        begin
            expanded_mask = '0;

            for (int unsigned byte_idx = 0; byte_idx < 8; byte_idx++) begin
                expanded_mask[(byte_idx*8) +: 8] = {8{byte_mask[byte_idx]}};
            end

            expand_byte_mask = expanded_mask;
        end
    endfunction

    function automatic logic [31:0] extract_access_data(
        input logic [63:0] read_data,
        input logic [1:0]  size,
        input logic [1:0]  offset
    );
        logic [63:0] shifted_data;
        begin
            shifted_data = read_data >> {offset, 3'b000};

            case (size)
                2'b00: extract_access_data = {24'h000000, shifted_data[7:0]};
                2'b01: extract_access_data = {16'h0000, shifted_data[15:0]};
                2'b10: extract_access_data = shifted_data[31:0];
                default: extract_access_data = 32'h0000_0000;
            endcase
        end
    endfunction

    function automatic logic [63:0] merge_access_data(
        input logic [63:0] current_data,
        input logic [31:0] write_data,
        input logic [1:0]  size,
        input logic [1:0]  offset
    );
        logic [63:0] shifted_write_data;
        logic [63:0] bit_mask;
        begin
            shifted_write_data = {{32{1'b0}}, write_data} << {offset, 3'b000};
            bit_mask = expand_byte_mask(access_byte_mask(size, offset));
            merge_access_data = (current_data & ~bit_mask) | (shifted_write_data & bit_mask);
        end
    endfunction

    assign periph_mem_a_handshake = periph_mem_a_valid && periph_mem_a_ready;
    assign periph_mem_d_handshake = periph_mem_d_valid && periph_mem_d_ready;

    assign incoming_addr_ge_base = periph_mem_a_addr >= BASE_ADDR;
    assign incoming_byte_offset = periph_mem_a_addr - BASE_ADDR;
    assign incoming_word0_byte_offset = {incoming_byte_offset[31:2], 2'b00};
    assign incoming_access_bytes = access_byte_count(periph_mem_a_size);
    assign incoming_split =
        ({1'b0, periph_mem_a_addr[1:0]} + {1'b0, incoming_access_bytes}) > 4'd4;
    assign incoming_last_offset_ext =
        {1'b0, incoming_byte_offset} + {30'd0, incoming_access_bytes} - 33'd1;
    assign incoming_in_range =
        (incoming_access_bytes != 3'd0)
        && incoming_addr_ge_base
        && (incoming_last_offset_ext < {1'b0, ADDR_SIZE});
    assign incoming_word_addr = incoming_word0_byte_offset[WORD_ADDR_WIDTH+1:2];

    assign req_offset = req_addr_reg[1:0];
    assign req_read_concat = {read_word1_reg, read_word0_reg};
    assign req_extracted_rdata = extract_access_data(req_read_concat, req_size_reg, req_offset);
    assign req_merged_write_concat =
        merge_access_data(req_read_concat, req_wdata_reg, req_size_reg, req_offset);
    assign active_read_word_addr =
        req_word_addr_reg + WORD_ADDR_WIDTH'(read_second_word_pending_reg ? 1'b1 : 1'b0);
    assign active_write_word_addr =
        req_word_addr_reg + WORD_ADDR_WIDTH'(active_write_word_is_second_reg ? 1'b1 : 1'b0);
    assign active_write_word_data = active_write_word_is_second_reg ? write_word1_reg : write_word0_reg;

    assign periph_mem_a_ready = !sdram_rst && (state == S_IDLE) && !response_pending;
    assign periph_mem_d_rdata = response_data;
    assign periph_mem_d_valid = response_pending;

    ff_sync #(
        .STAGES(BUS_CDC_SYNC_STAGES),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) sdram_reset_sync (
        .clk(sdram_clk),
        .rst(1'b0),
        .din(!rst),
        .dout(reset_n_sdram_sync)
    );

    assign sdram_rst = !reset_n_sdram_sync;

    bus_cdc_bridge #(
        .ADDR_WIDTH(32),
        .DATA_WIDTH(32),
        .SIZE_WIDTH(2),
        .SYNC_STAGES(BUS_CDC_SYNC_STAGES)
    ) u_bus_cdc_bridge (
        .sys_clk(sys_clk),
        .periph_clk(sdram_clk),
        .sys_rst(rst),
        .periph_rst(sdram_rst),
        .sys_mem_a_addr(mem_a_addr),
        .sys_mem_a_wdata(mem_a_wdata),
        .sys_mem_a_we(mem_a_we),
        .sys_mem_a_size(mem_a_size),
        .sys_mem_a_valid(mem_a_valid),
        .sys_mem_a_ready(mem_a_ready),
        .sys_mem_d_rdata(mem_d_rdata),
        .sys_mem_d_valid(mem_d_valid),
        .sys_mem_d_ready(mem_d_ready),
        .periph_mem_a_addr(periph_mem_a_addr),
        .periph_mem_a_wdata(periph_mem_a_wdata),
        .periph_mem_a_we(periph_mem_a_we),
        .periph_mem_a_size(periph_mem_a_size),
        .periph_mem_a_valid(periph_mem_a_valid),
        .periph_mem_a_ready(periph_mem_a_ready),
        .periph_mem_d_rdata(periph_mem_d_rdata),
        .periph_mem_d_valid(periph_mem_d_valid),
        .periph_mem_d_ready(periph_mem_d_ready)
    );

    initial begin
        if (BASE_ADDR[1:0] != 2'b00) begin
            $fatal(1, "sdram_peripheral: BASE_ADDR must be 32-bit aligned, got 0x%08h", BASE_ADDR);
        end
        if (ADDR_SIZE == 32'h0000_0000) begin
            $fatal(1, "sdram_peripheral: ADDR_SIZE must be non-zero");
        end
        if (ADDR_SIZE[1:0] != 2'b00) begin
            $fatal(1, "sdram_peripheral: ADDR_SIZE must be 32-bit aligned, got 0x%08h", ADDR_SIZE);
        end
        if (ADDR_SIZE > DECODE_LIMIT_BYTES) begin
            $fatal(1, "sdram_peripheral: ADDR_SIZE exceeds registered_bus decode limit: 0x%08h", ADDR_SIZE);
        end
        if (ADDR_SIZE > INTERFACE_LIMIT_BYTES) begin
            $fatal(1, "sdram_peripheral: ADDR_SIZE exceeds word interface capacity: 0x%08h", ADDR_SIZE);
        end
        if (BASE_ADDR[31:28] != LAST_ADDR[31:28]) begin
            $fatal(1, "sdram_peripheral: address window crosses registered_bus decode nibble");
        end
    end

    always_ff @(posedge sdram_clk) begin
        if (sdram_rst) begin
            state <= S_IDLE;
            response_pending <= 1'b0;
            word_rd <= 1'b0;
            word_wr <= 1'b0;
            read_second_word_pending_reg <= 1'b0;
            write_second_word_pending_reg <= 1'b0;
            active_write_word_is_second_reg <= 1'b0;
        end else begin
            word_rd <= 1'b0;
            word_wr <= 1'b0;

            if (periph_mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            case (state)
                S_IDLE: begin
                    if (periph_mem_a_handshake) begin
                        req_addr_reg <= periph_mem_a_addr;
                        req_wdata_reg <= periph_mem_a_wdata;
                        req_we_reg <= periph_mem_a_we;
                        req_size_reg <= periph_mem_a_size;
                        req_split_reg <= incoming_split;
                        req_word_addr_reg <= incoming_word_addr;
                        read_second_word_pending_reg <= 1'b0;
                        write_second_word_pending_reg <= 1'b0;
                        active_write_word_is_second_reg <= 1'b0;

                        if (!incoming_in_range) begin
                            response_data <= 32'h0000_0000;
                            response_pending <= 1'b1;
                            state <= S_RESPOND;
                        end else if (periph_mem_a_we
                                     && (periph_mem_a_size == 2'b10)
                                     && (periph_mem_a_addr[1:0] == 2'b00)) begin
                            write_word0_reg <= periph_mem_a_wdata;
                            write_word1_reg <= 32'h0000_0000;
                            state <= S_WRITE_CMD;
                        end else begin
                            state <= S_READ_CMD;
                        end
                    end
                end

                S_READ_CMD: begin
                    if (!word_busy) begin
                        word_rd <= 1'b1;
                        word_addr <= active_read_word_addr;
                        state <= S_READ_WAIT_BUSY;
                    end
                end

                S_READ_WAIT_BUSY: begin
                    if (word_busy) begin
                        state <= S_READ_WAIT_DONE;
                    end
                end

                S_READ_WAIT_DONE: begin
                    if (!word_busy) begin
                        if (!read_second_word_pending_reg) begin
                            read_word0_reg <= word_q;
                            if (req_split_reg) begin
                                read_second_word_pending_reg <= 1'b1;
                                state <= S_READ_CMD;
                            end else begin
                                read_word1_reg <= 32'h0000_0000;
                                state <= S_READ_PROCESS;
                            end
                        end else begin
                            read_word1_reg <= word_q;
                            state <= S_READ_PROCESS;
                        end
                    end
                end

                S_READ_PROCESS: begin
                    if (req_we_reg) begin
                        write_word0_reg <= req_merged_write_concat[31:0];
                        write_word1_reg <= req_merged_write_concat[63:32];
                        write_second_word_pending_reg <= req_split_reg;
                        active_write_word_is_second_reg <= 1'b0;
                        state <= S_WRITE_CMD;
                    end else begin
                        response_data <= req_extracted_rdata;
                        response_pending <= 1'b1;
                        state <= S_RESPOND;
                    end
                end

                S_WRITE_CMD: begin
                    if (!word_busy) begin
                        word_wr <= 1'b1;
                        word_addr <= active_write_word_addr;
                        word_data <= active_write_word_data;
                        state <= S_WRITE_WAIT_BUSY;
                    end
                end

                S_WRITE_WAIT_BUSY: begin
                    if (word_busy) begin
                        state <= S_WRITE_WAIT_DONE;
                    end
                end

                S_WRITE_WAIT_DONE: begin
                    if (!word_busy) begin
                        if (!active_write_word_is_second_reg && write_second_word_pending_reg) begin
                            active_write_word_is_second_reg <= 1'b1;
                            state <= S_WRITE_CMD;
                        end else begin
                            response_data <= 32'h0000_0000;
                            response_pending <= 1'b1;
                            state <= S_RESPOND;
                        end
                    end
                end

                S_RESPOND: begin
                    if (periph_mem_d_handshake) begin
                        state <= S_IDLE;
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

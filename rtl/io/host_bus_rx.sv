// Host RX Buffer Module
// Parses burst-native 8-byte metadata framing from host RX byte stream.
// Emits decoded packets as a beat stream with ready/valid handshaking.

module host_bus_rx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,

    // RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,

    // Decoded Packet Beat Stream
    output logic        packet_valid,
    output logic        packet_start,
    output logic        packet_last,
    output logic        packet_req,
    output logic        packet_we,
    output logic [1:0]  packet_size,
    output logic        packet_src_fixed,
    output logic        packet_dst_fixed,
    output logic [15:0] packet_burst_len_m1,
    output logic [31:0] packet_base_addr,
    output logic [31:0] packet_data,
    input  logic        packet_ready
);

    typedef enum logic [3:0] {
        STATE_IDLE      = 4'd0,
        STATE_CTRL1     = 4'd1,
        STATE_LEN_0     = 4'd2,
        STATE_LEN_1     = 4'd3,
        STATE_ADDR_0    = 4'd4,
        STATE_ADDR_1    = 4'd5,
        STATE_ADDR_2    = 4'd6,
        STATE_ADDR_3    = 4'd7,
        STATE_PAYLOAD   = 4'd8
    } state_t;

    state_t state;

    logic [3:0]  packet_type_reg;
    logic        packet_we_reg;
    logic [1:0]  packet_size_reg;
    logic        packet_src_fixed_reg;
    logic        packet_dst_fixed_reg;
    logic [15:0] packet_burst_len_m1_reg;
    logic [31:0] packet_base_addr_reg;

    logic [2:0]  beat_bytes_reg;
    logic [16:0] burst_len_reg;
    logic [16:0] beats_remaining_reg;
    logic [1:0]  beat_byte_idx_reg;
    logic [31:0] beat_accum_reg;

    logic        out_valid_reg;
    logic        out_start_reg;
    logic        out_last_reg;
    logic [31:0] out_data_reg;

    logic [31:0] beat_word_with_byte;
    logic [2:0]  beat_bytes_local;
    logic [16:0] burst_len_local;
    logic        beat_done;
    logic        is_last_beat;

    logic payload_enabled;

    assign payload_enabled = ((packet_type_reg == 4'b0010) && packet_we_reg)
                          || ((packet_type_reg == 4'b0001) && !packet_we_reg);

    always_comb begin
        beat_word_with_byte = beat_accum_reg;
        case (beat_byte_idx_reg)
            2'd0: beat_word_with_byte[7:0]   = rx_data;
            2'd1: beat_word_with_byte[15:8]  = rx_data;
            2'd2: beat_word_with_byte[23:16] = rx_data;
            default: beat_word_with_byte[31:24] = rx_data;
        endcase
    end

    assign packet_valid        = out_valid_reg;
    assign packet_start        = out_start_reg;
    assign packet_last         = out_last_reg;
    assign packet_req          = (packet_type_reg == 4'b0010);
    assign packet_we           = packet_we_reg;
    assign packet_size         = packet_size_reg;
    assign packet_src_fixed    = packet_src_fixed_reg;
    assign packet_dst_fixed    = packet_dst_fixed_reg;
    assign packet_burst_len_m1 = packet_burst_len_m1_reg;
    assign packet_base_addr    = packet_base_addr_reg;
    assign packet_data         = out_data_reg;

    // Single-beat output buffering: stop byte intake while waiting packet_ready.
    assign rx_ready = !out_valid_reg;

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;

            packet_type_reg         <= 4'h0;
            packet_we_reg           <= 1'b0;
            packet_size_reg         <= 2'b00;
            packet_src_fixed_reg    <= 1'b0;
            packet_dst_fixed_reg    <= 1'b0;
            packet_burst_len_m1_reg <= 16'h0000;
            packet_base_addr_reg    <= 32'h0000_0000;

            beat_bytes_reg      <= 3'd1;
            burst_len_reg       <= 17'd0;
            beats_remaining_reg <= 17'd0;
            beat_byte_idx_reg   <= 2'd0;
            beat_accum_reg      <= 32'h0000_0000;

            out_valid_reg <= 1'b0;
            out_start_reg <= 1'b0;
            out_last_reg  <= 1'b0;
            out_data_reg  <= 32'h0000_0000;
        end else begin
            if (out_valid_reg && packet_ready) begin
                out_valid_reg <= 1'b0;
            end

            if (!out_valid_reg && rx_valid && rx_ready) begin
                case (state)
                    STATE_IDLE: begin
                        packet_type_reg      <= rx_data[7:4];
                        packet_size_reg      <= rx_data[3:2];
                        packet_src_fixed_reg <= rx_data[1];
                        packet_dst_fixed_reg <= rx_data[0];

                        // Accept only host->FPGA packet types: response(0001), request(0010)
                        if ((rx_data[7:4] == 4'b0001 || rx_data[7:4] == 4'b0010)
                            && (rx_data[3:2] != 2'b11)) begin
                            state <= STATE_CTRL1;
                        end else begin
                            // Malformed header: flush and resync at next byte.
                            state <= STATE_IDLE;
                        end
                    end

                    STATE_CTRL1: begin
                        // CTRL1[7:1] reserved must be zero.
                        if (rx_data[7:1] != 7'b0) begin
                            state <= STATE_IDLE;
                        end else begin
                            packet_we_reg <= rx_data[0];
                            state <= STATE_LEN_0;
                        end
                    end

                    STATE_LEN_0: begin
                        packet_burst_len_m1_reg[7:0] <= rx_data;
                        state <= STATE_LEN_1;
                    end

                    STATE_LEN_1: begin
                        packet_burst_len_m1_reg[15:8] <= rx_data;
                        state <= STATE_ADDR_0;
                    end

                    STATE_ADDR_0: begin
                        packet_base_addr_reg[7:0] <= rx_data;
                        state <= STATE_ADDR_1;
                    end

                    STATE_ADDR_1: begin
                        packet_base_addr_reg[15:8] <= rx_data;
                        state <= STATE_ADDR_2;
                    end

                    STATE_ADDR_2: begin
                        packet_base_addr_reg[23:16] <= rx_data;
                        state <= STATE_ADDR_3;
                    end

                    STATE_ADDR_3: begin
                        packet_base_addr_reg[31:24] <= rx_data;

                        case (packet_size_reg)
                            2'b00: beat_bytes_local = 3'd1;
                            2'b01: beat_bytes_local = 3'd2;
                            default: beat_bytes_local = 3'd4;
                        endcase

                        burst_len_local = {1'b0, packet_burst_len_m1_reg} + 17'd1;
                        beat_bytes_reg      <= beat_bytes_local;
                        burst_len_reg       <= burst_len_local;
                        beats_remaining_reg <= burst_len_local;
                        beat_byte_idx_reg   <= 2'd0;
                        beat_accum_reg      <= 32'h0000_0000;

                        if (!payload_enabled) begin
                            // Metadata-only packet (read request or write response)
                            out_valid_reg <= 1'b1;
                            out_start_reg <= 1'b1;
                            out_last_reg  <= 1'b1;
                            out_data_reg  <= 32'h0000_0000;
                            state <= STATE_IDLE;
                        end else begin
                            state <= STATE_PAYLOAD;
                        end
                    end

                    STATE_PAYLOAD: begin
                        beat_accum_reg <= beat_word_with_byte;
                        beat_done = ({1'b0, beat_byte_idx_reg} == (beat_bytes_reg - 3'd1));

                        if (beat_done) begin
                            is_last_beat = (beats_remaining_reg == 17'd1);

                            out_valid_reg <= 1'b1;
                            out_start_reg <= (beats_remaining_reg == burst_len_reg);
                            out_last_reg  <= is_last_beat;
                            out_data_reg  <= beat_word_with_byte;

`ifdef ASSERT_ON
                            assert (beats_remaining_reg != 17'd0)
                                else $error("host_bus_rx: beats_remaining_reg underflow");
`endif
                            beats_remaining_reg <= beats_remaining_reg - 17'd1;
                            beat_byte_idx_reg <= 2'd0;
                            beat_accum_reg    <= 32'h0000_0000;

                            if (is_last_beat) begin
                                state <= STATE_IDLE;
                            end
                        end else begin
                            beat_byte_idx_reg <= beat_byte_idx_reg + 2'd1;
                        end
                    end

                    default: begin
                        state <= STATE_IDLE;
                    end
                endcase
            end
        end
    end

endmodule

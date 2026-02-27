// Host TX Buffer Module
// Serializes burst-native metadata framing and payload beat stream to host TX byte stream.

module host_bus_tx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,

    // TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,

    // Packet Beat Stream Input
    input  logic        packet_valid,
    output logic        packet_ready,
    input  logic        packet_start,
    input  logic        packet_last,
    input  logic        packet_req,
    input  logic        packet_we,
    input  logic [1:0]  packet_size,
    input  logic        packet_src_fixed,
    input  logic        packet_dst_fixed,
    input  logic [15:0] packet_burst_len_m1,
    input  logic [31:0] packet_base_addr,
    input  logic [31:0] packet_data
);

    typedef enum logic [3:0] {
        STATE_IDLE      = 4'd0,
        STATE_TX_CTRL0  = 4'd1,
        STATE_TX_CTRL1  = 4'd2,
        STATE_TX_LEN_0  = 4'd3,
        STATE_TX_LEN_1  = 4'd4,
        STATE_TX_ADDR_0 = 4'd5,
        STATE_TX_ADDR_1 = 4'd6,
        STATE_TX_ADDR_2 = 4'd7,
        STATE_TX_ADDR_3 = 4'd8,
        STATE_WAIT_BEAT = 4'd9,
        STATE_TX_DATA_0 = 4'd10,
        STATE_TX_DATA_1 = 4'd11,
        STATE_TX_DATA_2 = 4'd12,
        STATE_TX_DATA_3 = 4'd13
    } state_t;

    state_t state, next_state;

    logic [3:0]  packet_type_reg;
    logic        packet_we_reg;
    logic [1:0]  packet_size_reg;
    logic        packet_src_fixed_reg;
    logic        packet_dst_fixed_reg;
    logic [15:0] packet_burst_len_m1_reg;
    logic [31:0] packet_base_addr_reg;

    logic        payload_enabled_reg;
    logic [2:0]  beat_bytes_reg;
    logic [16:0] beats_remaining_reg;

    logic [31:0] beat_data_reg;

    logic        tx_handshake;
    logic [7:0]  tx_byte;

    assign tx_handshake = tx_valid && tx_ready;

    // Packet beat acceptance:
    // - IDLE: accept first beat/metadata (packet_start must be 1)
    // - WAIT_BEAT: accept subsequent payload beats
    assign packet_ready = (state == STATE_IDLE      && packet_start) ||
                          (state == STATE_WAIT_BEAT && !packet_start);

    assign tx_valid = (state != STATE_IDLE) && (state != STATE_WAIT_BEAT);

    always_comb begin
        next_state = state;

        case (state)
            STATE_IDLE: begin
                if (packet_valid && packet_ready && packet_start) begin
                    next_state = STATE_TX_CTRL0;
                end
            end

            STATE_TX_CTRL0:  if (tx_handshake) next_state = STATE_TX_CTRL1;
            STATE_TX_CTRL1:  if (tx_handshake) next_state = STATE_TX_LEN_0;
            STATE_TX_LEN_0:  if (tx_handshake) next_state = STATE_TX_LEN_1;
            STATE_TX_LEN_1:  if (tx_handshake) next_state = STATE_TX_ADDR_0;
            STATE_TX_ADDR_0: if (tx_handshake) next_state = STATE_TX_ADDR_1;
            STATE_TX_ADDR_1: if (tx_handshake) next_state = STATE_TX_ADDR_2;
            STATE_TX_ADDR_2: if (tx_handshake) next_state = STATE_TX_ADDR_3;

            STATE_TX_ADDR_3: begin
                if (tx_handshake) begin
                    if (!payload_enabled_reg) begin
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_TX_DATA_0;
                    end
                end
            end

            STATE_WAIT_BEAT: begin
                if (packet_valid && packet_ready && !packet_start) begin
                    next_state = STATE_TX_DATA_0;
                end
            end

            STATE_TX_DATA_0: begin
                if (tx_handshake) begin
                    if (beat_bytes_reg == 3'd1) begin
                        if (beats_remaining_reg == 17'd1) begin
                            next_state = STATE_IDLE;
                        end else begin
                            next_state = STATE_WAIT_BEAT;
                        end
                    end else begin
                        next_state = STATE_TX_DATA_1;
                    end
                end
            end

            STATE_TX_DATA_1: begin
                if (tx_handshake) begin
                    if (beat_bytes_reg == 3'd2) begin
                        if (beats_remaining_reg == 17'd1) begin
                            next_state = STATE_IDLE;
                        end else begin
                            next_state = STATE_WAIT_BEAT;
                        end
                    end else begin
                        next_state = STATE_TX_DATA_2;
                    end
                end
            end

            STATE_TX_DATA_2: if (tx_handshake) next_state = STATE_TX_DATA_3;

            STATE_TX_DATA_3: begin
                if (tx_handshake) begin
                    if (beats_remaining_reg == 17'd1) begin
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_WAIT_BEAT;
                    end
                end
            end

            default: next_state = STATE_IDLE;
        endcase
    end

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

            payload_enabled_reg <= 1'b0;
            beat_bytes_reg      <= 3'd1;
            beats_remaining_reg <= 17'd0;
            beat_data_reg       <= 32'h0000_0000;
        end else begin
            state <= next_state;

            if (state == STATE_IDLE && packet_valid && packet_ready && packet_start) begin
                packet_type_reg         <= packet_req ? 4'b0000 : 4'b0011;
                packet_we_reg           <= packet_we;
                packet_size_reg         <= packet_size;
                packet_src_fixed_reg    <= packet_src_fixed;
                packet_dst_fixed_reg    <= packet_dst_fixed;
                packet_burst_len_m1_reg <= packet_burst_len_m1;
                packet_base_addr_reg    <= packet_base_addr;

                payload_enabled_reg <= (packet_req && packet_we) || (!packet_req && !packet_we);

                case (packet_size)
                    2'b00: beat_bytes_reg <= 3'd1;
                    2'b01: beat_bytes_reg <= 3'd2;
                    default: beat_bytes_reg <= 3'd4;
                endcase

                beats_remaining_reg <= {1'b0, packet_burst_len_m1} + 17'd1;
                beat_data_reg       <= packet_data;
            end

            if (state == STATE_WAIT_BEAT && packet_valid && packet_ready && !packet_start) begin
                beat_data_reg <= packet_data;
            end

            // Beat counters decrement after fully transmitting one beat.
            if (tx_handshake) begin
                case (state)
                    STATE_TX_DATA_0: begin
                        if (beat_bytes_reg == 3'd1 && beats_remaining_reg != 17'd0) begin
                            beats_remaining_reg <= beats_remaining_reg - 17'd1;
                        end
                    end
                    STATE_TX_DATA_1: begin
                        if (beat_bytes_reg == 3'd2 && beats_remaining_reg != 17'd0) begin
                            beats_remaining_reg <= beats_remaining_reg - 17'd1;
                        end
                    end
                    STATE_TX_DATA_3: begin
                        if (beats_remaining_reg != 17'd0) begin
                            beats_remaining_reg <= beats_remaining_reg - 17'd1;
                        end
                    end
                    default: ;
                endcase
            end

        end
    end

`ifdef ASSERT_ON
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            // no-op
        end else begin
            if (state == STATE_IDLE && packet_valid && packet_ready && packet_start &&
                ((packet_req && packet_we) || (!packet_req && !packet_we))) begin
                assert (packet_last == (packet_burst_len_m1 == 16'h0000))
                    else $error("host_bus_tx: packet_last does not match burst_len on first beat");
            end
            if (state == STATE_WAIT_BEAT && packet_valid && packet_ready && !packet_start) begin
                assert (packet_last == (beats_remaining_reg == 17'd1))
                    else $error("host_bus_tx: packet_last does not match beats_remaining_reg");
            end
        end
    end
`endif

    always_comb begin
        tx_byte = 8'h00;

        case (state)
            STATE_TX_CTRL0: tx_byte = {packet_type_reg, packet_size_reg, packet_src_fixed_reg, packet_dst_fixed_reg};
            STATE_TX_CTRL1: tx_byte = {7'b0000000, packet_we_reg};
            STATE_TX_LEN_0: tx_byte = packet_burst_len_m1_reg[7:0];
            STATE_TX_LEN_1: tx_byte = packet_burst_len_m1_reg[15:8];
            STATE_TX_ADDR_0: tx_byte = packet_base_addr_reg[7:0];
            STATE_TX_ADDR_1: tx_byte = packet_base_addr_reg[15:8];
            STATE_TX_ADDR_2: tx_byte = packet_base_addr_reg[23:16];
            STATE_TX_ADDR_3: tx_byte = packet_base_addr_reg[31:24];
            STATE_TX_DATA_0: tx_byte = beat_data_reg[7:0];
            STATE_TX_DATA_1: tx_byte = beat_data_reg[15:8];
            STATE_TX_DATA_2: tx_byte = beat_data_reg[23:16];
            STATE_TX_DATA_3: tx_byte = beat_data_reg[31:24];
            default: ;
        endcase
    end

    assign tx_data = tx_byte;

endmodule

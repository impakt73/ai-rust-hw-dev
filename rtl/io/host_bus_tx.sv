// Host TX Buffer Module
// Buffers one outgoing host packet at a time using a unified packet interface
//
// Features:
//   - Single unified packet buffer (`packet_req` indicates request vs response)
//   - Serializes variable-length packets to TX byte stream
//   - Little-endian data format for x86/ARM compatibility
//   - Standard valid/ready handshaking on packet input and TX output
//
// Packet Types:
//   0000 = CPU-initiated request (packet_req = 1)
//   0011 = FPGA response to host request (packet_req = 0)

module host_bus_tx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,

    // TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,

    // Unified Packet Input Interface
    input  logic        packet_valid,
    input  logic        packet_req,
    input  logic        packet_we,
    input  logic [1:0]  packet_size,
    input  logic [31:0] packet_addr,
    input  logic [31:0] packet_data,
    output logic        packet_ready
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [3:0] {
        STATE_IDLE       = 4'd0,
        STATE_TX_HEADER  = 4'd1,
        STATE_TX_ADDR_0  = 4'd2,
        STATE_TX_ADDR_1  = 4'd3,
        STATE_TX_ADDR_2  = 4'd4,
        STATE_TX_ADDR_3  = 4'd5,
        STATE_TX_DATA_0  = 4'd6,
        STATE_TX_DATA_1  = 4'd7,
        STATE_TX_DATA_2  = 4'd8,
        STATE_TX_DATA_3  = 4'd9
    } state_t;

    state_t state, next_state;

    // ============================================================
    // Storage Registers
    // ============================================================
    logic        packet_valid_reg;
    logic        packet_req_reg;
    logic        packet_we_reg;
    logic [1:0]  packet_size_reg;
    logic [31:0] packet_addr_reg;
    logic [31:0] packet_data_reg;

    logic        tx_handshake;
    logic        tx_packet_complete;
    logic [7:0]  tx_byte;

    assign tx_handshake = tx_valid && tx_ready;

    // Ready to accept a new packet only when idle and no buffered packet exists
    assign packet_ready = (state == STATE_IDLE) && !packet_valid_reg;

    // TX active whenever state machine is not idle
    assign tx_valid = (state != STATE_IDLE);

    // ============================================================
    // State Register
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;
        end else begin
            state <= next_state;
        end
    end

    // ============================================================
    // Next State Logic
    // ============================================================
    always_comb begin
        next_state = state;

        case (state)
            STATE_IDLE: begin
                if (packet_valid_reg || (packet_valid && packet_ready)) begin
                    next_state = STATE_TX_HEADER;
                end
            end

            STATE_TX_HEADER: begin
                if (tx_handshake) begin
                    if (packet_req_reg) begin
                        next_state = STATE_TX_ADDR_0;
                    end else if (packet_we_reg) begin
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_TX_DATA_0;
                    end
                end
            end

            STATE_TX_ADDR_0: begin
                if (tx_handshake) next_state = STATE_TX_ADDR_1;
            end

            STATE_TX_ADDR_1: begin
                if (tx_handshake) next_state = STATE_TX_ADDR_2;
            end

            STATE_TX_ADDR_2: begin
                if (tx_handshake) next_state = STATE_TX_ADDR_3;
            end

            STATE_TX_ADDR_3: begin
                if (tx_handshake) begin
                    if (packet_we_reg) begin
                        next_state = STATE_TX_DATA_0;
                    end else begin
                        next_state = STATE_IDLE;
                    end
                end
            end

            STATE_TX_DATA_0: begin
                if (tx_handshake) begin
                    if (packet_size_reg == 2'b00) begin
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_TX_DATA_1;
                    end
                end
            end

            STATE_TX_DATA_1: begin
                if (tx_handshake) begin
                    if (packet_size_reg == 2'b01) begin
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_TX_DATA_2;
                    end
                end
            end

            STATE_TX_DATA_2: begin
                if (tx_handshake) next_state = STATE_TX_DATA_3;
            end

            STATE_TX_DATA_3: begin
                if (tx_handshake) next_state = STATE_IDLE;
            end

            default: next_state = STATE_IDLE;
        endcase
    end

    // ============================================================
    // Packet Buffer Capture and Completion
    // ============================================================
    always_comb begin
        tx_packet_complete = 1'b0;

        case (state)
            STATE_TX_HEADER: begin
                tx_packet_complete = tx_handshake && !packet_req_reg && packet_we_reg;
            end
            STATE_TX_ADDR_3: begin
                tx_packet_complete = tx_handshake && packet_req_reg && !packet_we_reg;
            end
            STATE_TX_DATA_0: begin
                tx_packet_complete = tx_handshake && (packet_size_reg == 2'b00);
            end
            STATE_TX_DATA_1: begin
                tx_packet_complete = tx_handshake && (packet_size_reg == 2'b01);
            end
            STATE_TX_DATA_3: begin
                tx_packet_complete = tx_handshake;
            end
            default: begin
                tx_packet_complete = 1'b0;
            end
        endcase
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            packet_valid_reg <= 1'b0;
            packet_req_reg   <= 1'b0;
            packet_we_reg    <= 1'b0;
            packet_size_reg  <= 2'b00;
            packet_addr_reg  <= 32'h0;
            packet_data_reg  <= 32'h0;
        end else begin
            if (packet_valid && packet_ready) begin
                packet_valid_reg <= 1'b1;
                packet_req_reg   <= packet_req;
                packet_we_reg    <= packet_we;
                packet_size_reg  <= packet_size;
                packet_addr_reg  <= packet_addr;
                packet_data_reg  <= packet_data;
            end else if (tx_packet_complete) begin
                packet_valid_reg <= 1'b0;
            end
        end
    end

    // ============================================================
    // TX Data Mux (Little-endian for address/data payload)
    // Header format: {packet_type[3:0], size[1:0], 1'b0, we}
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;

        case (state)
            STATE_TX_HEADER: tx_byte = {packet_req_reg ? 4'b0000 : 4'b0011, packet_size_reg, 1'b0, packet_we_reg};
            STATE_TX_ADDR_0: tx_byte = packet_addr_reg[7:0];
            STATE_TX_ADDR_1: tx_byte = packet_addr_reg[15:8];
            STATE_TX_ADDR_2: tx_byte = packet_addr_reg[23:16];
            STATE_TX_ADDR_3: tx_byte = packet_addr_reg[31:24];
            STATE_TX_DATA_0: tx_byte = packet_data_reg[7:0];
            STATE_TX_DATA_1: tx_byte = packet_data_reg[15:8];
            STATE_TX_DATA_2: tx_byte = packet_data_reg[23:16];
            STATE_TX_DATA_3: tx_byte = packet_data_reg[31:24];
            default: ;
        endcase
    end

    assign tx_data = tx_byte;

endmodule

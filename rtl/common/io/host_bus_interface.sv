`default_nettype none
// Host Bus Interface Module
// Routes host bus transactions between system bus, RX parser, and TX serializer.
// Supports burst-native metadata framing and beat-stream execution.

module host_bus_interface (
    // Clock and reset
    input wire logic        clk,
    input wire logic        rst_n,

    // CPU slave interface (CPU->Host path)
    input wire logic [31:0] mem_a_addr,
    input wire logic [31:0] mem_a_wdata,
    input wire logic        mem_a_we,
    input wire logic [1:0]  mem_a_size,
    input wire logic        mem_a_valid,
    output logic        mem_a_ready,

    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input wire logic        mem_d_ready,

    // Host-initiated master interface (Host->RTL path)
    output logic [31:0] host_mem_a_addr,
    output logic [31:0] host_mem_a_wdata,
    output logic        host_mem_a_we,
    output logic [1:0]  host_mem_a_size,
    output logic        host_mem_a_valid,
    input wire logic        host_mem_a_ready,

    input wire logic [31:0] host_mem_d_rdata,
    input wire logic        host_mem_d_valid,
    output logic        host_mem_d_ready,

    // Host TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input wire logic        tx_ready,

    // Host RX Interface (from External Host)
    input wire logic [7:0]  rx_data,
    input wire logic        rx_valid,
    output logic        rx_ready
);

    // ============================================================
    // RX beat stream signals
    // ============================================================
    logic        rx_pkt_valid;
    logic        rx_pkt_ready;
    logic        rx_pkt_start;
    logic        rx_pkt_last;
    logic        rx_pkt_req;
    logic        rx_pkt_we;
    logic [1:0]  rx_pkt_size;
    logic        rx_pkt_src_fixed;
    logic        rx_pkt_dst_fixed;
    logic [15:0] rx_pkt_burst_len_m1;
    logic [31:0] rx_pkt_base_addr;
    logic [31:0] rx_pkt_data;

    // ============================================================
    // TX beat stream signals
    // ============================================================
    logic        tx_pkt_valid;
    logic        tx_pkt_ready;
    logic        tx_pkt_start;
    logic        tx_pkt_last;
    logic        tx_pkt_req;
    logic        tx_pkt_we;
    logic [1:0]  tx_pkt_size;
    logic        tx_pkt_src_fixed;
    logic        tx_pkt_dst_fixed;
    logic [15:0] tx_pkt_burst_len_m1;
    logic [31:0] tx_pkt_base_addr;
    logic [31:0] tx_pkt_data;

    // ============================================================
    // CPU request/response tracking
    // ============================================================
    logic [31:0] cpu_cap_addr;
    logic [31:0] cpu_cap_wdata;
    logic        cpu_cap_we;
    logic [1:0]  cpu_cap_size;
    logic        cpu_req_pending;
    logic        cpu_wait_resp;
    logic [31:0] cpu_resp_data;
    logic        cpu_resp_valid;

    // ============================================================
    // Host transaction execution state
    // ============================================================
    typedef enum logic [2:0] {
        HOST_IDLE        = 3'd0,
        HOST_WRITE_A     = 3'd1,
        HOST_WRITE_D     = 3'd2,
        HOST_WRITE_WAIT  = 3'd3,
        HOST_WRITE_RESP  = 3'd4,
        HOST_READ_A      = 3'd5,
        HOST_READ_D      = 3'd6,
        HOST_READ_TX     = 3'd7
    } host_state_t;

    host_state_t host_state;

    logic        host_req_we;
    logic [1:0]  host_req_size;
    logic        host_req_src_fixed;
    logic        host_req_dst_fixed;
    logic [15:0] host_req_burst_len_m1;
    logic [31:0] host_req_base_addr;
    logic [31:0] host_curr_addr;
    logic [2:0]  host_stride;
    logic [16:0] host_beats_remaining;
    logic [31:0] host_write_data;
    logic [31:0] host_read_data;
    logic        host_read_first_beat;

    logic host_a_handshake;
    logic host_d_handshake;
    assign host_a_handshake = host_mem_a_valid && host_mem_a_ready;
    assign host_d_handshake = host_mem_d_valid && host_mem_d_ready;

    // ============================================================
    // RX/TX submodules
    // ============================================================
    host_bus_rx rx_buf (
        .clk(clk),
        .rst_n(rst_n),
        .rx_data(rx_data),
        .rx_valid(rx_valid),
        .rx_ready(rx_ready),
        .packet_valid(rx_pkt_valid),
        .packet_start(rx_pkt_start),
        .packet_last(rx_pkt_last),
        .packet_req(rx_pkt_req),
        .packet_we(rx_pkt_we),
        .packet_size(rx_pkt_size),
        .packet_src_fixed(rx_pkt_src_fixed),
        .packet_dst_fixed(rx_pkt_dst_fixed),
        .packet_burst_len_m1(rx_pkt_burst_len_m1),
        .packet_base_addr(rx_pkt_base_addr),
        .packet_data(rx_pkt_data),
        .packet_ready(rx_pkt_ready)
    );

    host_bus_tx tx_buf (
        .clk(clk),
        .rst_n(rst_n),
        .tx_data(tx_data),
        .tx_valid(tx_valid),
        .tx_ready(tx_ready),
        .packet_valid(tx_pkt_valid),
        .packet_ready(tx_pkt_ready),
        .packet_start(tx_pkt_start),
        .packet_last(tx_pkt_last),
        .packet_req(tx_pkt_req),
        .packet_we(tx_pkt_we),
        .packet_size(tx_pkt_size),
        .packet_src_fixed(tx_pkt_src_fixed),
        .packet_dst_fixed(tx_pkt_dst_fixed),
        .packet_burst_len_m1(tx_pkt_burst_len_m1),
        .packet_base_addr(tx_pkt_base_addr),
        .packet_data(tx_pkt_data)
    );

    // ============================================================
    // CPU slave interface
    // ============================================================
    logic cpu_a_handshake;
    logic cpu_d_handshake;
    assign cpu_a_handshake = mem_a_valid && mem_a_ready;
    assign cpu_d_handshake = mem_d_valid && mem_d_ready;

    assign mem_a_ready = !cpu_req_pending && !cpu_wait_resp && !cpu_resp_valid;
    assign mem_d_rdata = cpu_resp_data;
    assign mem_d_valid = cpu_resp_valid;

    // ============================================================
    // RX packet consumption
    // ============================================================
    always_comb begin
        rx_pkt_ready = 1'b0;

        // CPU response path (host response to CPU-originated request)
        if (cpu_wait_resp && rx_pkt_valid && !rx_pkt_req) begin
            rx_pkt_ready = 1'b1;
        end else begin
            case (host_state)
                HOST_IDLE: begin
                    // Accept host request start beat (or metadata-only read request)
                    if (rx_pkt_valid && rx_pkt_req && rx_pkt_start) begin
                        rx_pkt_ready = 1'b1;
                    end
                end

                HOST_WRITE_WAIT: begin
                    // Accept next write payload beat from host RX
                    if (rx_pkt_valid && rx_pkt_req && !rx_pkt_start) begin
                        rx_pkt_ready = 1'b1;
                    end
                end

                default: begin
                    rx_pkt_ready = 1'b0;
                end
            endcase
        end
    end

    // ============================================================
    // TX arbitration (host responses have priority over CPU requests)
    // ============================================================
    always_comb begin
        tx_pkt_valid        = 1'b0;
        tx_pkt_start        = 1'b0;
        tx_pkt_last         = 1'b0;
        tx_pkt_req          = 1'b0;
        tx_pkt_we           = 1'b0;
        tx_pkt_size         = 2'b00;
        tx_pkt_src_fixed    = 1'b0;
        tx_pkt_dst_fixed    = 1'b0;
        tx_pkt_burst_len_m1 = 16'h0000;
        tx_pkt_base_addr    = 32'h0000_0000;
        tx_pkt_data         = 32'h0000_0000;

        case (host_state)
            HOST_WRITE_RESP: begin
                tx_pkt_valid        = 1'b1;
                tx_pkt_start        = 1'b1;
                tx_pkt_last         = 1'b1;
                tx_pkt_req          = 1'b0;
                tx_pkt_we           = 1'b1;
                tx_pkt_size         = host_req_size;
                tx_pkt_src_fixed    = host_req_src_fixed;
                tx_pkt_dst_fixed    = host_req_dst_fixed;
                tx_pkt_burst_len_m1 = host_req_burst_len_m1;
                tx_pkt_base_addr    = host_req_base_addr;
            end

            HOST_READ_TX: begin
                tx_pkt_valid        = 1'b1;
                tx_pkt_start        = host_read_first_beat;
                tx_pkt_last         = (host_beats_remaining == 17'd1);
                tx_pkt_req          = 1'b0;
                tx_pkt_we           = 1'b0;
                tx_pkt_size         = host_req_size;
                tx_pkt_src_fixed    = host_req_src_fixed;
                tx_pkt_dst_fixed    = host_req_dst_fixed;
                tx_pkt_burst_len_m1 = host_req_burst_len_m1;
                tx_pkt_base_addr    = host_req_base_addr;
                tx_pkt_data         = host_read_data;
            end

            default: begin
                if (cpu_req_pending) begin
                    tx_pkt_valid        = 1'b1;
                    tx_pkt_start        = 1'b1;
                    tx_pkt_last         = 1'b1;
                    tx_pkt_req          = 1'b1;
                    tx_pkt_we           = cpu_cap_we;
                    tx_pkt_size         = cpu_cap_size;
                    tx_pkt_src_fixed    = 1'b0;
                    tx_pkt_dst_fixed    = 1'b0;
                    tx_pkt_burst_len_m1 = 16'h0000;
                    tx_pkt_base_addr    = cpu_cap_addr;
                    tx_pkt_data         = cpu_cap_wdata;
                end
            end
        endcase
    end

    // ============================================================
    // Bus master drive (host-initiated request execution)
    // ============================================================
    assign host_mem_a_addr  = host_curr_addr;
    assign host_mem_a_wdata = host_write_data;
    assign host_mem_a_we    = host_req_we;
    assign host_mem_a_size  = host_req_size;
    assign host_mem_a_valid = (host_state == HOST_WRITE_A) || (host_state == HOST_READ_A);
    assign host_mem_d_ready = (host_state == HOST_WRITE_D) || (host_state == HOST_READ_D);

    // ============================================================
    // Sequential control
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            cpu_cap_addr    <= 32'h0000_0000;
            cpu_cap_wdata   <= 32'h0000_0000;
            cpu_cap_we      <= 1'b0;
            cpu_cap_size    <= 2'b00;
            cpu_req_pending <= 1'b0;
            cpu_wait_resp   <= 1'b0;
            cpu_resp_data   <= 32'h0000_0000;
            cpu_resp_valid  <= 1'b0;

            host_state           <= HOST_IDLE;
            host_req_we          <= 1'b0;
            host_req_size        <= 2'b00;
            host_req_src_fixed   <= 1'b0;
            host_req_dst_fixed   <= 1'b0;
            host_req_burst_len_m1<= 16'h0000;
            host_req_base_addr   <= 32'h0000_0000;
            host_curr_addr       <= 32'h0000_0000;
            host_stride          <= 3'd1;
            host_beats_remaining <= 17'd0;
            host_write_data      <= 32'h0000_0000;
            host_read_data       <= 32'h0000_0000;
            host_read_first_beat <= 1'b0;
        end else begin
            // Capture CPU request (single outstanding)
            if (cpu_a_handshake) begin
                cpu_cap_addr    <= mem_a_addr;
                cpu_cap_wdata   <= mem_a_wdata;
                cpu_cap_we      <= mem_a_we;
                cpu_cap_size    <= mem_a_size;
                cpu_req_pending <= 1'b1;
            end

            // CPU request accepted by TX
            if (cpu_req_pending && tx_pkt_valid && tx_pkt_ready && tx_pkt_req) begin
                cpu_req_pending <= 1'b0;
                cpu_wait_resp   <= 1'b1;
            end

            // CPU response consumed by the CPU D channel
            if (cpu_d_handshake) begin
                cpu_resp_valid <= 1'b0;
            end

            // CPU response captured from RX
            if (cpu_wait_resp && rx_pkt_valid && rx_pkt_ready && !rx_pkt_req) begin
                cpu_wait_resp <= 1'b0;
                cpu_resp_data <= rx_pkt_data;
                cpu_resp_valid <= 1'b1;
            end

            case (host_state)
                HOST_IDLE: begin
                    if (rx_pkt_valid && rx_pkt_ready && rx_pkt_req && rx_pkt_start) begin
                        host_req_we           <= rx_pkt_we;
                        host_req_size         <= rx_pkt_size;
                        host_req_src_fixed    <= rx_pkt_src_fixed;
                        host_req_dst_fixed    <= rx_pkt_dst_fixed;
                        host_req_burst_len_m1 <= rx_pkt_burst_len_m1;
                        host_req_base_addr    <= rx_pkt_base_addr;
                        host_curr_addr        <= rx_pkt_base_addr;
                        host_beats_remaining  <= {1'b0, rx_pkt_burst_len_m1} + 17'd1;

                        case (rx_pkt_size)
                            2'b00: host_stride <= 3'd1;
                            2'b01: host_stride <= 3'd2;
                            default: host_stride <= 3'd4;
                        endcase

                        if (rx_pkt_we) begin
                            host_write_data <= rx_pkt_data;
                            host_state <= HOST_WRITE_A;
                        end else begin
                            host_read_first_beat <= 1'b1;
                            host_state <= HOST_READ_A;
                        end
                    end
                end

                HOST_WRITE_A: begin
                    if (host_a_handshake) begin
                        host_state <= HOST_WRITE_D;
                    end
                end

                HOST_WRITE_D: begin
                    if (host_d_handshake) begin
                        if (host_beats_remaining == 17'd1) begin
                            host_state <= HOST_WRITE_RESP;
                        end else begin
                            host_beats_remaining <= host_beats_remaining - 17'd1;
                            if (!host_req_dst_fixed) begin
                                host_curr_addr <= host_curr_addr + {{29{1'b0}}, host_stride};
                            end
                            host_state <= HOST_WRITE_WAIT;
                        end
                    end
                end

                HOST_WRITE_WAIT: begin
                    if (rx_pkt_valid && rx_pkt_ready && rx_pkt_req && !rx_pkt_start) begin
                        host_write_data <= rx_pkt_data;
                        host_state <= HOST_WRITE_A;
                    end
                end

                HOST_WRITE_RESP: begin
                    if (tx_pkt_valid && tx_pkt_ready) begin
                        host_state <= HOST_IDLE;
                    end
                end

                HOST_READ_A: begin
                    if (host_a_handshake) begin
                        host_state <= HOST_READ_D;
                    end
                end

                HOST_READ_D: begin
                    if (host_d_handshake) begin
                        host_read_data <= host_mem_d_rdata;
                        host_state <= HOST_READ_TX;
                    end
                end

                HOST_READ_TX: begin
                    if (tx_pkt_valid && tx_pkt_ready) begin
                        if (host_read_first_beat) begin
                            host_read_first_beat <= 1'b0;
                        end
                        if (host_beats_remaining == 17'd1) begin
                            host_state <= HOST_IDLE;
                        end else begin
                            host_beats_remaining <= host_beats_remaining - 17'd1;
                            if (!host_req_src_fixed) begin
                                host_curr_addr <= host_curr_addr + {{29{1'b0}}, host_stride};
                            end
                            host_state <= HOST_READ_A;
                        end
                    end
                end

                default: host_state <= HOST_IDLE;
            endcase
        end
    end

`ifdef ASSERT_ON
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            // no-op
        end else if (cpu_wait_resp && rx_pkt_valid && rx_pkt_ready && !rx_pkt_req) begin
            assert (rx_pkt_start)
                else $error("host_bus_interface: CPU response beat missing packet_start");
            assert (rx_pkt_last)
                else $error("host_bus_interface: CPU response must be single-beat (packet_last=1)");
            assert (rx_pkt_burst_len_m1 == 16'h0000)
                else $error("host_bus_interface: CPU response burst_len_m1 must be 0");
        end
    end
`endif

endmodule
`default_nettype wire

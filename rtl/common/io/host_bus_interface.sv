`default_nettype none
// Host Bus Interface Module
// Routes host bus transactions between system bus, RX parser, and TX serializer.
// Supports burst-native metadata framing and beat-stream execution.

module host_bus_interface (
    // Clock and reset
    input wire logic        clk,
    input wire logic        rst,

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

    logic        tx_issue_valid;
    logic        tx_issue_ready;
    logic        tx_issue_start;
    logic        tx_issue_last;
    logic        tx_issue_req;
    logic        tx_issue_we;
    logic [1:0]  tx_issue_size;
    logic        tx_issue_src_fixed;
    logic        tx_issue_dst_fixed;
    logic [15:0] tx_issue_burst_len_m1;
    logic [31:0] tx_issue_base_addr;
    logic [31:0] tx_issue_data;
    logic        tx_slice_valid;
    logic        tx_slice_start;
    logic        tx_slice_last;
    logic        tx_slice_req;
    logic        tx_slice_we;
    logic [1:0]  tx_slice_size;
    logic        tx_slice_src_fixed;
    logic        tx_slice_dst_fixed;
    logic [15:0] tx_slice_burst_len_m1;
    logic [31:0] tx_slice_base_addr;
    logic [31:0] tx_slice_data;

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
    logic [31:0] host_next_addr;
    logic [2:0]  host_stride;
    logic [16:0] host_beats_remaining;
    logic [31:0] host_write_data;
    logic [31:0] host_read_data;
    logic        host_read_first_beat;
    logic        host_addr_fixed;

    logic host_a_handshake;
    logic host_d_handshake;
    assign host_a_handshake = host_mem_a_valid && host_mem_a_ready;
    assign host_d_handshake = host_mem_d_valid && host_mem_d_ready;

    logic host_in_idle;
    logic host_in_write_a;
    logic host_in_write_d;
    logic host_in_write_wait;
    logic host_in_write_resp;
    logic host_in_read_a;
    logic host_in_read_d;
    logic host_in_read_tx;
    logic host_mem_a_phase;
    logic host_mem_d_phase;
    logic host_last_beat;
    logic accept_cpu_resp;
    logic accept_host_req_start;
    logic accept_host_write_payload;
    logic rx_pkt_addr_fixed;
    logic rx_pkt_addr_increments;
    logic [2:0]  rx_pkt_stride;
    logic [31:0] host_stride_ext;
    logic [31:0] host_next_addr_advance;
    logic tx_issue_handshake;
    logic tx_output_handshake;

    assign host_in_idle       = (host_state == HOST_IDLE);
    assign host_in_write_a    = (host_state == HOST_WRITE_A);
    assign host_in_write_d    = (host_state == HOST_WRITE_D);
    assign host_in_write_wait = (host_state == HOST_WRITE_WAIT);
    assign host_in_write_resp = (host_state == HOST_WRITE_RESP);
    assign host_in_read_a     = (host_state == HOST_READ_A);
    assign host_in_read_d     = (host_state == HOST_READ_D);
    assign host_in_read_tx    = (host_state == HOST_READ_TX);

    assign host_mem_a_phase = host_in_write_a || host_in_read_a;
    assign host_mem_d_phase = host_in_write_d || host_in_read_d;
    assign host_last_beat   = (host_beats_remaining == 17'd1);

    assign accept_cpu_resp          = cpu_wait_resp && rx_pkt_valid && !rx_pkt_req;
    assign accept_host_req_start    = host_in_idle && rx_pkt_valid && rx_pkt_req && rx_pkt_start;
    assign accept_host_write_payload = host_in_write_wait && rx_pkt_valid && rx_pkt_req && !rx_pkt_start;
    assign rx_pkt_addr_fixed        = rx_pkt_we ? rx_pkt_dst_fixed : rx_pkt_src_fixed;
    assign rx_pkt_addr_increments   = !rx_pkt_addr_fixed;

    assign rx_pkt_stride = (rx_pkt_size == 2'b00) ? 3'd1 :
                           (rx_pkt_size == 2'b01) ? 3'd2 : 3'd4;
    assign host_stride_ext       = {{29{1'b0}}, host_stride};
    assign host_next_addr_advance = host_next_addr + host_stride_ext;
    assign tx_issue_handshake    = tx_issue_valid && tx_issue_ready;
    assign tx_output_handshake   = tx_pkt_valid && tx_pkt_ready;
    assign tx_pkt_valid          = tx_slice_valid;
    assign tx_pkt_start          = tx_slice_valid ? tx_slice_start : tx_issue_start;
    assign tx_pkt_last           = tx_slice_last;
    assign tx_pkt_req            = tx_slice_req;
    assign tx_pkt_we             = tx_slice_we;
    assign tx_pkt_size           = tx_slice_size;
    assign tx_pkt_src_fixed      = tx_slice_src_fixed;
    assign tx_pkt_dst_fixed      = tx_slice_dst_fixed;
    assign tx_pkt_burst_len_m1   = tx_slice_burst_len_m1;
    assign tx_pkt_base_addr      = tx_slice_base_addr;
    assign tx_pkt_data           = tx_slice_data;

    // ============================================================
    // RX/TX submodules
    // ============================================================
    host_bus_rx rx_buf (
        .clk(clk),
        .rst(rst),
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
        .rst(rst),
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

        if (accept_cpu_resp) begin
            rx_pkt_ready = 1'b1;
        end else if (accept_host_req_start || accept_host_write_payload) begin
            rx_pkt_ready = 1'b1;
        end
    end

    // ============================================================
    // TX arbitration (host responses have priority over CPU requests)
    // ============================================================
    always_comb begin
        tx_issue_valid        = 1'b0;
        tx_issue_start        = 1'b0;
        tx_issue_last         = 1'b0;
        tx_issue_req          = 1'b0;
        tx_issue_we           = 1'b0;
        tx_issue_size         = 2'b00;
        tx_issue_src_fixed    = 1'b0;
        tx_issue_dst_fixed    = 1'b0;
        tx_issue_burst_len_m1 = 16'h0000;
        tx_issue_base_addr    = 32'h0000_0000;
        tx_issue_data         = 32'h0000_0000;

        if (host_in_write_resp) begin
            tx_issue_valid        = 1'b1;
            tx_issue_start        = 1'b1;
            tx_issue_last         = 1'b1;
            tx_issue_we           = 1'b1;
            tx_issue_size         = host_req_size;
            tx_issue_src_fixed    = host_req_src_fixed;
            tx_issue_dst_fixed    = host_req_dst_fixed;
            tx_issue_burst_len_m1 = host_req_burst_len_m1;
            tx_issue_base_addr    = host_req_base_addr;
        end else if (host_in_read_tx) begin
            tx_issue_valid        = 1'b1;
            tx_issue_start        = host_read_first_beat;
            tx_issue_last         = host_last_beat;
            tx_issue_size         = host_req_size;
            tx_issue_src_fixed    = host_req_src_fixed;
            tx_issue_dst_fixed    = host_req_dst_fixed;
            tx_issue_burst_len_m1 = host_req_burst_len_m1;
            tx_issue_base_addr    = host_req_base_addr;
            tx_issue_data         = host_read_data;
        end else if (cpu_req_pending) begin
            tx_issue_valid        = 1'b1;
            tx_issue_start        = 1'b1;
            tx_issue_last         = 1'b1;
            tx_issue_req          = 1'b1;
            tx_issue_we           = cpu_cap_we;
            tx_issue_size         = cpu_cap_size;
            tx_issue_burst_len_m1 = 16'h0000;
            tx_issue_base_addr    = cpu_cap_addr;
            tx_issue_data         = cpu_cap_wdata;
        end
    end

    assign tx_issue_ready = !tx_slice_valid && tx_pkt_ready;

    // ============================================================
    // Bus master drive (host-initiated request execution)
    // ============================================================
    assign host_mem_a_addr  = host_curr_addr;
    assign host_mem_a_wdata = host_write_data;
    assign host_mem_a_we    = host_req_we;
    assign host_mem_a_size  = host_req_size;
    assign host_mem_a_valid = host_mem_a_phase;
    assign host_mem_d_ready = host_mem_d_phase;

    // ============================================================
    // Sequential control
    // ============================================================
    always_ff @(posedge clk) begin
        if (rst) begin
            cpu_req_pending <= 1'b0;
            cpu_wait_resp   <= 1'b0;
            cpu_resp_valid  <= 1'b0;
            tx_slice_valid  <= 1'b0;

            host_state           <= HOST_IDLE;
            host_beats_remaining <= 17'd0;
            host_read_first_beat <= 1'b0;
            host_addr_fixed      <= 1'b0;
        end else begin
            if (tx_issue_handshake) begin
                tx_slice_valid          <= 1'b1;
                tx_slice_start          <= tx_issue_start;
                tx_slice_last           <= tx_issue_last;
                tx_slice_req            <= tx_issue_req;
                tx_slice_we             <= tx_issue_we;
                tx_slice_size           <= tx_issue_size;
                tx_slice_src_fixed      <= tx_issue_src_fixed;
                tx_slice_dst_fixed      <= tx_issue_dst_fixed;
                tx_slice_burst_len_m1   <= tx_issue_burst_len_m1;
                tx_slice_base_addr      <= tx_issue_base_addr;
                tx_slice_data           <= tx_issue_data;
            end else if (tx_output_handshake) begin
                tx_slice_valid <= 1'b0;
            end

            // Capture CPU request (single outstanding)
            if (cpu_a_handshake) begin
                cpu_cap_addr    <= mem_a_addr;
                cpu_cap_wdata   <= mem_a_wdata;
                cpu_cap_we      <= mem_a_we;
                cpu_cap_size    <= mem_a_size;
                cpu_req_pending <= 1'b1;
            end

            // CPU request accepted by TX
            if (cpu_req_pending && tx_output_handshake && tx_pkt_req) begin
                cpu_req_pending <= 1'b0;
                cpu_wait_resp   <= 1'b1;
            end

            // CPU response consumed by the CPU D channel
            if (cpu_d_handshake) begin
                cpu_resp_valid <= 1'b0;
            end

            // CPU response captured from RX
            if (accept_cpu_resp && rx_pkt_ready) begin
                cpu_wait_resp <= 1'b0;
                cpu_resp_data  <= rx_pkt_data;
                cpu_resp_valid <= 1'b1;
            end

            case (host_state)
                HOST_IDLE: begin
                    if (accept_host_req_start && rx_pkt_ready) begin
                        host_req_we           <= rx_pkt_we;
                        host_req_size         <= rx_pkt_size;
                        host_req_src_fixed    <= rx_pkt_src_fixed;
                        host_req_dst_fixed    <= rx_pkt_dst_fixed;
                        host_req_burst_len_m1 <= rx_pkt_burst_len_m1;
                        host_req_base_addr    <= rx_pkt_base_addr;
                        host_curr_addr        <= rx_pkt_base_addr;
                        host_addr_fixed       <= rx_pkt_addr_fixed;
                        host_beats_remaining  <= {1'b0, rx_pkt_burst_len_m1} + 17'd1;
                        host_stride           <= rx_pkt_stride;
                        if (rx_pkt_addr_increments) begin
                            host_next_addr <= rx_pkt_base_addr + {{29{1'b0}}, rx_pkt_stride};
                        end else begin
                            host_next_addr <= rx_pkt_base_addr;
                        end

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
                        if (host_last_beat) begin
                            host_state <= HOST_WRITE_RESP;
                        end else begin
                            host_beats_remaining <= host_beats_remaining - 17'd1;
                            if (!host_addr_fixed) begin
                                host_curr_addr <= host_next_addr;
                                host_next_addr <= host_next_addr_advance;
                            end
                            host_state <= HOST_WRITE_WAIT;
                        end
                    end
                end

                HOST_WRITE_WAIT: begin
                    if (accept_host_write_payload && rx_pkt_ready) begin
                        host_write_data <= rx_pkt_data;
                        host_state <= HOST_WRITE_A;
                    end
                end

                HOST_WRITE_RESP: begin
                    if (tx_output_handshake) begin
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
                    if (tx_output_handshake) begin
                        if (host_read_first_beat) begin
                            host_read_first_beat <= 1'b0;
                        end
                        if (host_last_beat) begin
                            host_state <= HOST_IDLE;
                        end else begin
                            host_beats_remaining <= host_beats_remaining - 17'd1;
                            if (!host_addr_fixed) begin
                                host_curr_addr <= host_next_addr;
                                host_next_addr <= host_next_addr_advance;
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
        if (rst) begin
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

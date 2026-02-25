// Host Bus Interface Module
// Routes host bus transactions between system bus, RX buffer, and TX buffer

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,

    // Bus Slave Interface (from System Bus - CPU→Host path)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,

    // Bus Master Interface (to Arbiter - Host→CPU path)
    output logic [31:0] host_bus_addr,
    output logic [31:0] host_bus_wdata,
    input  logic [31:0] host_bus_rdata,
    output logic        host_bus_we,
    output logic [1:0]  host_bus_size,
    output logic        host_bus_req,
    input  logic        host_bus_ready,

    // Host TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,

    // Host RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready
);

    // ============================================================
    // RX Buffer Signals
    // ============================================================
    logic        buf_pkt_valid;
    logic        buf_pkt_req;
    logic        buf_pkt_we;
    logic [1:0]  buf_pkt_size;
    logic [31:0] buf_pkt_addr;
    logic [31:0] buf_pkt_data;
    logic        buf_pkt_ready;

    logic        buf_resp_valid;
    logic        buf_req_valid;
    logic        buf_resp_ready;
    logic        buf_req_ready;

    // ============================================================
    // TX Buffer Signals
    // ============================================================
    logic        tx_pkt_valid;
    logic        tx_pkt_req;
    logic        tx_pkt_we;
    logic [1:0]  tx_pkt_size;
    logic [31:0] tx_pkt_addr;
    logic [31:0] tx_pkt_data;
    logic        tx_pkt_ready;

    logic        tx_cpu_pkt_accepted;
    logic        tx_host_pkt_accepted;

    // ============================================================
    // CPU Request Tracking (CPU-initiated transactions)
    // ============================================================
    logic [31:0] cpu_cap_addr;
    logic [31:0] cpu_cap_wdata;
    logic        cpu_cap_we;
    logic [1:0]  cpu_cap_size;
    logic        cpu_req_pending;
    logic        cpu_wait_resp;

    // ============================================================
    // Host Response Tracking (for host-initiated transactions)
    // ============================================================
    logic [31:0] host_resp_rdata;
    logic        host_resp_we;
    logic [1:0]  host_resp_size;
    logic        host_resp_pending;

    logic bus_master_handshake_complete;
    assign bus_master_handshake_complete = host_bus_req && host_bus_ready;

    // ============================================================
    // RX Buffer Instance
    // ============================================================
    host_bus_rx rx_buf (
        .clk(clk),
        .rst_n(rst_n),
        .rx_data(rx_data),
        .rx_valid(rx_valid),
        .rx_ready(rx_ready),
        .packet_valid(buf_pkt_valid),
        .packet_req(buf_pkt_req),
        .packet_we(buf_pkt_we),
        .packet_size(buf_pkt_size),
        .packet_addr(buf_pkt_addr),
        .packet_data(buf_pkt_data),
        .packet_ready(buf_pkt_ready)
    );

    assign buf_resp_valid = buf_pkt_valid && !buf_pkt_req;
    assign buf_req_valid  = buf_pkt_valid && buf_pkt_req;

    // ============================================================
    // TX Buffer Input Arbitration
    // Priority: CPU requests > host responses
    // ============================================================
    always_comb begin
        tx_pkt_valid = 1'b0;
        tx_pkt_req   = 1'b0;
        tx_pkt_we    = 1'b0;
        tx_pkt_size  = 2'b00;
        tx_pkt_addr  = 32'h0;
        tx_pkt_data  = 32'h0;

        if (cpu_req_pending) begin
            tx_pkt_valid = 1'b1;
            tx_pkt_req   = 1'b1;
            tx_pkt_we    = cpu_cap_we;
            tx_pkt_size  = cpu_cap_size;
            tx_pkt_addr  = cpu_cap_addr;
            tx_pkt_data  = cpu_cap_wdata;
        end else if (host_resp_pending && !cpu_wait_resp) begin
            tx_pkt_valid = 1'b1;
            tx_pkt_req   = 1'b0;
            tx_pkt_we    = host_resp_we;
            tx_pkt_size  = host_resp_size;
            tx_pkt_addr  = 32'h0;
            tx_pkt_data  = host_resp_rdata;
        end
    end

    assign tx_cpu_pkt_accepted  = tx_pkt_valid && tx_pkt_ready && tx_pkt_req;
    assign tx_host_pkt_accepted = tx_pkt_valid && tx_pkt_ready && !tx_pkt_req;

    // ============================================================
    // TX Buffer Instance
    // ============================================================
    host_bus_tx tx_buf (
        .clk(clk),
        .rst_n(rst_n),
        .tx_data(tx_data),
        .tx_valid(tx_valid),
        .tx_ready(tx_ready),
        .packet_valid(tx_pkt_valid),
        .packet_req(tx_pkt_req),
        .packet_we(tx_pkt_we),
        .packet_size(tx_pkt_size),
        .packet_addr(tx_pkt_addr),
        .packet_data(tx_pkt_data),
        .packet_ready(tx_pkt_ready)
    );

    // ============================================================
    // CPU Slave Interface (ready when buffered host response is available)
    // ============================================================
    assign ready = cpu_wait_resp && buf_resp_valid;
    assign rdata = buf_pkt_data;

    // ============================================================
    // Buffer Ready Signals
    // ============================================================
    assign buf_resp_ready = ready;
    assign buf_req_ready  = bus_master_handshake_complete && !host_resp_pending;
    assign buf_pkt_ready  = (buf_resp_ready && buf_resp_valid) || (buf_req_ready && buf_req_valid);

    // ============================================================
    // Bus Master Interface (Host→CPU path)
    // ============================================================
    assign host_bus_addr  = buf_pkt_addr;
    assign host_bus_wdata = buf_pkt_data;
    assign host_bus_we    = buf_pkt_we;
    assign host_bus_size  = buf_pkt_size;
    assign host_bus_req   = buf_req_valid && !host_resp_pending;

    // ============================================================
    // Sequential request/response tracking
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cpu_cap_addr      <= 32'h0;
            cpu_cap_wdata     <= 32'h0;
            cpu_cap_we        <= 1'b0;
            cpu_cap_size      <= 2'b00;
            cpu_req_pending   <= 1'b0;
            cpu_wait_resp     <= 1'b0;

            host_resp_rdata   <= 32'h0;
            host_resp_we      <= 1'b0;
            host_resp_size    <= 2'b00;
            host_resp_pending <= 1'b0;
        end else begin
            // Capture CPU request (one at a time)
            if (!cpu_req_pending && !cpu_wait_resp && req) begin
                cpu_cap_addr    <= addr;
                cpu_cap_wdata   <= wdata;
                cpu_cap_we      <= we;
                cpu_cap_size    <= size;
                cpu_req_pending <= 1'b1;
            end

            // CPU request accepted by TX buffer
            if (tx_cpu_pkt_accepted) begin
                cpu_req_pending <= 1'b0;
                cpu_wait_resp   <= 1'b1;
            end

            // CPU response consumed from RX buffer
            if (buf_resp_ready) begin
                cpu_wait_resp <= 1'b0;
            end

            // Host response packet accepted by TX buffer
            if (tx_host_pkt_accepted) begin
                host_resp_pending <= 1'b0;
            end else if (bus_master_handshake_complete && !host_resp_pending) begin
                // Capture bus master completion as TX response payload
                host_resp_we      <= buf_pkt_we;
                host_resp_size    <= buf_pkt_size;
                host_resp_rdata   <= host_bus_rdata;
                host_resp_pending <= 1'b1;
            end
        end
    end

endmodule

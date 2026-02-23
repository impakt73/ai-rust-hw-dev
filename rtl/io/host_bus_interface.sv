// Host Bus Interface Module (Orchestrator)
// Coordinates RX and TX modules for external host communication
//
// Architecture:
//   - Instantiates host_bus_rx module for packet reception and parsing
//   - Instantiates host_bus_tx module for packet serialization and transmission
//   - Routes data between modules and top-level interfaces
//   - Implements handshake completion logic
//   - No FSM - simple combinational routing with registered handshakes
//
// Features:
//   - 32-bit bus slave interface compatible with system bus (CPU->Host requests)
//   - 32-bit bus master interface for Host->CPU requests (via arbiter)
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Extended header format with packet type bits
//   - Little-endian data format for x86/ARM compatibility
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length, Little-Endian, Extended Header):
//   CPU-initiated request (type 0000):  [ext_header][addr0-3][data...] (FPGA → Host TX)
//   Host response to CPU (type 0001):   [ext_header][data...]          (Host → FPGA RX)
//   Host-initiated request (type 0010): [ext_header][addr0-3][data...] (Host → FPGA RX)
//   FPGA response to Host (type 0011):  [ext_header][data...]          (FPGA → Host TX)
//
// Extended Header Format (1 byte):
//   Bits [7:4]: Packet type
//     0000 = CPU-initiated request (FPGA → Host TX)
//     0001 = Host response to CPU request (Host → FPGA RX)
//     0010 = Host-initiated request (Host → FPGA RX)
//     0011 = FPGA response to Host request (FPGA → Host TX)
//   Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
//   Bit  [1]:   Reserved (0)
//   Bit  [0]:   we (1=write, 0=read)

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
    // RX Module Instance
    // ============================================================
    logic        rx_resp_valid;
    // Metadata from RX response packets; consumed by simulation assertions below.
    logic        rx_resp_we;
    logic [1:0]  rx_resp_size;
    logic [31:0] rx_resp_rdata;
    logic        rx_resp_consumed;
    
    logic        rx_req_valid;
    logic        rx_req_we;
    logic [1:0]  rx_req_size;
    logic [31:0] rx_req_addr;
    logic [31:0] rx_req_wdata;
    logic        rx_req_consumed;
    
    host_bus_rx rx_module (
        .clk(clk),
        .rst_n(rst_n),
        
        // RX byte stream
        .rx_data(rx_data),
        .rx_valid(rx_valid),
        .rx_ready(rx_ready),
        
        // Response packet output (type 0001)
        .resp_valid(rx_resp_valid),
        .resp_we(rx_resp_we),
        .resp_size(rx_resp_size),
        .resp_rdata(rx_resp_rdata),
        .resp_consumed(rx_resp_consumed),
        
        // Request packet output (type 0010)
        .req_valid(rx_req_valid),
        .req_we(rx_req_we),
        .req_size(rx_req_size),
        .req_addr(rx_req_addr),
        .req_wdata(rx_req_wdata),
        .req_consumed(rx_req_consumed)
    );
    
    // ============================================================
    // TX Module Instance
    // ============================================================
    logic        tx_cpu_req_ready;
    logic        tx_cpu_req_valid;
    logic        tx_host_resp_valid;
    logic        tx_host_resp_ready;
    logic [31:0] tx_host_resp_rdata;
    logic        tx_host_resp_we;
    logic [1:0]  tx_host_resp_size;
    
    host_bus_tx tx_module (
        .clk(clk),
        .rst_n(rst_n),
        
        // TX byte stream
        .tx_data(tx_data),
        .tx_valid(tx_valid),
        .tx_ready(tx_ready),
        
        // CPU request input (type 0000)
        .cpu_req_addr(addr),
        .cpu_req_wdata(wdata),
        .cpu_req_we(we),
        .cpu_req_size(size),
        .cpu_req_valid(tx_cpu_req_valid),
        .cpu_req_ready(tx_cpu_req_ready),
        
        // Host response input (type 0011)
        .host_resp_rdata(tx_host_resp_rdata),
        .host_resp_we(tx_host_resp_we),
        .host_resp_size(tx_host_resp_size),
        .host_resp_valid(tx_host_resp_valid),
        .host_resp_ready(tx_host_resp_ready)
    );
    
    // ============================================================
    // CPU Slave Interface Routing
    // ============================================================
    logic cpu_req_inflight;
    logic cpu_req_last_valid;
    logic [31:0] cpu_req_last_addr;
    logic [31:0] cpu_req_last_wdata;
    logic        cpu_req_last_we;
    logic [1:0]  cpu_req_last_size;
    logic        cpu_req_changed;
    logic cpu_ready_pulse;

    assign cpu_req_changed = !cpu_req_last_valid ||
                             (addr  != cpu_req_last_addr)  ||
                             (we && (wdata != cpu_req_last_wdata)) ||
                             (we   != cpu_req_last_we)     ||
                             (size != cpu_req_last_size);
    assign tx_cpu_req_valid = req && !cpu_req_inflight && cpu_req_changed;

    // Ready signal: Transaction completes when response is received
    // For CPU-initiated transactions:
    //   1. CPU asserts req
    //   2. TX module transmits request packet
    //   3. RX module receives response packet and asserts rx_resp_valid
    //   4. Orchestrator asserts ready (transaction complete)
    //   5. CPU deasserts req
    //   6. Orchestrator pulses rx_resp_consumed
    
    assign ready = cpu_ready_pulse;
    
    // CPU read data comes from RX module response buffer
    assign rdata = rx_resp_rdata;

    // Consume resp_we/resp_size metadata for protocol sanity checks
`ifndef SYNTHESIS
    always_ff @(posedge clk) begin
        if (rx_resp_valid) begin
            assert (rx_resp_size != 2'b11)
            else $error("host_bus_interface: protocol violation, response size 2'b11 is reserved");
            if (rx_resp_we) begin
                assert (rx_resp_rdata == 32'h0)
                else $error("host_bus_interface: protocol violation, write response must have zero rdata");
            end
        end
    end

`endif
    
    // ============================================================
    // Bus Master Interface Routing
    // ============================================================
    // Host requests route directly from RX module to bus master
    assign host_bus_addr  = rx_req_addr;
    assign host_bus_wdata = rx_req_wdata;
    assign host_bus_we    = rx_req_we;
    assign host_bus_size  = rx_req_size;
    assign host_bus_req   = rx_req_valid;
    
    // ============================================================
    // Handshake Completion Logic
    // ============================================================
    
    // CPU-initiated transaction completion
    // When ready is asserted (response received), consume the response on next cycle
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_resp_consumed <= 1'b0;
            cpu_req_inflight <= 1'b0;
            cpu_req_last_valid <= 1'b0;
            cpu_req_last_addr  <= 32'h0;
            cpu_req_last_wdata <= 32'h0;
            cpu_req_last_we    <= 1'b0;
            cpu_req_last_size  <= 2'b00;
            cpu_ready_pulse <= 1'b0;
        end else begin
            cpu_ready_pulse <= 1'b0;
            rx_resp_consumed <= 1'b0;

            if (tx_cpu_req_valid && tx_cpu_req_ready) begin
                cpu_req_inflight <= 1'b1;
                cpu_req_last_valid <= 1'b1;
                cpu_req_last_addr  <= addr;
                cpu_req_last_wdata <= wdata;
                cpu_req_last_we    <= we;
                cpu_req_last_size  <= size;
            end

            if (rx_resp_valid && cpu_req_inflight) begin
                cpu_req_inflight <= 1'b0;
                cpu_ready_pulse <= 1'b1;
                rx_resp_consumed <= 1'b1;
            end

            if (!req) begin
                cpu_req_last_valid <= 1'b0;
            end
        end
    end
    
    // Host-initiated transaction completion
    // When bus master completes, capture response and send to TX module
    logic bus_master_handshake_complete;
    assign bus_master_handshake_complete = host_bus_req && host_bus_ready;
    
    logic host_resp_pending;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            host_resp_pending    <= 1'b0;
            rx_req_consumed      <= 1'b0;
            tx_host_resp_valid   <= 1'b0;
            tx_host_resp_rdata   <= 32'h0;
            tx_host_resp_we      <= 1'b0;
            tx_host_resp_size    <= 2'b00;
        end else begin
            // Step 1: Bus master completes → capture response, mark pending
            if (bus_master_handshake_complete && !host_resp_pending) begin
                host_resp_pending  <= 1'b1;
                rx_req_consumed    <= 1'b1;
                // Capture response data from bus master
                tx_host_resp_rdata <= host_bus_rdata;
                tx_host_resp_we    <= rx_req_we;    // Echo from original request
                tx_host_resp_size  <= rx_req_size;  // Echo from original request
            end else begin
                rx_req_consumed <= 1'b0;  // Single-cycle pulse
            end
            
            // Step 2: TX module ready → send response
            if (host_resp_pending && !tx_host_resp_valid) begin
                tx_host_resp_valid <= 1'b1;
            end
            
            // Step 3: TX module accepts → clear pending
            if (tx_host_resp_valid && tx_host_resp_ready) begin
                tx_host_resp_valid <= 1'b0;
                host_resp_pending  <= 1'b0;
            end
        end
    end

endmodule

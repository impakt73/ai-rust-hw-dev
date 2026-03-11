// Host Bus Mux
// Routes CPU memory transactions to either:
// - System bus path (RTL peripherals, addr[31] == 0)
// - Host bus interface path (Rust/external memory, addr[31] == 1)
//
// Uses the same decoupled address/data channel structure as the CPU memory
// interface. Internally, one request and one response can be buffered.

module host_bus_mux (
    input  logic        clk,
    input  logic        rst_n,

    // CPU-side interface
    input  logic [31:0] cpu_mem_a_addr,
    input  logic [31:0] cpu_mem_a_wdata,
    input  logic        cpu_mem_a_we,
    input  logic [1:0]  cpu_mem_a_size,
    input  logic        cpu_mem_a_valid,
    output logic        cpu_mem_a_ready,

    output logic [31:0] cpu_mem_d_rdata,
    output logic        cpu_mem_d_valid,
    input  logic        cpu_mem_d_ready,

    // System bus path (RTL peripherals)
    output logic [31:0] sys_mem_a_addr,
    output logic [31:0] sys_mem_a_wdata,
    output logic        sys_mem_a_we,
    output logic [1:0]  sys_mem_a_size,
    output logic        sys_mem_a_valid,
    input  logic        sys_mem_a_ready,

    input  logic [31:0] sys_mem_d_rdata,
    input  logic        sys_mem_d_valid,
    output logic        sys_mem_d_ready,

    // Host bus interface path (external memory)
    output logic [31:0] host_mem_a_addr,
    output logic [31:0] host_mem_a_wdata,
    output logic        host_mem_a_we,
    output logic [1:0]  host_mem_a_size,
    output logic        host_mem_a_valid,
    input  logic        host_mem_a_ready,

    input  logic [31:0] host_mem_d_rdata,
    input  logic        host_mem_d_valid,
    output logic        host_mem_d_ready
);
    logic [31:0] pending_req_addr;
    logic [31:0] pending_req_wdata;
    logic        pending_req_we;
    logic [1:0]  pending_req_size;
    logic        pending_req_valid;

    logic        pending_route_host;
    logic        waiting_for_resp;

    logic [31:0] pending_resp_rdata;
    logic        pending_resp_valid;

    logic [31:0] next_pending_req_addr;
    logic [31:0] next_pending_req_wdata;
    logic        next_pending_req_we;
    logic [1:0]  next_pending_req_size;
    logic        next_pending_req_valid;

    logic        next_pending_route_host;
    logic        next_waiting_for_resp;

    logic [31:0] next_pending_resp_rdata;
    logic        next_pending_resp_valid;

    logic        next_cpu_mem_a_ready;
    logic [31:0] next_cpu_mem_d_rdata;
    logic        next_cpu_mem_d_valid;

    logic [31:0] next_sys_mem_a_addr;
    logic [31:0] next_sys_mem_a_wdata;
    logic        next_sys_mem_a_we;
    logic [1:0]  next_sys_mem_a_size;
    logic        next_sys_mem_a_valid;
    logic        next_sys_mem_d_ready;

    logic [31:0] next_host_mem_a_addr;
    logic [31:0] next_host_mem_a_wdata;
    logic        next_host_mem_a_we;
    logic [1:0]  next_host_mem_a_size;
    logic        next_host_mem_a_valid;
    logic        next_host_mem_d_ready;

    logic cpu_a_handshake;
    logic cpu_d_handshake;
    logic sys_a_handshake;
    logic host_a_handshake;
    logic sys_d_handshake;
    logic host_d_handshake;

    assign cpu_a_handshake = cpu_mem_a_valid && cpu_mem_a_ready;
    assign cpu_d_handshake = cpu_mem_d_valid && cpu_mem_d_ready;
    assign sys_a_handshake = sys_mem_a_valid && sys_mem_a_ready;
    assign host_a_handshake = host_mem_a_valid && host_mem_a_ready;
    assign sys_d_handshake = waiting_for_resp && !pending_route_host && sys_mem_d_valid && sys_mem_d_ready;
    assign host_d_handshake = waiting_for_resp && pending_route_host && host_mem_d_valid && host_mem_d_ready;

    always_comb begin
        next_pending_req_addr = pending_req_addr;
        next_pending_req_wdata = pending_req_wdata;
        next_pending_req_we = pending_req_we;
        next_pending_req_size = pending_req_size;
        next_pending_req_valid = pending_req_valid;

        next_pending_route_host = pending_route_host;
        next_waiting_for_resp = waiting_for_resp;

        next_pending_resp_rdata = pending_resp_rdata;
        next_pending_resp_valid = pending_resp_valid;

        if (cpu_a_handshake) begin
            next_pending_req_addr = cpu_mem_a_addr;
            next_pending_req_wdata = cpu_mem_a_wdata;
            next_pending_req_we = cpu_mem_a_we;
            next_pending_req_size = cpu_mem_a_size;
            next_pending_req_valid = 1'b1;
        end

        if (sys_a_handshake) begin
            next_pending_req_valid = 1'b0;
            next_pending_route_host = 1'b0;
            next_waiting_for_resp = 1'b1;
        end else if (host_a_handshake) begin
            next_pending_req_valid = 1'b0;
            next_pending_route_host = 1'b1;
            next_waiting_for_resp = 1'b1;
        end

        if (sys_d_handshake) begin
            next_pending_resp_rdata = sys_mem_d_rdata;
            next_pending_resp_valid = 1'b1;
            next_waiting_for_resp = 1'b0;
        end else if (host_d_handshake) begin
            next_pending_resp_rdata = host_mem_d_rdata;
            next_pending_resp_valid = 1'b1;
            next_waiting_for_resp = 1'b0;
        end

        if (cpu_d_handshake) begin
            next_pending_resp_valid = 1'b0;
        end

        next_cpu_mem_a_ready = !next_pending_req_valid && !next_waiting_for_resp && !next_pending_resp_valid;
        next_cpu_mem_d_rdata = next_pending_resp_rdata;
        next_cpu_mem_d_valid = next_pending_resp_valid;

        next_sys_mem_a_addr = next_pending_req_addr;
        next_sys_mem_a_wdata = next_pending_req_wdata;
        next_sys_mem_a_we = next_pending_req_we;
        next_sys_mem_a_size = next_pending_req_size;
        next_sys_mem_a_valid = next_pending_req_valid && !next_pending_req_addr[31];
        next_sys_mem_d_ready = next_waiting_for_resp && !next_pending_resp_valid && !next_pending_route_host;

        next_host_mem_a_addr = next_pending_req_addr;
        next_host_mem_a_wdata = next_pending_req_wdata;
        next_host_mem_a_we = next_pending_req_we;
        next_host_mem_a_size = next_pending_req_size;
        next_host_mem_a_valid = next_pending_req_valid && next_pending_req_addr[31];
        next_host_mem_d_ready = next_waiting_for_resp && !next_pending_resp_valid && next_pending_route_host;
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            pending_req_addr <= 32'h0;
            pending_req_wdata <= 32'h0;
            pending_req_we <= 1'b0;
            pending_req_size <= 2'b00;
            pending_req_valid <= 1'b0;
            pending_route_host <= 1'b0;
            waiting_for_resp <= 1'b0;
            pending_resp_rdata <= 32'h0;
            pending_resp_valid <= 1'b0;

            cpu_mem_a_ready <= 1'b1;
            cpu_mem_d_rdata <= 32'h0;
            cpu_mem_d_valid <= 1'b0;

            sys_mem_a_addr <= 32'h0;
            sys_mem_a_wdata <= 32'h0;
            sys_mem_a_we <= 1'b0;
            sys_mem_a_size <= 2'b00;
            sys_mem_a_valid <= 1'b0;
            sys_mem_d_ready <= 1'b0;

            host_mem_a_addr <= 32'h0;
            host_mem_a_wdata <= 32'h0;
            host_mem_a_we <= 1'b0;
            host_mem_a_size <= 2'b00;
            host_mem_a_valid <= 1'b0;
            host_mem_d_ready <= 1'b0;
        end else begin
            pending_req_addr <= next_pending_req_addr;
            pending_req_wdata <= next_pending_req_wdata;
            pending_req_we <= next_pending_req_we;
            pending_req_size <= next_pending_req_size;
            pending_req_valid <= next_pending_req_valid;
            pending_route_host <= next_pending_route_host;
            waiting_for_resp <= next_waiting_for_resp;
            pending_resp_rdata <= next_pending_resp_rdata;
            pending_resp_valid <= next_pending_resp_valid;

            cpu_mem_a_ready <= next_cpu_mem_a_ready;
            cpu_mem_d_rdata <= next_cpu_mem_d_rdata;
            cpu_mem_d_valid <= next_cpu_mem_d_valid;

            sys_mem_a_addr <= next_sys_mem_a_addr;
            sys_mem_a_wdata <= next_sys_mem_a_wdata;
            sys_mem_a_we <= next_sys_mem_a_we;
            sys_mem_a_size <= next_sys_mem_a_size;
            sys_mem_a_valid <= next_sys_mem_a_valid;
            sys_mem_d_ready <= next_sys_mem_d_ready;

            host_mem_a_addr <= next_host_mem_a_addr;
            host_mem_a_wdata <= next_host_mem_a_wdata;
            host_mem_a_we <= next_host_mem_a_we;
            host_mem_a_size <= next_host_mem_a_size;
            host_mem_a_valid <= next_host_mem_a_valid;
            host_mem_d_ready <= next_host_mem_d_ready;
        end
    end
endmodule

`default_nettype none
// Host Bus Mux
// Routes CPU memory transactions to either:
// - System bus path (RTL peripherals, addr[31] == 0)
// - Host bus interface path (Rust/external memory, addr[31] == 1)
//
// Uses the same decoupled address/data channel structure as the CPU memory
// interface. Internally, one request and one response can be buffered.

module host_bus_mux (
    input wire logic        clk,
    input wire logic        rst_n,

    // CPU-side interface
    input wire logic [31:0] cpu_mem_a_addr,
    input wire logic [31:0] cpu_mem_a_wdata,
    input wire logic        cpu_mem_a_we,
    input wire logic [1:0]  cpu_mem_a_size,
    input wire logic        cpu_mem_a_valid,
    output logic        cpu_mem_a_ready,

    output logic [31:0] cpu_mem_d_rdata,
    output logic        cpu_mem_d_valid,
    input wire logic        cpu_mem_d_ready,

    // System bus path (RTL peripherals)
    output logic [31:0] sys_mem_a_addr,
    output logic [31:0] sys_mem_a_wdata,
    output logic        sys_mem_a_we,
    output logic [1:0]  sys_mem_a_size,
    output logic        sys_mem_a_valid,
    input wire logic        sys_mem_a_ready,

    input wire logic [31:0] sys_mem_d_rdata,
    input wire logic        sys_mem_d_valid,
    output logic        sys_mem_d_ready,

    // Host bus interface path (external memory)
    output logic [31:0] host_mem_a_addr,
    output logic [31:0] host_mem_a_wdata,
    output logic        host_mem_a_we,
    output logic [1:0]  host_mem_a_size,
    output logic        host_mem_a_valid,
    input wire logic        host_mem_a_ready,

    input wire logic [31:0] host_mem_d_rdata,
    input wire logic        host_mem_d_valid,
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

    assign cpu_mem_a_ready = !pending_req_valid && !waiting_for_resp && !pending_resp_valid;
    assign cpu_mem_d_rdata = pending_resp_rdata;
    assign cpu_mem_d_valid = pending_resp_valid;

    always_comb begin
        sys_mem_a_addr = pending_req_addr;
        sys_mem_a_wdata = pending_req_wdata;
        sys_mem_a_we = pending_req_we;
        sys_mem_a_size = pending_req_size;
        sys_mem_a_valid = 1'b0;
        sys_mem_d_ready = 1'b0;

        host_mem_a_addr = pending_req_addr;
        host_mem_a_wdata = pending_req_wdata;
        host_mem_a_we = pending_req_we;
        host_mem_a_size = pending_req_size;
        host_mem_a_valid = 1'b0;
        host_mem_d_ready = 1'b0;

        if (pending_req_valid) begin
            if (pending_req_addr[31]) begin
                host_mem_a_valid = 1'b1;
            end else begin
                sys_mem_a_valid = 1'b1;
            end
        end

        if (waiting_for_resp && !pending_resp_valid) begin
            if (pending_route_host) begin
                host_mem_d_ready = 1'b1;
            end else begin
                sys_mem_d_ready = 1'b1;
            end
        end
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
        end else begin
            if (cpu_a_handshake) begin
                pending_req_addr <= cpu_mem_a_addr;
                pending_req_wdata <= cpu_mem_a_wdata;
                pending_req_we <= cpu_mem_a_we;
                pending_req_size <= cpu_mem_a_size;
                pending_req_valid <= 1'b1;
            end

            if (sys_a_handshake) begin
                pending_req_valid <= 1'b0;
                pending_route_host <= 1'b0;
                waiting_for_resp <= 1'b1;
            end else if (host_a_handshake) begin
                pending_req_valid <= 1'b0;
                pending_route_host <= 1'b1;
                waiting_for_resp <= 1'b1;
            end

            if (sys_d_handshake) begin
                pending_resp_rdata <= sys_mem_d_rdata;
                pending_resp_valid <= 1'b1;
                waiting_for_resp <= 1'b0;
            end else if (host_d_handshake) begin
                pending_resp_rdata <= host_mem_d_rdata;
                pending_resp_valid <= 1'b1;
                waiting_for_resp <= 1'b0;
            end

            if (cpu_d_handshake) begin
                pending_resp_valid <= 1'b0;
            end
        end
    end
endmodule
`default_nettype wire

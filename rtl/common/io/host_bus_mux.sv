// Host Bus Mux
// Routes CPU memory transactions to either:
// - System bus path (RTL peripherals)
// - Host bus interface path (external memory / Rust peripherals)

module host_bus_mux #(
    parameter logic [31:0] RTL_PERIPH_BASE  = 32'h50000000,
    parameter logic [31:0] RTL_PERIPH_LIMIT = 32'h60000000
) (
    input  logic        clk,
    input  logic        rst_n,

    // CPU-side interface (A/D channels)
    input  logic [31:0] cpu_mem_a_addr,
    input  logic [31:0] cpu_mem_a_wdata,
    input  logic        cpu_mem_a_we,
    input  logic [1:0]  cpu_mem_a_size,
    input  logic        cpu_mem_a_valid,
    output logic        cpu_mem_a_ready,
    output logic [31:0] cpu_mem_d_rdata,
    output logic        cpu_mem_d_valid,
    input  logic        cpu_mem_d_ready,

    // System bus path (RTL peripherals, A/D channels)
    output logic [31:0] sys_mem_a_addr,
    output logic [31:0] sys_mem_a_wdata,
    output logic        sys_mem_a_we,
    output logic [1:0]  sys_mem_a_size,
    output logic        sys_mem_a_valid,
    input  logic        sys_mem_a_ready,
    input  logic [31:0] sys_mem_d_rdata,
    input  logic        sys_mem_d_valid,
    output logic        sys_mem_d_ready,

    // Host bus interface path (external memory, A/D channels)
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
    logic sel_rtl_periph;
    logic pending_sel_rtl_periph;
    logic pending_req_valid;
    logic mem_a_handshake;
    logic mem_d_handshake;

    assign sel_rtl_periph = (cpu_mem_a_addr >= RTL_PERIPH_BASE) && (cpu_mem_a_addr < RTL_PERIPH_LIMIT);
    assign mem_a_handshake = cpu_mem_a_valid && cpu_mem_a_ready;
    assign mem_d_handshake = cpu_mem_d_valid && cpu_mem_d_ready;

    assign sys_mem_a_addr  = cpu_mem_a_addr;
    assign sys_mem_a_wdata = cpu_mem_a_wdata;
    assign sys_mem_a_we    = cpu_mem_a_we;
    assign sys_mem_a_size  = cpu_mem_a_size;
    assign sys_mem_a_valid = cpu_mem_a_valid && sel_rtl_periph;

    assign host_mem_a_addr  = cpu_mem_a_addr;
    assign host_mem_a_wdata = cpu_mem_a_wdata;
    assign host_mem_a_we    = cpu_mem_a_we;
    assign host_mem_a_size  = cpu_mem_a_size;
    assign host_mem_a_valid = cpu_mem_a_valid && !sel_rtl_periph;

    assign cpu_mem_a_ready = !pending_req_valid && (sel_rtl_periph ? sys_mem_a_ready : host_mem_a_ready);

    assign cpu_mem_d_valid = pending_req_valid
        ? (pending_sel_rtl_periph ? sys_mem_d_valid : host_mem_d_valid)
        : 1'b0;
    assign cpu_mem_d_rdata = pending_sel_rtl_periph ? sys_mem_d_rdata : host_mem_d_rdata;
    assign sys_mem_d_ready = cpu_mem_d_ready && pending_req_valid && pending_sel_rtl_periph;
    assign host_mem_d_ready = cpu_mem_d_ready && pending_req_valid && !pending_sel_rtl_periph;

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pending_sel_rtl_periph <= 1'b0;
            pending_req_valid <= 1'b0;
        end else begin
            if (mem_d_handshake) begin
                pending_req_valid <= 1'b0;
            end
            if (mem_a_handshake) begin
                pending_sel_rtl_periph <= sel_rtl_periph;
                pending_req_valid <= 1'b1;
            end
        end
    end
endmodule

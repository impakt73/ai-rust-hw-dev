// Host Bus Mux
// Routes CPU memory transactions to either:
// - System bus path (RTL peripherals selected by top nibble)
// - Host bus interface path (external memory / Rust peripherals)

module host_bus_mux #(
    // NOTE: These nibble assignments must stay in sync with rtl/common/memory/bus.sv.
    parameter logic [3:0] SYSCTRL_TOP_NIBBLE = 4'h2,
    parameter logic [3:0] LED_TOP_NIBBLE     = 4'h5,
    parameter logic [3:0] CLOCK_TOP_NIBBLE   = 4'h6,
    parameter logic [3:0] SRAM_TOP_NIBBLE    = 4'h7
) (
    // CPU-side interface
    input  logic [31:0] cpu_addr,
    input  logic [31:0] cpu_wdata,
    output logic [31:0] cpu_rdata,
    input  logic        cpu_we,
    input  logic [1:0]  cpu_size,
    input  logic        cpu_req,
    output logic        cpu_ready,

    // System bus path (RTL peripherals)
    output logic [31:0] sys_addr,
    output logic [31:0] sys_wdata,
    input  logic [31:0] sys_rdata,
    output logic        sys_we,
    output logic [1:0]  sys_size,
    output logic        sys_req,
    input  logic        sys_ready,

    // Host bus interface path (external memory)
    output logic [31:0] host_addr,
    output logic [31:0] host_wdata,
    input  logic [31:0] host_rdata,
    output logic        host_we,
    output logic [1:0]  host_size,
    output logic        host_req,
    input  logic        host_ready
);
    logic sel_rtl_periph;

    assign sel_rtl_periph =
        (cpu_addr[31:28] == SYSCTRL_TOP_NIBBLE) ||
        (cpu_addr[31:28] == LED_TOP_NIBBLE) ||
        (cpu_addr[31:28] == CLOCK_TOP_NIBBLE) ||
        (cpu_addr[31:28] == SRAM_TOP_NIBBLE);

    assign sys_addr  = cpu_addr;
    assign sys_wdata = cpu_wdata;
    assign sys_we    = cpu_we;
    assign sys_size  = cpu_size;
    assign sys_req   = cpu_req && sel_rtl_periph;

    assign host_addr  = cpu_addr;
    assign host_wdata = cpu_wdata;
    assign host_we    = cpu_we;
    assign host_size  = cpu_size;
    assign host_req   = cpu_req && !sel_rtl_periph;

    assign cpu_rdata = sel_rtl_periph ? sys_rdata : host_rdata;
    assign cpu_ready = sel_rtl_periph ? sys_ready : host_ready;
endmodule

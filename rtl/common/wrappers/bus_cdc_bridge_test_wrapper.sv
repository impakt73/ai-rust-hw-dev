`default_nettype none
module bus_cdc_bridge_test_wrapper (
    input wire logic        sys_clk,
    input wire logic        periph_clk,
    input wire logic        rst,

    input wire logic [31:0] sys_mem_a_addr,
    input wire logic [31:0] sys_mem_a_wdata,
    input wire logic        sys_mem_a_we,
    input wire logic [1:0]  sys_mem_a_size,
    input wire logic        sys_mem_a_valid,
    output logic            sys_mem_a_ready,

    output logic [31:0] sys_mem_d_rdata,
    output logic        sys_mem_d_valid,
    input wire logic        sys_mem_d_ready,

    output logic [31:0] periph_mem_a_addr,
    output logic [31:0] periph_mem_a_wdata,
    output logic        periph_mem_a_we,
    output logic [1:0]  periph_mem_a_size,
    output logic        periph_mem_a_valid,
    input wire logic        periph_mem_a_ready,

    input wire logic [31:0] periph_mem_d_rdata,
    input wire logic        periph_mem_d_valid,
    output logic            periph_mem_d_ready
);

    bus_cdc_bridge #(
        .ADDR_WIDTH(32),
        .DATA_WIDTH(32),
        .SIZE_WIDTH(2),
        .SYNC_STAGES(2)
    ) u_bus_cdc_bridge (
        .sys_clk(sys_clk),
        .periph_clk(periph_clk),
        .sys_rst(rst),
        .periph_rst(rst),
        .sys_mem_a_addr(sys_mem_a_addr),
        .sys_mem_a_wdata(sys_mem_a_wdata),
        .sys_mem_a_we(sys_mem_a_we),
        .sys_mem_a_size(sys_mem_a_size),
        .sys_mem_a_valid(sys_mem_a_valid),
        .sys_mem_a_ready(sys_mem_a_ready),
        .sys_mem_d_rdata(sys_mem_d_rdata),
        .sys_mem_d_valid(sys_mem_d_valid),
        .sys_mem_d_ready(sys_mem_d_ready),
        .periph_mem_a_addr(periph_mem_a_addr),
        .periph_mem_a_wdata(periph_mem_a_wdata),
        .periph_mem_a_we(periph_mem_a_we),
        .periph_mem_a_size(periph_mem_a_size),
        .periph_mem_a_valid(periph_mem_a_valid),
        .periph_mem_a_ready(periph_mem_a_ready),
        .periph_mem_d_rdata(periph_mem_d_rdata),
        .periph_mem_d_valid(periph_mem_d_valid),
        .periph_mem_d_ready(periph_mem_d_ready)
    );

endmodule
`default_nettype wire

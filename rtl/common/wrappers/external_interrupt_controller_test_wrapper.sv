`default_nettype none

module external_interrupt_controller_test_wrapper (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [3:0]  irq_sources,
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready,
    output logic             meip
);

    external_interrupt_controller #(
        .NUM_SOURCES(4)
    ) u_external_interrupt_controller (
        .clk(clk),
        .rst(rst),
        .irq_sources(irq_sources),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready),
        .meip(meip)
    );

endmodule

`default_nettype wire

module sram_peripheral_test_wrapper (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] mem_a_addr,
    input  logic [31:0] mem_a_wdata,
    input  logic        mem_a_we,
    input  logic [1:0]  mem_a_size,
    input  logic        mem_a_valid,
    output logic        mem_a_ready,
    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input  logic        mem_d_ready
);
    sram_peripheral u_sram_peripheral (
        .clk(clk),
        .rst_n(rst_n),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready)
    );
endmodule

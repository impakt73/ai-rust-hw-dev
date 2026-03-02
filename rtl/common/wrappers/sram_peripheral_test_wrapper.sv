module sram_peripheral_test_wrapper (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic        req,
    input  logic [1:0]  size,
    output logic        ready
);
    sram_peripheral u_sram_peripheral (
        .clk(clk),
        .rst_n(rst_n),
        .addr(addr),
        .wdata(wdata),
        .rdata(rdata),
        .we(we),
        .req(req),
        .size(size),
        .ready(ready)
    );
endmodule

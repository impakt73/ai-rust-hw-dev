`default_nettype none
module sram_test_wrapper (
    input wire logic        clk,
    input wire logic        we,
    input wire logic [3:0]  wmask,
    input wire logic [3:0]  waddr,
    input wire logic [31:0] wdata,
    input wire logic [3:0]  raddr,
    output logic [31:0] rdata
);
    sram #(
        .ADDR_WIDTH(4)
    ) u_sram (
        .clk(clk),
        .we(we),
        .wmask(wmask),
        .waddr(waddr),
        .wdata(wdata),
        .raddr(raddr),
        .rdata(rdata)
    );
endmodule
`default_nettype wire

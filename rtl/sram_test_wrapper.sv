module sram_test_wrapper (
    input  logic        clk,
    input  logic        we,
    input  logic [3:0]  wmask,
    input  logic [3:0]  waddr,
    input  logic [31:0] wdata,
    input  logic [3:0]  raddr,
    output logic [31:0] rdata,
    input  logic [3:0]  raddr2,
    output logic [31:0] rdata2
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
        .rdata(rdata),
        .raddr2(raddr2),
        .rdata2(rdata2)
    );
endmodule

`default_nettype none
module async_fifo_test_wrapper (
    input wire logic       wr_clk,
    input wire logic       rd_clk,
    input wire logic       rst,
    input wire logic       wr_valid,
    output logic       wr_ready,
    input wire logic [7:0] wdata,
    output logic       rd_valid,
    input wire logic       rd_ready,
    output logic [7:0] rdata,
    output logic [2:0] count
);

    async_fifo #(
        .WIDTH(8),
        .DEPTH(4),
        .SYNC_STAGES(2)
    ) u_async_fifo (
        .wr_clk(wr_clk),
        .rd_clk(rd_clk),
        .rst(rst),
        .wr_valid(wr_valid),
        .wr_ready(wr_ready),
        .wdata(wdata),
        .rd_valid(rd_valid),
        .rd_ready(rd_ready),
        .rdata(rdata),
        .count(count)
    );

endmodule
`default_nettype wire

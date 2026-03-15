`default_nettype none
module sync_fifo_test_wrapper (
    input wire logic       clk,
    input wire logic       rst,
    input wire logic       wr_valid,
    output logic       wr_ready,
    input wire logic [7:0] wdata,
    output logic       rd_valid,
    input wire logic       rd_ready,
    output logic [7:0] rdata,
    output logic [2:0] count
);

    sync_fifo #(
        .WIDTH(8),
        .DEPTH(4)
    ) u_sync_fifo (
        .clk(clk),
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

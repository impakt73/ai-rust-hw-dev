module sync_fifo_test_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       wr_en,
    input  logic [7:0] wdata,
    input  logic       rd_en,
    output logic [7:0] rdata,
    output logic       full,
    output logic       empty,
    output logic [2:0] count
);

    sync_fifo #(
        .WIDTH(8),
        .DEPTH(4)
    ) u_sync_fifo (
        .clk(clk),
        .rst_n(rst_n),
        .wr_en(wr_en),
        .wdata(wdata),
        .rd_en(rd_en),
        .rdata(rdata),
        .full(full),
        .empty(empty),
        .count(count)
    );

endmodule

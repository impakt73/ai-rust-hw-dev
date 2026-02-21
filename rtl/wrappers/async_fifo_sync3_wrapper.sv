module async_fifo_sync3_wrapper (
    input  logic       wr_clk,
    input  logic       rd_clk,
    input  logic       rst_n,
    input  logic       wr_en,
    input  logic [7:0] wdata,
    input  logic       rd_en,
    output logic [7:0] rdata,
    output logic       full,
    output logic       empty,
    output logic [2:0] count
);

    async_fifo #(
        .WIDTH(8),
        .DEPTH(4),
        .SYNC_STAGES(3)
    ) u_async_fifo (
        .wr_clk(wr_clk),
        .rd_clk(rd_clk),
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

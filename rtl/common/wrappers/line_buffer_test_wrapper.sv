`default_nettype none
module line_buffer_test_wrapper (
    input  wire logic        wr_clk,
    input  wire logic        wr_rst,
    input  wire logic [7:0]  wr_data,
    input  wire logic        wr_valid,
    output      logic        wr_ready,
    input  wire logic        wr_eol,
    input  wire logic        wr_sof,

    input  wire logic        rd_clk,
    input  wire logic        rd_rst,
    output      logic [7:0]  rd_data,
    output      logic        rd_valid,
    input  wire logic        rd_ready,
    output      logic        rd_eol,
    output      logic        rd_sof
);

    line_buffer #(
        .PIXEL_WIDTH   (8),
        .MAX_LINE_WIDTH(16),
        .SYNC_STAGES   (2)
    ) u_line_buffer (
        .wr_clk  (wr_clk),
        .wr_rst  (wr_rst),
        .wr_data (wr_data),
        .wr_valid(wr_valid),
        .wr_ready(wr_ready),
        .wr_eol  (wr_eol),
        .wr_sof  (wr_sof),
        .rd_clk  (rd_clk),
        .rd_rst  (rd_rst),
        .rd_data (rd_data),
        .rd_valid(rd_valid),
        .rd_ready(rd_ready),
        .rd_eol  (rd_eol),
        .rd_sof  (rd_sof)
    );

endmodule
`default_nettype wire

`default_nettype none
module line_buffer_test_wrapper (
    input  wire logic       wr_clk,
    input  wire logic       rd_clk,
    input  wire logic       rst,
    input  wire logic       start_of_frame,
    input  wire logic       wr_valid,
    output logic            wr_ready,
    input  wire logic [7:0] wr_pixel,
    input  wire logic       wr_eol,
    output logic            rd_valid,
    input  wire logic       rd_ready,
    output logic [7:0]      rd_pixel,
    output logic            rd_eol
);

    line_buffer #(
        .PIXEL_WIDTH(8),
        .MAX_LINE_PIXELS(8),
        .SYNC_STAGES(2)
    ) u_line_buffer (
        .wr_clk(wr_clk),
        .rd_clk(rd_clk),
        .rst(rst),
        .start_of_frame(start_of_frame),
        .wr_valid(wr_valid),
        .wr_ready(wr_ready),
        .wr_pixel(wr_pixel),
        .wr_eol(wr_eol),
        .rd_valid(rd_valid),
        .rd_ready(rd_ready),
        .rd_pixel(rd_pixel),
        .rd_eol(rd_eol)
    );

endmodule
`default_nettype wire

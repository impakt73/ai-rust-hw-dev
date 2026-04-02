`default_nettype none
module line_buffer_test_wrapper (
    input  wire logic       wr_clk,
    input  wire logic       rd_clk,
    input  wire logic       rst,
    input  wire logic       wr_sof,
    input  wire logic       rd_sof,
    input  wire logic       wr_en,
    input  wire logic [7:0] wdata,
    output logic            wr_ready,
    output logic            wr_eol,
    output logic            wr_bank,
    input  wire logic       rd_en,
    output logic            rd_ready,
    output logic            rd_valid,
    output logic [7:0]      rdata,
    output logic            rd_eol,
    output logic            rd_bank
);

    dual_clock_line_buffer #(
        .PIXEL_WIDTH(8),
        .LINE_PIXELS(4),
        .SYNC_STAGES(2)
    ) u_line_buffer (
        .wr_clk(wr_clk),
        .rd_clk(rd_clk),
        .rst(rst),
        .wr_sof(wr_sof),
        .rd_sof(rd_sof),
        .wr_en(wr_en),
        .wdata(wdata),
        .wr_ready(wr_ready),
        .wr_eol(wr_eol),
        .wr_bank(wr_bank),
        .rd_en(rd_en),
        .rd_ready(rd_ready),
        .rd_valid(rd_valid),
        .rdata(rdata),
        .rd_eol(rd_eol),
        .rd_bank(rd_bank)
    );

endmodule

module line_buffer_nonpow2_test_wrapper (
    input  wire logic       wr_clk,
    input  wire logic       rd_clk,
    input  wire logic       rst,
    input  wire logic       wr_sof,
    input  wire logic       rd_sof,
    input  wire logic       wr_en,
    input  wire logic [7:0] wdata,
    output logic            wr_ready,
    output logic            wr_eol,
    output logic            wr_bank,
    input  wire logic       rd_en,
    output logic            rd_ready,
    output logic            rd_valid,
    output logic [7:0]      rdata,
    output logic            rd_eol,
    output logic            rd_bank
);

    dual_clock_line_buffer #(
        .PIXEL_WIDTH(8),
        .LINE_PIXELS(3),
        .SYNC_STAGES(2)
    ) u_line_buffer (
        .wr_clk(wr_clk),
        .rd_clk(rd_clk),
        .rst(rst),
        .wr_sof(wr_sof),
        .rd_sof(rd_sof),
        .wr_en(wr_en),
        .wdata(wdata),
        .wr_ready(wr_ready),
        .wr_eol(wr_eol),
        .wr_bank(wr_bank),
        .rd_en(rd_en),
        .rd_ready(rd_ready),
        .rd_valid(rd_valid),
        .rdata(rdata),
        .rd_eol(rd_eol),
        .rd_bank(rd_bank)
    );

endmodule
`default_nettype wire

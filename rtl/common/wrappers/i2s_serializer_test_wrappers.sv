`default_nettype none
module i2s_serializer_equal_width_wrapper (
    input wire logic       clk,
    input wire logic       rst_n,
    input wire logic [7:0] sample_data,
    input wire logic       sample_valid,
    output logic       sample_ready,
    output logic       i2s_bclk,
    output logic       i2s_lrclk,
    output logic       i2s_sd
);
    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH(8),
        .OUTPUT_SAMPLE_WIDTH(8)
    ) dut (
        .clk(clk),
        .rst_n(rst_n),
        .sample_data(sample_data),
        .sample_valid(sample_valid),
        .sample_ready(sample_ready),
        .i2s_bclk(i2s_bclk),
        .i2s_lrclk(i2s_lrclk),
        .i2s_sd(i2s_sd)
    );
endmodule

module i2s_serializer_expand_wrapper (
    input wire logic       clk,
    input wire logic       rst_n,
    input wire logic [7:0] sample_data,
    input wire logic       sample_valid,
    output logic       sample_ready,
    output logic       i2s_bclk,
    output logic       i2s_lrclk,
    output logic       i2s_sd
);
    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH(8),
        .OUTPUT_SAMPLE_WIDTH(12)
    ) dut (
        .clk(clk),
        .rst_n(rst_n),
        .sample_data(sample_data),
        .sample_valid(sample_valid),
        .sample_ready(sample_ready),
        .i2s_bclk(i2s_bclk),
        .i2s_lrclk(i2s_lrclk),
        .i2s_sd(i2s_sd)
    );
endmodule

module i2s_serializer_truncate_wrapper (
    input wire logic        clk,
    input wire logic        rst_n,
    input wire logic [11:0] sample_data,
    input wire logic        sample_valid,
    output logic        sample_ready,
    output logic        i2s_bclk,
    output logic        i2s_lrclk,
    output logic        i2s_sd
);
    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH(12),
        .OUTPUT_SAMPLE_WIDTH(8)
    ) dut (
        .clk(clk),
        .rst_n(rst_n),
        .sample_data(sample_data),
        .sample_valid(sample_valid),
        .sample_ready(sample_ready),
        .i2s_bclk(i2s_bclk),
        .i2s_lrclk(i2s_lrclk),
        .i2s_sd(i2s_sd)
    );
endmodule
`default_nettype wire

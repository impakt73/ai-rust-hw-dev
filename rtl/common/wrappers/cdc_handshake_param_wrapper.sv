`default_nettype none
module cdc_handshake_param_wrapper (
    input wire logic        src_clk,
    input wire logic        dst_clk,
    input wire logic        rst,
    input wire logic        src_valid,
    output logic            src_ready,
    input wire logic [15:0] src_data,
    output logic            dst_valid,
    input wire logic        dst_ready,
    output logic [15:0]     dst_data
);

    cdc_handshake #(
        .WIDTH(16),
        .SYNC_STAGES(3)
    ) u_cdc_handshake (
        .src_clk(src_clk),
        .dst_clk(dst_clk),
        .rst(rst),
        .src_valid(src_valid),
        .src_ready(src_ready),
        .src_data(src_data),
        .dst_valid(dst_valid),
        .dst_ready(dst_ready),
        .dst_data(dst_data)
    );

endmodule
`default_nettype wire

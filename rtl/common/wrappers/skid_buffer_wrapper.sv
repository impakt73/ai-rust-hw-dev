`default_nettype none
module skid_buffer_wrapper (
    input wire logic       clk,
    input wire logic       rst,
    input wire logic       in_valid,
    input wire logic [7:0] in_data,
    output logic       in_ready,
    output logic       out_valid,
    output logic [7:0] out_data,
    input wire logic       out_ready
);

    skid_buffer #(
        .WIDTH(8)
    ) u_skid_buffer (
        .clk(clk),
        .rst(rst),
        .in_valid(in_valid),
        .in_data(in_data),
        .in_ready(in_ready),
        .out_valid(out_valid),
        .out_data(out_data),
        .out_ready(out_ready)
    );

endmodule
`default_nettype wire

module skid_buffer_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       in_valid,
    input  logic [7:0] in_data,
    output logic       in_ready,
    output logic       out_valid,
    output logic [7:0] out_data,
    input  logic       out_ready
);

    skid_buffer #(
        .WIDTH(8)
    ) u_skid_buffer (
        .clk(clk),
        .rst_n(rst_n),
        .in_valid(in_valid),
        .in_data(in_data),
        .in_ready(in_ready),
        .out_valid(out_valid),
        .out_data(out_data),
        .out_ready(out_ready)
    );

endmodule

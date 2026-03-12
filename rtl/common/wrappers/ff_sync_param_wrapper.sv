`default_nettype none
module ff_sync_param_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic [3:0] din,
    output logic [3:0] dout
);

    ff_sync #(
        .STAGES(2),
        .WIDTH(4)
    ) u_ff_sync (
        .clk(clk),
        .rst_n(rst_n),
        .din(din),
        .dout(dout)
    );

endmodule
`default_nettype wire

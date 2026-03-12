`default_nettype none
module ff_sync_default_wrapper (
    input  logic clk,
    input  logic rst_n,
    input  logic din,
    output logic dout
);

    ff_sync u_ff_sync (
        .clk(clk),
        .rst_n(rst_n),
        .din(din),
        .dout(dout)
    );

endmodule
`default_nettype wire

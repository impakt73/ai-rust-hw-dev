`default_nettype none
module ff_sync_default_wrapper (
    input wire logic clk,
    input wire logic rst,
    input wire logic din,
    output logic dout
);

    ff_sync u_ff_sync (
        .clk(clk),
        .rst(rst),
        .din(din),
        .dout(dout)
    );

endmodule
`default_nettype wire

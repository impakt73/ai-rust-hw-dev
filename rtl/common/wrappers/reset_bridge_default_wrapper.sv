`default_nettype none
module reset_bridge_default_wrapper (
    input wire logic clk,
    input wire logic rst_n,
    output logic     rst
);

    reset_bridge u_reset_bridge (
        .clk(clk),
        .rst_n(rst_n),
        .rst(rst)
    );

endmodule
`default_nettype wire

`default_nettype none
module debouncer_single_cycle_wrapper (
    input wire logic clk,
    input wire logic rst,
    input wire logic noisy_in,
    output logic debounced_out
);

    debouncer #(
        .CLK_FREQ_HZ(1_000_000),
        .STABLE_TIME_US(1)
    ) u_debouncer (
        .clk(clk),
        .rst(rst),
        .din(noisy_in),
        .dout(debounced_out)
    );

endmodule
`default_nettype wire

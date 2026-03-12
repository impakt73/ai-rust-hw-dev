module debouncer_wrapper (
    input  logic clk,
    input  logic rst_n,
    input  logic noisy_in,
    output logic debounced_out
);

    debouncer #(
        .CLK_FREQ_HZ(1_000_000),
        .STABLE_TIME_US(3)
    ) u_debouncer (
        .clk(clk),
        .rst_n(rst_n),
        .din(noisy_in),
        .dout(debounced_out)
    );

endmodule

`default_nettype none
module activity_indicator_wrapper (
    input wire logic clk,
    input wire logic rst_n,
    input wire logic activity,
    output logic indicator
);

    activity_indicator #(
        .CLK_FREQ_HZ(100),
        .INDICATOR_FREQ_MILLIHERTZ(25_000)
    ) u_activity_indicator (
        .clk(clk),
        .rst_n(rst_n),
        .activity(activity),
        .indicator(indicator)
    );

endmodule
`default_nettype wire

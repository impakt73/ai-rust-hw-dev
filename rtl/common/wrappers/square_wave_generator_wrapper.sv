`default_nettype none
module square_wave_generator_wrapper (
    input wire logic clk,
    input wire logic rst_n,
    output logic square_wave
);

    square_wave_generator #(
        .CLK_FREQ_HZ(100),
        .WAVE_FREQ_MILLIHERTZ(25_000)
    ) u_square_wave_generator (
        .clk(clk),
        .rst_n(rst_n),
        .square_wave(square_wave)
    );

endmodule
`default_nettype wire

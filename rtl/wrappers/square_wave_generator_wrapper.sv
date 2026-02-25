module square_wave_generator_wrapper (
    input  logic clk,
    input  logic rst_n,
    output logic square_wave
);

    square_wave_generator #(
        .CLK_FREQ_HZ(100),
        .SQUARE_WAVE_FREQ_HZ(5)
    ) u_square_wave_generator (
        .clk(clk),
        .rst_n(rst_n),
        .square_wave(square_wave)
    );

endmodule

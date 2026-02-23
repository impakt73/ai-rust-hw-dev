module phase_accumulator_wrapper (
    input  logic clk,
    input  logic rst_n,
    output logic tick
);

    phase_accumulator #(
        .PHASE_WIDTH(16),
        .CLK_FREQ_HZ(100),
        .TICK_FREQ_HZ(33)
    ) u_phase_accumulator (
        .clk(clk),
        .rst_n(rst_n),
        .tick(tick)
    );

endmodule

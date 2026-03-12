`default_nettype none
// Square Wave Generator
// Generates a registered square wave by toggling output after a configurable
// half-period count.
//
// Parameters:
//   CLK_FREQ_HZ          - Input clock frequency in Hz
//   WAVE_FREQ_MILLIHERTZ - Desired square wave frequency in millihertz
//
// Interface:
//   clk         - System clock
//   rst_n       - Synchronous active-low reset
//   square_wave - Registered square wave output

module square_wave_generator #(
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned WAVE_FREQ_MILLIHERTZ = 1_000
) (
    input  logic clk,
    input  logic rst_n,
    output logic square_wave
);

    localparam longint unsigned HALF_PERIOD_CYCLES =
        (64'(CLK_FREQ_HZ) * 64'd1000 + 64'(WAVE_FREQ_MILLIHERTZ)) / (64'd2 * 64'(WAVE_FREQ_MILLIHERTZ));
    localparam int unsigned COUNTER_WIDTH = (HALF_PERIOD_CYCLES <= 1) ? 1 : $clog2(HALF_PERIOD_CYCLES);
    localparam logic [COUNTER_WIDTH-1:0] HALF_PERIOD_COUNT_MAX = COUNTER_WIDTH'(HALF_PERIOD_CYCLES - 1);

    logic [COUNTER_WIDTH-1:0] counter;

    // Parameter validation (simulation only)
    initial begin
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "square_wave_generator: CLK_FREQ_HZ must be > 0");
        end
        if (WAVE_FREQ_MILLIHERTZ == 0) begin
            $fatal(1, "square_wave_generator: WAVE_FREQ_MILLIHERTZ must be > 0");
        end
        if (64'(WAVE_FREQ_MILLIHERTZ) > (64'(CLK_FREQ_HZ) * 64'd1000) / 2) begin
            $fatal(1, "square_wave_generator: WAVE_FREQ_MILLIHERTZ must be <= CLK_FREQ_HZ*1000/2, got %0d", WAVE_FREQ_MILLIHERTZ);
        end
        if (HALF_PERIOD_CYCLES == 0) begin
            $fatal(1, "square_wave_generator: HALF_PERIOD_CYCLES resolved to 0");
        end
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            counter <= '0;
            square_wave <= 1'b0;
        end else if (counter == HALF_PERIOD_COUNT_MAX) begin
            counter <= '0;
            square_wave <= ~square_wave;
        end else begin
            counter <= counter + 1'b1;
        end
    end

endmodule
`default_nettype wire

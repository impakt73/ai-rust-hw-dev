// Square wave generator
// Uses a counter to toggle output at a parameterized frequency.
//
// Parameters:
//   CLK_FREQ_HZ         - Input clock frequency in Hz
//   SQUARE_WAVE_FREQ_HZ - Output square wave frequency in Hz
//
// Interface:
//   clk         - System clock
//   rst_n       - Asynchronous active-low reset
//   square_wave - Registered square wave output
module square_wave_generator #(
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned SQUARE_WAVE_FREQ_HZ = 1
) (
    input  logic clk,
    input  logic rst_n,
    output logic square_wave
);

    localparam int unsigned HALF_PERIOD_CYCLES = CLK_FREQ_HZ / (2 * SQUARE_WAVE_FREQ_HZ);
    localparam int unsigned COUNTER_WIDTH = (HALF_PERIOD_CYCLES <= 1) ? 1 : $clog2(HALF_PERIOD_CYCLES);
    localparam logic [COUNTER_WIDTH-1:0] TERMINAL_COUNT = COUNTER_WIDTH'(HALF_PERIOD_CYCLES - 1);

    logic [COUNTER_WIDTH-1:0] cycle_count;

    // Parameter validation (simulation only)
    initial begin
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "square_wave_generator: CLK_FREQ_HZ must be > 0");
        end
        if (SQUARE_WAVE_FREQ_HZ == 0) begin
            $fatal(1, "square_wave_generator: SQUARE_WAVE_FREQ_HZ must be > 0");
        end
        if ((2 * SQUARE_WAVE_FREQ_HZ) > CLK_FREQ_HZ) begin
            $fatal(
                1,
                "square_wave_generator: SQUARE_WAVE_FREQ_HZ must be <= CLK_FREQ_HZ/2, got %0d",
                SQUARE_WAVE_FREQ_HZ
            );
        end
        if (HALF_PERIOD_CYCLES == 0) begin
            $fatal(1, "square_wave_generator: HALF_PERIOD_CYCLES resolved to 0");
        end
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cycle_count  <= '0;
            square_wave <= 1'b0;
        end else if (cycle_count == TERMINAL_COUNT) begin
            cycle_count  <= '0;
            square_wave <= ~square_wave;
        end else begin
            cycle_count <= cycle_count + 1'b1;
        end
    end

endmodule

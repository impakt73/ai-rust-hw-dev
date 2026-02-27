// Activity Indicator
// Emits a single registered square-wave cycle when activity is triggered.
//
// Parameters:
//   CLK_FREQ_HZ                 - Input clock frequency in Hz
//   INDICATOR_FREQ_MILLIHERTZ   - Desired indicator square-wave frequency in millihertz
//
// Interface:
//   clk       - System clock
//   rst_n     - Asynchronous active-low reset
//   activity  - High level triggers one indicator cycle when idle
//   indicator - Registered indicator output

module activity_indicator #(
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned INDICATOR_FREQ_MILLIHERTZ = 1_000
) (
    input  logic clk,
    input  logic rst_n,
    input  logic activity,
    output logic indicator
);

    localparam longint unsigned HALF_PERIOD_CYCLES =
        (64'(CLK_FREQ_HZ) * 64'd1000 + 64'(INDICATOR_FREQ_MILLIHERTZ)) / (64'd2 * 64'(INDICATOR_FREQ_MILLIHERTZ));
    localparam int unsigned COUNTER_WIDTH = (HALF_PERIOD_CYCLES <= 1) ? 1 : $clog2(HALF_PERIOD_CYCLES);
    localparam logic [COUNTER_WIDTH-1:0] HALF_PERIOD_COUNT_MAX = COUNTER_WIDTH'(HALF_PERIOD_CYCLES - 1);

    logic [COUNTER_WIDTH-1:0] counter;
    logic pulse_active;
    logic high_half;

    // Parameter validation (simulation only)
    initial begin
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "activity_indicator: CLK_FREQ_HZ must be > 0");
        end
        if (INDICATOR_FREQ_MILLIHERTZ == 0) begin
            $fatal(1, "activity_indicator: INDICATOR_FREQ_MILLIHERTZ must be > 0");
        end
        if (64'(INDICATOR_FREQ_MILLIHERTZ) > (64'(CLK_FREQ_HZ) * 64'd1000) / 2) begin
            $fatal(1, "activity_indicator: INDICATOR_FREQ_MILLIHERTZ must be <= CLK_FREQ_HZ*1000/2, got %0d", INDICATOR_FREQ_MILLIHERTZ);
        end
        if (HALF_PERIOD_CYCLES == 0) begin
            $fatal(1, "activity_indicator: HALF_PERIOD_CYCLES resolved to 0");
        end
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            counter <= '0;
            pulse_active <= 1'b0;
            high_half <= 1'b0;
            indicator <= 1'b0;
        end else begin
            if (!pulse_active) begin
                counter <= '0;
                high_half <= 1'b0;
                indicator <= 1'b0;
                if (activity) begin
                    pulse_active <= 1'b1;
                    high_half <= 1'b1;
                    indicator <= 1'b1;
                end
            end else if (counter == HALF_PERIOD_COUNT_MAX) begin
                counter <= '0;
                if (high_half) begin
                    high_half <= 1'b0;
                    indicator <= 1'b0;
                end else begin
                    pulse_active <= 1'b0;
                    indicator <= 1'b0;
                end
            end else begin
                counter <= counter + 1'b1;
            end
        end
    end

endmodule

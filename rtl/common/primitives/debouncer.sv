`default_nettype none
// Debouncer
// Filters glitchy synchronized inputs and only updates the output after the
// input remains stable for a configurable time window.
//
// Parameters:
//   CLK_FREQ_HZ    - Input clock frequency in Hz
//   STABLE_TIME_US - Time in microseconds that the input must remain stable
//
// Interface:
//   clk  - System clock
//   rst - Synchronous active-high reset
//   din  - Synchronized input signal
//   dout - Debounced output signal

module debouncer #(
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned STABLE_TIME_US = 1_000
) (
    input wire logic clk,
    input wire logic rst,
    input wire logic din,
    output logic dout
);

    localparam longint unsigned STABLE_CYCLES =
        // Add 1_000_000-1 before dividing to round up so the debounce interval
        // never resolves shorter than the requested STABLE_TIME_US.
        ((64'(CLK_FREQ_HZ) * 64'(STABLE_TIME_US)) + 64'd999_999) / 64'd1_000_000;
    // $clog2(1) is 0, but the counter still needs a representable storage bit
    // for the single-cycle debounce case.
    localparam int unsigned COUNTER_WIDTH = (STABLE_CYCLES <= 1) ? 1 : $clog2(STABLE_CYCLES);
    localparam logic [COUNTER_WIDTH-1:0] STABLE_COUNT_MAX = COUNTER_WIDTH'(STABLE_CYCLES - 1);
    localparam logic [COUNTER_WIDTH-1:0] STABLE_COUNT_PRE_MAX =
        (STABLE_CYCLES <= 1) ? '0 : COUNTER_WIDTH'(STABLE_CYCLES - 2);
    localparam bit SINGLE_STABLE_CYCLE = (STABLE_CYCLES == 1);

    logic [COUNTER_WIDTH-1:0] stable_counter;
    logic stable_counter_is_max;

    // Parameter validation (simulation only)
    initial begin
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "debouncer: CLK_FREQ_HZ must be > 0");
        end
        if (STABLE_TIME_US == 0) begin
            $fatal(1, "debouncer: STABLE_TIME_US must be > 0");
        end
        if (STABLE_CYCLES == 0) begin
            $fatal(1, "debouncer: STABLE_CYCLES resolved to 0");
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            stable_counter <= '0;
            stable_counter_is_max <= 1'b0;
            dout <= 1'b0;
        end else begin
            if (din == dout) begin
                stable_counter <= '0;
                stable_counter_is_max <= 1'b0;
            end else if (SINGLE_STABLE_CYCLE || stable_counter_is_max) begin
                stable_counter <= '0;
                stable_counter_is_max <= 1'b0;
                dout <= din;
            end else begin
                stable_counter <= stable_counter + 1'b1;
                stable_counter_is_max <= (stable_counter == STABLE_COUNT_PRE_MAX);
            end
        end
    end

endmodule
`default_nettype wire

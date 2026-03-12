// Debouncer
// Filters glitchy asynchronous inputs and only updates the output after the
// synchronized input remains stable for a configurable time window.
//
// Parameters:
//   CLK_FREQ_HZ    - Input clock frequency in Hz
//   STABLE_TIME_US - Time in microseconds that the input must remain stable
//
// Interface:
//   clk  - System clock
//   rst_n - Synchronous active-low reset
//   din  - Noisy asynchronous input signal
//   dout - Debounced output signal

module debouncer #(
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned STABLE_TIME_US = 1_000
) (
    input  logic clk,
    input  logic rst_n,
    input  logic din,
    output logic dout
);

    localparam longint unsigned STABLE_CYCLES =
        ((64'(CLK_FREQ_HZ) * 64'(STABLE_TIME_US)) + 64'd999_999) / 64'd1_000_000;
    localparam int unsigned COUNTER_WIDTH = (STABLE_CYCLES <= 1) ? 1 : $clog2(STABLE_CYCLES);
    localparam logic [COUNTER_WIDTH-1:0] STABLE_COUNT_MAX = COUNTER_WIDTH'(STABLE_CYCLES - 1);

    logic [1:0] sync_regs;
    logic [COUNTER_WIDTH-1:0] stable_counter;

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
        if (!rst_n) begin
            sync_regs <= '0;
            stable_counter <= '0;
            dout <= 1'b0;
        end else begin
            sync_regs[0] <= din;
            sync_regs[1] <= sync_regs[0];

            if (sync_regs[1] == dout) begin
                stable_counter <= '0;
            end else if (stable_counter == STABLE_COUNT_MAX) begin
                stable_counter <= '0;
                dout <= sync_regs[1];
            end else begin
                stable_counter <= stable_counter + 1'b1;
            end
        end
    end

endmodule

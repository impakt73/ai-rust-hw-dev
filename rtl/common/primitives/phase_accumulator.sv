// Phase Accumulator
// Generates a high-precision tick pulse using fractional phase accumulation.
//
// Parameters:
//   PHASE_WIDTH - Number of bits in the phase accumulator
//   CLK_FREQ_HZ - Input clock frequency in Hz
//   TICK_FREQ_HZ - Desired average tick frequency in Hz
//
// Interface:
//   clk   - System clock
//   rst_n - Asynchronous active-low reset
//   tick  - One-cycle pulse at the configured average frequency

module phase_accumulator #(
    parameter int unsigned PHASE_WIDTH = 32,
    parameter int unsigned CLK_FREQ_HZ = 100_000_000,
    parameter int unsigned TICK_FREQ_HZ = 115_200
) (
    input  logic clk,
    input  logic rst_n,
    output logic tick
);

    localparam longint unsigned PHASE_MODULUS = 64'd1 << PHASE_WIDTH;
    localparam longint unsigned PHASE_INCREMENT =
        ((64'(TICK_FREQ_HZ) * PHASE_MODULUS) + (64'(CLK_FREQ_HZ) / 2)) / 64'(CLK_FREQ_HZ);

    logic [PHASE_WIDTH-1:0] phase_acc;
    logic [PHASE_WIDTH:0] phase_sum;

    // Parameter validation (simulation only)
    initial begin
        if (PHASE_WIDTH == 0 || PHASE_WIDTH > 63) begin
            $fatal(1, "phase_accumulator: PHASE_WIDTH must be in range [1, 63], got %0d", PHASE_WIDTH);
        end
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "phase_accumulator: CLK_FREQ_HZ must be > 0");
        end
        if (TICK_FREQ_HZ == 0 || TICK_FREQ_HZ >= CLK_FREQ_HZ) begin
            $fatal(1, "phase_accumulator: TICK_FREQ_HZ must be in range [1, CLK_FREQ_HZ-1], got %0d", TICK_FREQ_HZ);
        end
        if (PHASE_INCREMENT == 0) begin
            $fatal(1, "phase_accumulator: PHASE_INCREMENT resolved to 0, increase PHASE_WIDTH");
        end
    end

    assign phase_sum = {1'b0, phase_acc} + {1'b0, PHASE_INCREMENT[PHASE_WIDTH-1:0]};

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            phase_acc <= '0;
            tick <= 1'b0;
        end else begin
            phase_acc <= phase_sum[PHASE_WIDTH-1:0];
            tick <= phase_sum[PHASE_WIDTH];
        end
    end

endmodule

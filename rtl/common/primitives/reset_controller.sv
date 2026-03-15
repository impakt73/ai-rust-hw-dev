`default_nettype none
// Reset Controller Module
// Generates a power-on reset signal that remains asserted (high) for a
// configurable number of clock cycles after the input reset is deasserted.
//
// Usage:
//   - Connect rst_in to an active-high reset source (e.g. ~pll_locked)
//   - Use rst_out (active high) directly for downstream modules
//
// Behavior:
//   - When rst_in is high, the counter resets and rst_out is held high (reset asserted)
//   - When rst_in goes low, the counter starts counting
//   - rst_out remains high until the counter reaches RESET_CYCLES
//   - rst_out is registered to avoid timing issues

module reset_controller #(
    parameter RESET_CYCLES = 8  // Number of cycles to hold reset (default: 8)
) (
    input wire logic clk,       // System clock
    input wire logic rst_in,    // Input reset (active high)
    output logic rst_out        // Output reset (active high, registered: 1 = reset asserted)
);

    // Calculate counter width based on RESET_CYCLES parameter
    // Need enough bits to count from 0 up to RESET_CYCLES
    // Minimum width of 1 to handle edge case of RESET_CYCLES = 0 or 1
    localparam COUNTER_WIDTH = (RESET_CYCLES < 2) ? 1 : $clog2(RESET_CYCLES + 1);
    
    // Internal counter to track reset duration
    logic [COUNTER_WIDTH-1:0] counter;
    
    // Reset complete flag - set when counter reaches target
    // Counter counts from 0 to RESET_CYCLES, so reset is held for exactly RESET_CYCLES cycles
    logic reset_complete;
    assign reset_complete = (counter >= COUNTER_WIDTH'(RESET_CYCLES));
    
    // Counter logic
    always_ff @(posedge clk) begin
        if (rst_in) begin
            // Input reset asserted - clear counter
            counter <= '0;
        end else if (!reset_complete) begin
            // Count up until we reach RESET_CYCLES
            counter <= counter + 1'b1;
        end
        // When reset_complete, counter stops (holds value)
    end
    
    // Registered output reset signal
    // Output is high (reset asserted) until counter reaches RESET_CYCLES
    always_ff @(posedge clk) begin
        if (rst_in) begin
            // Input reset asserted - output reset asserted (high)
            rst_out <= 1'b1;
        end else begin
            // Output is de-asserted once reset_complete (low when done)
            rst_out <= ~reset_complete;
        end
    end

endmodule
`default_nettype wire

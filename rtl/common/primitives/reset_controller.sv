// Reset Controller Module
// Generates a power-on reset signal that remains asserted (low) for a
// configurable number of clock cycles after the input reset is deasserted.
//
// Usage:
//   - Connect rst_n_in to PLL lock signal (or other stable reset source)
//   - Use rst_n_out (active low) directly for downstream modules
//
// Behavior:
//   - When rst_n_in is low, the counter resets and rst_n_out is held low (reset asserted)
//   - When rst_n_in goes high, the counter starts counting
//   - rst_n_out remains low until the counter reaches RESET_CYCLES
//   - rst_n_out is registered to avoid timing issues

module reset_controller #(
    parameter RESET_CYCLES = 8  // Number of cycles to hold reset (default: 8)
) (
    input  logic clk,           // System clock
    input  logic rst_n_in,      // Input reset (active low, typically from PLL lock)
    output logic rst_n_out      // Output reset (active low, registered: 0 = reset asserted)
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
        if (!rst_n_in) begin
            // Input reset asserted - clear counter
            counter <= '0;
        end else if (!reset_complete) begin
            // Count up until we reach RESET_CYCLES
            counter <= counter + 1'b1;
        end
        // When reset_complete, counter stops (holds value)
    end
    
    // Registered output reset signal
    // Output is low (reset asserted) until counter reaches RESET_CYCLES
    // Using active-low output to match standard reset conventions
    always_ff @(posedge clk) begin
        if (!rst_n_in) begin
            // Input reset asserted - output reset asserted (low)
            rst_n_out <= 1'b0;
        end else begin
            // Output follows reset_complete (high when reset done)
            rst_n_out <= reset_complete;
        end
    end

endmodule

// Reset Controller Module
// Generates a power-on reset signal that remains asserted (high) for a
// configurable number of clock cycles after the input reset is deasserted.
// Also supports soft reset requests from on-board logic (e.g., CPU).
//
// Usage:
//   - Connect rst_n_in to PLL lock signal (or other stable reset source)
//   - Connect reset_request to CPU/system reset request (active high)
//   - Use rst_out (active high) for downstream modules, or invert for active-low
//
// Behavior:
//   - When rst_n_in is low, the counter resets and rst_out is held high (reset asserted)
//   - When rst_n_in goes high, the counter starts counting
//   - rst_out remains high until the counter reaches RESET_CYCLES
//   - If reset_request is asserted at any time, the counter restarts
//   - rst_out is registered to avoid timing issues

module reset_controller #(
    parameter RESET_CYCLES = 8  // Number of cycles to hold reset (default: 8)
) (
    input  logic clk,           // System clock
    input  logic rst_n_in,      // Input reset (active low, typically from PLL lock)
    input  logic reset_request, // Reset request from on-board logic (active high)
    output logic rst_out        // Output reset (active high, registered: 1 = reset asserted)
);

    // Calculate counter width based on RESET_CYCLES parameter
    // Need enough bits to count from 0 to RESET_CYCLES-1
    // Minimum width of 1 to handle edge case of RESET_CYCLES = 0 or 1
    localparam COUNTER_WIDTH = (RESET_CYCLES <= 1) ? 1 : $clog2(RESET_CYCLES);
    
    // Internal counter to track reset duration
    logic [COUNTER_WIDTH-1:0] counter;
    
    // Reset complete flag - set when counter reaches target
    // Counter counts from 0 to RESET_CYCLES-1, so reset is held for exactly RESET_CYCLES cycles
    logic reset_complete;
    assign reset_complete = (counter >= COUNTER_WIDTH'(RESET_CYCLES - 1));
    
    // Counter logic
    always_ff @(posedge clk or negedge rst_n_in) begin
        if (!rst_n_in) begin
            // Input reset asserted - clear counter
            counter <= '0;
        end else if (reset_request) begin
            // Soft reset requested - restart counter
            counter <= '0;
        end else if (!reset_complete) begin
            // Count up until we reach RESET_CYCLES
            counter <= counter + 1'b1;
        end
        // When reset_complete, counter stops (holds value)
    end
    
    // Registered output reset signal
    // Output is high (reset asserted) until counter reaches RESET_CYCLES
    // Using active-high output as per specification
    always_ff @(posedge clk or negedge rst_n_in) begin
        if (!rst_n_in) begin
            // Input reset asserted - output reset asserted (high)
            rst_out <= 1'b1;
        end else if (reset_request) begin
            // Soft reset requested - output reset asserted (high)
            rst_out <= 1'b1;
        end else begin
            // Output is inverted reset_complete (high when still resetting)
            rst_out <= ~reset_complete;
        end
    end

endmodule

`default_nettype none
// LED Pattern Demo Top Module for Alchitry Cu v1
// Minimal design that displays an alternating pattern on 8-bit LED output
// Pattern shifts by one position every second
// Board: Alchitry Cu v1 (iCE40-HX8K-CB132)

module led_pattern_top (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,
    
    // Reset button (active low)
    input  logic       rst_n_btn,
    
    // LED outputs (8 LEDs on Alchitry Cu main board)
    output logic [7:0] led
);
    localparam int unsigned BUTTON_DEBOUNCE_US = 10_000;

    // ============================================================
    // Parameters
    // ============================================================
    // Clock frequency: 100 MHz
    // Shift interval: 1 second = 100,000,000 clock cycles
    localparam int CLOCK_FREQ = 100_000_000;
    localparam int SHIFT_COUNT = CLOCK_FREQ;  // 1 second interval
    localparam int COUNTER_WIDTH = $clog2(SHIFT_COUNT + 1);  // 27 bits for 100M

    // ============================================================
    // Reset synchronizer plus debouncer
    // ============================================================
    logic rst_n_sync2;
    logic rst_n;

    ff_sync #(
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(clk),
        .rst_n(1'b1),
        .din(rst_n_btn),
        .dout(rst_n_sync2)
    );

    debouncer #(
        .CLK_FREQ_HZ(CLOCK_FREQ),
        .STABLE_TIME_US(BUTTON_DEBOUNCE_US)
    ) rst_n_btn_debouncer_inst (
        .clk(clk),
        .rst_n(rst_n_sync2),
        .din(rst_n_sync2),
        .dout(rst_n)
    );

    // ============================================================
    // Second Counter
    // ============================================================
    logic [COUNTER_WIDTH-1:0] counter;
    logic shift_pulse;
    
    // Counter threshold with proper width
    localparam logic [COUNTER_WIDTH-1:0] COUNTER_MAX = COUNTER_WIDTH'(SHIFT_COUNT - 1);
    
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            counter <= '0;
        end else if (counter >= COUNTER_MAX) begin
            counter <= '0;
        end else begin
            counter <= counter + 1'b1;
        end
    end
    
    // Generate pulse when counter wraps (every second)
    assign shift_pulse = (counter == COUNTER_MAX);

    // ============================================================
    // LED Pattern Register with Rotating Shift
    // ============================================================
    // Initial pattern: 0xAA = 10101010 (alternating)
    logic [7:0] led_pattern;
    
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            led_pattern <= 8'hAA;  // Alternating pattern: 10101010
        end else if (shift_pulse) begin
            // Rotate left by 1 bit
            led_pattern <= {led_pattern[6:0], led_pattern[7]};
        end
    end

    // ============================================================
    // LED Output Assignment
    // ============================================================
    assign led = led_pattern;

endmodule
`default_nettype wire

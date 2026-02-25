// System LED Controller
// Generates registered system LED status from CPU boot/halt signals.
//
// Parameters:
//   CLK_FREQ_HZ - Input clock frequency in Hz
//
// Interface:
//   clk        - System clock
//   rst_n      - Asynchronous active-low reset
//   cpu_booting - High while CPU is in boot state
//   cpu_halted  - High while CPU is halted
//   sys_led     - Registered LED output

module sys_led_controller #(
    parameter int unsigned CLK_FREQ_HZ = 50_000_000
) (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       cpu_booting,
    input  logic       cpu_halted,
    output logic [7:0] sys_led
);

    logic boot_blink_wave;

    square_wave_generator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .WAVE_FREQ_MILLIHERTZ(500)
    ) boot_blink_generator (
        .clk(clk),
        .rst_n(rst_n),
        .square_wave(boot_blink_wave)
    );

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            sys_led <= 8'hFF;
        end else begin
            sys_led <= 8'h00;
            sys_led[0] <= cpu_booting ? boot_blink_wave : 1'b0;
            sys_led[7] <= cpu_halted;
        end
    end

endmodule

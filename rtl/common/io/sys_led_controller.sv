`default_nettype none
// System LED Controller
// Generates registered system LED status from CPU state, activity handshakes, and com_err.
//
// Parameters:
//   CLK_FREQ_HZ - Input clock frequency in Hz
//
// Interface:
//   clk                 - System clock
//   rst_n               - Synchronous active-low reset
//   cpu_booting         - High while CPU is in boot state
//   cpu_halted          - High while CPU is halted
//   instr_complete      - CPU instruction completion pulse
//   sys_bus_handshake   - System bus handshake pulse
//   host_bus_rx_handshake - Host RX handshake pulse
//   host_bus_tx_handshake - Host TX handshake pulse
//   com_err             - Communication error status input
//   sys_led             - Registered LED output
//
// LED mapping:
//   sys_led[0] - Boot blink (while cpu_booting) or halted state
//   sys_led[1] - instr_complete activity indicator
//   sys_led[2] - sys_bus_handshake activity indicator
//   sys_led[3] - host_bus_rx_handshake activity indicator
//   sys_led[4] - host_bus_tx_handshake activity indicator
//   sys_led[7] - com_err passthrough

module sys_led_controller #(
    parameter int unsigned CLK_FREQ_HZ = 50_000_000
) (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       cpu_booting,
    input  logic       cpu_halted,
    input  logic       instr_complete,
    input  logic       sys_bus_handshake,
    input  logic       host_bus_rx_handshake,
    input  logic       host_bus_tx_handshake,
    input  logic       com_err,
    output logic [7:0] sys_led
);

    // Parameter validation (simulation only)
    initial begin
        if (CLK_FREQ_HZ == 0) begin
            $fatal(1, "sys_led_controller: CLK_FREQ_HZ must be > 0");
        end
    end

    logic boot_blink_wave;
    logic instr_complete_indicator;
    logic sys_bus_indicator;
    logic host_bus_rx_indicator;
    logic host_bus_tx_indicator;

    square_wave_generator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .WAVE_FREQ_MILLIHERTZ(1000)
    ) boot_blink_generator (
        .clk(clk),
        .rst_n(rst_n),
        .square_wave(boot_blink_wave)
    );

    activity_indicator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .INDICATOR_FREQ_MILLIHERTZ(8000)
    ) instr_complete_activity (
        .clk(clk),
        .rst_n(rst_n),
        .activity(instr_complete),
        .indicator(instr_complete_indicator)
    );

    activity_indicator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .INDICATOR_FREQ_MILLIHERTZ(8000)
    ) sys_bus_activity (
        .clk(clk),
        .rst_n(rst_n),
        .activity(sys_bus_handshake),
        .indicator(sys_bus_indicator)
    );

    activity_indicator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .INDICATOR_FREQ_MILLIHERTZ(8000)
    ) host_bus_rx_activity (
        .clk(clk),
        .rst_n(rst_n),
        .activity(host_bus_rx_handshake),
        .indicator(host_bus_rx_indicator)
    );

    activity_indicator #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .INDICATOR_FREQ_MILLIHERTZ(8000)
    ) host_bus_tx_activity (
        .clk(clk),
        .rst_n(rst_n),
        .activity(host_bus_tx_handshake),
        .indicator(host_bus_tx_indicator)
    );

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            sys_led <= 8'hFF;
        end else begin
            sys_led <= 8'h00;
            sys_led[0] <= cpu_booting ? boot_blink_wave : cpu_halted;
            sys_led[1] <= instr_complete_indicator;
            sys_led[2] <= sys_bus_indicator;
            sys_led[3] <= host_bus_rx_indicator;
            sys_led[4] <= host_bus_tx_indicator;
            sys_led[7] <= com_err;
        end
    end

endmodule
`default_nettype wire

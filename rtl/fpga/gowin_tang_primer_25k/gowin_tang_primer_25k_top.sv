`default_nettype none
// FPGA top-level wrapper for the Sipeed Tang Primer 25K (GW5A-LV25MG121).
// Keeps the board-specific adaptation thin and reuses fpga_common_top for the
// shared CPU/UART runtime.

module gowin_tang_primer_25k_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 1_000_000
) (
    input  wire logic clk,
    input  wire logic rst_btn,
    output logic      led_ready,
    output logic      led_done,
    input  wire logic usb_rx,
    output logic      usb_tx
);
    localparam int unsigned BUTTON_DEBOUNCE_US = 10_000;

    logic sys_clk;
    assign sys_clk = clk;

    logic rst_btn_sync2;
    logic rst_btn_debounced;

    // Keep synchronizer reset deasserted so it can safely sample the async
    // button even while downstream reset is asserted.
    ff_sync #(
        .WIDTH(1)
    ) rst_btn_sync_inst (
        .clk(sys_clk),
        .rst(1'b0),
        .din(rst_btn),
        .dout(rst_btn_sync2)
    );

    debouncer #(
        .CLK_FREQ_HZ(50_000_000),
        .STABLE_TIME_US(BUTTON_DEBOUNCE_US)
    ) rst_btn_debouncer_inst (
        .clk(sys_clk),
        .rst(rst_btn_sync2),
        .din(rst_btn_sync2),
        .dout(rst_btn_debounced)
    );

    logic [7:0] sys_led_out;

    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(50_000_000),
        .RESET_CYCLES(50_000_000),
        .BAUD_RATE(BAUD_RATE)
    ) fpga_common_top_inst (
        .sys_clk(sys_clk),
        .video_clk(1'b0),
        .audio_clk(1'b0),
        .rst(rst_btn_debounced),
        .usb_rx(usb_rx),
        .usb_tx(usb_tx),
        .led_out(),
        .sys_led_out(sys_led_out),
        .rst_core(),
        .gamepad_in(10'b0),
        .video_rgb(),
        .video_de(),
        .video_skip(),
        .video_vs(),
        .video_hs(),
        .audio_dac(),
        .audio_lrclk()
    );

    assign led_ready = sys_led_out[0];
    assign led_done  = sys_led_out[1];

endmodule
`default_nettype wire

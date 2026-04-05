`default_nettype none
// FPGA Top-Level Module for iCE Pi Zero (ECP5-25F)
// Wraps RISC-V CPU with host communication via USB serial

module ecp5_icepi_zero_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 1_000_000
) (
    input wire logic clk,
    input wire logic rst_n_btn,
    output logic [4:0] led,
    input wire logic usb_rx,
    output logic usb_tx
);
    localparam int unsigned BUTTON_DEBOUNCE_US = 10_000;

    // iCE Pi Zero board clock is 50 MHz
    logic sys_clk;
    assign sys_clk = clk;
    logic rst_n_btn_sync2;
    logic rst_n_btn_debounced;
    // Keep synchronizer reset deasserted so it can safely sample the async button
    // even while downstream reset is asserted.
    ff_sync #(
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(sys_clk),
        .rst(1'b0),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );

    debouncer #(
        .CLK_FREQ_HZ(50_000_000),
        .STABLE_TIME_US(BUTTON_DEBOUNCE_US)
    ) rst_n_btn_debouncer_inst (
        .clk(sys_clk),
        .rst(~rst_n_btn_sync2),
        .din(rst_n_btn_sync2),
        .dout(rst_n_btn_debounced)
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
        .rst(~rst_n_btn_debounced),
        .usb_rx(usb_rx),
        .usb_tx(usb_tx),
        .led_out(),
        .sys_led_out(sys_led_out),
        .rst_core(),
        .gamepad_in(10'b0),
        .apf_bridge_addr(32'h0000_0000),
        .apf_bridge_rd(1'b0),
        .apf_bridge_wr(1'b0),
        .apf_bridge_wr_data(32'h0000_0000),
        .apf_bridge_rd_data(),
        .video_rgb(),
        .video_de(),
        .video_skip(),
        .video_vs(),
        .video_hs(),
        .audio_dac(),
        .audio_lrclk()
    );

    assign led = sys_led_out[4:0];

endmodule
`default_nettype wire

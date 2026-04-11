`default_nettype none
// FPGA Top-Level Module for Alchitry Au (Artix-7 XC7A35T)
// Wraps RISC-V CPU with host communication via USB serial

module artix7_alchitry_au_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 1_000_000
) (
    input wire logic clk,
    input wire logic rst_n_btn,
    output logic [7:0] led,
    input wire logic usb_rx,
    output logic usb_tx
);
    localparam int unsigned BUTTON_DEBOUNCE_US = 10_000;

    logic rst_n_btn_sync2;
    logic rst_n_btn_debounced;
    // Keep synchronizer reset deasserted so it can safely sample the async button
    // even while downstream reset is asserted.
    ff_sync #(
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(clk),
        .rst(1'b0),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );

    debouncer #(
        .CLK_FREQ_HZ(100_000_000),
        .STABLE_TIME_US(BUTTON_DEBOUNCE_US)
    ) rst_n_btn_debouncer_inst (
        .clk(clk),
        .rst(~rst_n_btn_sync2),
        .din(rst_n_btn_sync2),
        .dout(rst_n_btn_debounced)
    );

    // Alchitry Au: 100 MHz input -> 50 MHz system clock
    logic pll_clk_fb;
    logic pll_clk_out;
    logic pll_locked;
    logic sys_clk;

    PLLE2_ADV #(
        .BANDWIDTH("OPTIMIZED"),
        .CLKFBOUT_MULT(10),
        .CLKFBOUT_PHASE(0.0),
        .CLKIN1_PERIOD(10.0),
        .CLKOUT0_DIVIDE(20),
        .CLKOUT0_DUTY_CYCLE(0.5),
        .CLKOUT0_PHASE(0.0),
        .DIVCLK_DIVIDE(1),
        .REF_JITTER1(0.010),
        .STARTUP_WAIT("FALSE")
    ) pll_inst (
        .CLKFBIN(pll_clk_fb),
        .CLKIN1(clk),
        .CLKIN2(1'b0),
        .CLKINSEL(1'b1),
        .DADDR(7'b0),
        .DCLK(1'b0),
        .DEN(1'b0),
        .DI(16'b0),
        .DO(),
        .DRDY(),
        .DWE(1'b0),
        .PWRDWN(1'b0),
        .RST(~rst_n_btn_debounced),
        .CLKFBOUT(pll_clk_fb),
        .CLKOUT0(pll_clk_out),
        .CLKOUT1(),
        .CLKOUT2(),
        .CLKOUT3(),
        .CLKOUT4(),
        .CLKOUT5(),
        .LOCKED(pll_locked)
    );

    BUFG sys_clk_bufg (
        .I(pll_clk_out),
        .O(sys_clk)
    );

    logic pll_locked_sync2;
    ff_sync #(
        .WIDTH(1)
    ) pll_locked_sync_inst (
        .clk(sys_clk),
        .rst(1'b0),
        .din(pll_locked),
        .dout(pll_locked_sync2)
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
        .sdram_clk(1'b0),
        .rst(~pll_locked_sync2),
        .usb_rx(usb_rx),
        .usb_tx(usb_tx),
        .led_out(),
        .sys_led_out(sys_led_out),
        .rst_core(),
        .gamepad_in(10'b0),
        .ext_cpu_boot(1'b0),
        .ext_cpu_boot_addr(32'h0000_0000),
        .cpu_is_booting(),
        .apf_bridge_addr(32'h0000_0000),
        .apf_bridge_rd(1'b0),
        .apf_bridge_rd_ready(),
        .apf_bridge_wr(1'b0),
        .apf_bridge_wr_ready(),
        .apf_bridge_wr_data(32'h0000_0000),
        .apf_bridge_rd_data(),
        .video_rgb(),
        .video_de(),
        .video_skip(),
        .video_vs(),
        .video_hs(),
        .audio_dac(),
        .audio_lrclk(),
        .sdram_word_rd(),
        .sdram_word_wr(),
        .sdram_word_addr(),
        .sdram_word_data(),
        .sdram_word_q(32'h0000_0000),
        .sdram_word_busy(1'b0)
    );

    assign led = sys_led_out;

endmodule
`default_nettype wire

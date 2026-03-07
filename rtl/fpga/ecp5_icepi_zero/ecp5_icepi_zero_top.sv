// FPGA Top-Level Module for iCE Pi Zero (ECP5-25F)
// Wraps RISC-V CPU with host communication via USB serial

module ecp5_icepi_zero_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b1
) (
    input  logic clk,
    input  logic rst_n_btn,
    output logic led,
    input  logic usb_rx,
    output logic usb_tx
);
    // iCE Pi Zero board clock is 50 MHz
    logic sys_clk;
    assign sys_clk = clk;
    logic rst_n_btn_sync2;
    // Keep synchronizer reset deasserted so it can safely sample the async button
    // even while downstream reset is asserted.
    ff_sync #(
        .STAGES(2),
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(sys_clk),
        .rst_n(1'b1),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );
    logic [7:0] sys_led_out;

    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(50_000_000),
        .RESET_CYCLES(50_000_000)
    ) fpga_common_top_inst (
        .sys_clk(sys_clk),
        .rst_n(rst_n_btn_sync2),
        .usb_rx(usb_rx),
        .usb_tx(usb_tx),
        .led_out(),
        .sys_led_out(sys_led_out),
        .rst_n_core()
    );

    assign led = sys_led_out[0];

endmodule

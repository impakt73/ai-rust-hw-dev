`default_nettype none
module fpga_common_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter bit ENABLE_GFX2D = 1'b0,
    parameter bit ENABLE_AUDIOSYS = 1'b0,
    parameter int CLK_FREQ_HZ = 25_000_000,
    parameter int RESET_CYCLES = 25_000_000,
    parameter int BAUD_RATE = 1_000_000,
    parameter int unsigned GFX2D_BASE_ADDR = 32'h3000_0000,
    parameter int unsigned GFX2D_ADDR_SIZE = 32'h0000_6400,
    parameter int unsigned AUDIOSYS_BASE_ADDR = 32'h6000_0000,
    parameter int unsigned AUDIOSYS_ADDR_SIZE = 32'h0000_0020,
    parameter int unsigned GFX2D_VIDEO_ACTIVE_WIDTH = 256,
    parameter int unsigned GFX2D_VIDEO_ACTIVE_HEIGHT = 224,
    parameter int unsigned GFX2D_VIDEO_H_FRONT_PORCH = 10,
    parameter int unsigned GFX2D_VIDEO_H_SYNC_WIDTH = 1,
    parameter int unsigned GFX2D_VIDEO_H_BACK_PORCH = 133,
    parameter int unsigned GFX2D_VIDEO_V_FRONT_PORCH = 10,
    parameter int unsigned GFX2D_VIDEO_V_SYNC_WIDTH = 1,
    parameter int unsigned GFX2D_VIDEO_V_BACK_PORCH = 277,
    parameter bit GFX2D_VIDEO_HSYNC_ACTIVE_HIGH = 1'b1,
    parameter bit GFX2D_VIDEO_VSYNC_ACTIVE_HIGH = 1'b1,
    parameter int unsigned GFX2D_TILE_WIDTH = 8,
    parameter int unsigned GFX2D_TILE_HEIGHT = 8,
    parameter int unsigned GFX2D_TILE_COLUMNS = 32,
    parameter int unsigned GFX2D_TILE_ROWS = 32,
    parameter AUDIOSYS_INIT_FILE = "../fpga/cyclonev_analogue_pocket/src/fpga/core/sine_table_init.hex"
) (
    input wire logic       sys_clk,
    input wire logic       video_clk,
    input wire logic       audio_clk,
    input wire logic       rst,
    input wire logic       usb_rx,
    output logic       usb_tx,
    output logic [7:0] led_out,
    output logic [7:0] sys_led_out,
    output logic       rst_core,
    output logic [23:0] video_rgb,
    output logic       video_de,
    output logic       video_skip,
    output logic       video_vs,
    output logic       video_hs,
    output logic       audio_dac,
    output logic       audio_lrclk
);
    logic [7:0] host_tx_data;
    logic       host_tx_valid;
    logic       host_tx_ready;
    logic [7:0] host_rx_data;
    logic       host_rx_valid;
    logic       host_rx_ready;
    logic       com_err;
    logic       halted;
    logic       instr_complete;
    logic [31:0] debug_rs1_data;
    logic [31:0] debug_rs2_data;
    logic [31:0] debug_rd_data;
    logic [31:0] debug_pc;
    logic [31:0] debug_instruction;
    logic [31:0] debug_current_pc;
    logic [31:0] debug_current_instruction;
    logic [3:0]  debug_fsm_state;
    logic       cpu_booting;
    logic [31:0] halted_value;

    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .ENABLE_GFX2D(ENABLE_GFX2D),
        .ENABLE_AUDIOSYS(ENABLE_AUDIOSYS),
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .RESET_CYCLES(RESET_CYCLES),
        .GFX2D_BASE_ADDR(GFX2D_BASE_ADDR),
        .GFX2D_ADDR_SIZE(GFX2D_ADDR_SIZE),
        .AUDIOSYS_BASE_ADDR(AUDIOSYS_BASE_ADDR),
        .AUDIOSYS_ADDR_SIZE(AUDIOSYS_ADDR_SIZE),
        .GFX2D_VIDEO_ACTIVE_WIDTH(GFX2D_VIDEO_ACTIVE_WIDTH),
        .GFX2D_VIDEO_ACTIVE_HEIGHT(GFX2D_VIDEO_ACTIVE_HEIGHT),
        .GFX2D_VIDEO_H_FRONT_PORCH(GFX2D_VIDEO_H_FRONT_PORCH),
        .GFX2D_VIDEO_H_SYNC_WIDTH(GFX2D_VIDEO_H_SYNC_WIDTH),
        .GFX2D_VIDEO_H_BACK_PORCH(GFX2D_VIDEO_H_BACK_PORCH),
        .GFX2D_VIDEO_V_FRONT_PORCH(GFX2D_VIDEO_V_FRONT_PORCH),
        .GFX2D_VIDEO_V_SYNC_WIDTH(GFX2D_VIDEO_V_SYNC_WIDTH),
        .GFX2D_VIDEO_V_BACK_PORCH(GFX2D_VIDEO_V_BACK_PORCH),
        .GFX2D_VIDEO_HSYNC_ACTIVE_HIGH(GFX2D_VIDEO_HSYNC_ACTIVE_HIGH),
        .GFX2D_VIDEO_VSYNC_ACTIVE_HIGH(GFX2D_VIDEO_VSYNC_ACTIVE_HIGH),
        .GFX2D_TILE_WIDTH(GFX2D_TILE_WIDTH),
        .GFX2D_TILE_HEIGHT(GFX2D_TILE_HEIGHT),
        .GFX2D_TILE_COLUMNS(GFX2D_TILE_COLUMNS),
        .GFX2D_TILE_ROWS(GFX2D_TILE_ROWS),
        .AUDIOSYS_INIT_FILE(AUDIOSYS_INIT_FILE)
    ) cpu_inst (
        .clk(sys_clk),
        .video_clk(video_clk),
        .audio_clk(audio_clk),
        .rst(rst),
        .host_tx_data(host_tx_data),
        .host_tx_valid(host_tx_valid),
        .host_tx_ready(host_tx_ready),
        .host_rx_data(host_rx_data),
        .host_rx_valid(host_rx_valid),
        .host_rx_ready(host_rx_ready),
        .com_err(com_err),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .halted(halted),
        .instr_complete(instr_complete),
        .debug_rs1_data(debug_rs1_data),
        .debug_rs2_data(debug_rs2_data),
        .debug_rd_data(debug_rd_data),
        .debug_pc(debug_pc),
        .debug_instruction(debug_instruction),
        .debug_current_pc(debug_current_pc),
        .debug_current_instruction(debug_current_instruction),
        .debug_fsm_state(debug_fsm_state),
        .rst_out(rst_core),
        .cpu_booting(cpu_booting),
        .halted_value(halted_value),
        .video_rgb(video_rgb),
        .video_de(video_de),
        .video_skip(video_skip),
        .video_vs(video_vs),
        .video_hs(video_hs),
        .audio_dac(audio_dac),
        .audio_lrclk(audio_lrclk)
    );

    uart #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .BAUD_RATE(BAUD_RATE)
    ) host_uart_inst (
        .clk(sys_clk),
        .rst(rst_core),
        .tx_data(host_tx_data),
        .tx_valid(host_tx_valid),
        .tx_ready(host_tx_ready),
        .rx_data(host_rx_data),
        .rx_valid(host_rx_valid),
        .rx_ready(host_rx_ready),
        .rx_error(com_err),
        .rx_error_clr(1'b0),
        .tx_out(usb_tx),
        .rx_in(usb_rx)
    );

endmodule
`default_nettype wire

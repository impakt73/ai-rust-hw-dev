`default_nettype none

module cyclonev_analogue_pocket_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 9600,
    // Original Source: https://github.com/viler-int10h/vga-text-mode-fonts/blob/master/FONTS/PC-OTHER/ATI8X8.F08
    // bitmap_text_renderer_font_init.hex stores the same font with one 8-bit
    // entry per pixel in row-major order.
    parameter string FONT_INIT_FILE = "./core/bitmap_text_renderer_font_init.hex",
    parameter string CHAR_MAP_INIT_FILE = "./core/bitmap_text_renderer_char_map_init.hex"
) (
    input  wire logic       clk,
    input  wire logic       clk_video,
    input  wire logic       reset_n,
    input  wire logic       serial_rx,
    output logic            serial_tx,
    output logic            rst_out,
    output logic [23:0]     video_rgb,
    output logic            video_de,
    output logic            video_skip,
    output logic            video_vs,
    output logic            video_hs
);
    logic rst;
    logic reset_n_video_sync;
    logic video_rst;
    logic        bitmap_video_de;
    logic        bitmap_video_hs;
    logic        bitmap_video_vs;
    logic [7:0]  bitmap_pixel_data;
    logic [23:0] video_rgb_reg;
    logic        video_de_reg;
    logic        video_skip_reg;
    logic        video_vs_reg;
    logic        video_hs_reg;

    localparam int unsigned VIDEO_ACTIVE_WIDTH = 320;
    localparam int unsigned VIDEO_ACTIVE_HEIGHT = 240;
    localparam int unsigned VIDEO_H_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_H_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_H_BACK_PORCH = 69;
    localparam int unsigned VIDEO_V_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_V_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_V_BACK_PORCH = 261;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            rst <= 1'b1;
        end else begin
            rst <= 1'b0;
        end
    end

    ff_sync #(
        .STAGES(3),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) video_reset_sync (
        .clk(clk_video),
        .rst(1'b0),
        .din(reset_n),
        .dout(reset_n_video_sync)
    );

    assign video_rst = !reset_n_video_sync;

    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(74_250_000),
        .RESET_CYCLES(74_250_000),
        .BAUD_RATE(BAUD_RATE)
    ) repo_top_inst (
        .sys_clk(clk),
        .rst(rst),
        .usb_rx(serial_rx),
        .usb_tx(serial_tx),
        .led_out(),
        .sys_led_out(),
        .rst_core(rst_out)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .H_FRONT_PORCH(VIDEO_H_FRONT_PORCH),
        .H_SYNC_WIDTH(VIDEO_H_SYNC_WIDTH),
        .H_BACK_PORCH(VIDEO_H_BACK_PORCH),
        .V_FRONT_PORCH(VIDEO_V_FRONT_PORCH),
        .V_SYNC_WIDTH(VIDEO_V_SYNC_WIDTH),
        .V_BACK_PORCH(VIDEO_V_BACK_PORCH),
        .HSYNC_ACTIVE_HIGH(1'b1),
        .VSYNC_ACTIVE_HIGH(1'b1),
        .FONT_INIT_FILE(FONT_INIT_FILE),
        .CHAR_MAP_INIT_FILE(CHAR_MAP_INIT_FILE)
    ) pocket_bitmap_text_renderer (
        .clk(clk_video),
        .rst(video_rst),
        .video_de(bitmap_video_de),
        .video_hs(bitmap_video_hs),
        .video_vs(bitmap_video_vs),
        .line_start(),
        .frame_start(),
        .active_x(),
        .active_y(),
        .pixel_data(bitmap_pixel_data)
    );

    always_ff @(posedge clk_video) begin
        if (video_rst) begin
            video_rgb_reg <= 24'h00_00_00;
            video_de_reg <= 1'b0;
            video_skip_reg <= 1'b0;
            video_vs_reg <= 1'b0;
            video_hs_reg <= 1'b0;
        end else begin
            video_rgb_reg <= {bitmap_pixel_data, bitmap_pixel_data, bitmap_pixel_data};
            video_de_reg <= bitmap_video_de;
            video_skip_reg <= 1'b0;
            video_vs_reg <= bitmap_video_vs;
            video_hs_reg <= bitmap_video_hs;
        end
    end

    assign video_rgb = video_rgb_reg;
    assign video_de = video_de_reg;
    assign video_skip = video_skip_reg;
    assign video_vs = video_vs_reg;
    assign video_hs = video_hs_reg;
endmodule

`default_nettype wire

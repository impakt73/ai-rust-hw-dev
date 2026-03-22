`default_nettype none
module bitmap_text_renderer_test_wrapper (
    input wire logic clk,
    input wire logic rst,
    output logic active_video,
    output logic line_start,
    output logic frame_start,
    output logic [3:0] active_x,
    output logic [3:0] active_y,
    output logic pixel_on
);

    logic hsync;
    logic vsync;

    video_sync #(
        .H_ACTIVE(16),
        .H_FRONT_PORCH(8),
        .H_SYNC_WIDTH(1),
        .H_BACK_PORCH(1),
        .V_ACTIVE(16),
        .V_FRONT_PORCH(1),
        .V_SYNC_WIDTH(1),
        .V_BACK_PORCH(1),
        .HSYNC_ACTIVE_HIGH(1'b0),
        .VSYNC_ACTIVE_HIGH(1'b0)
    ) u_video_sync (
        .clk(clk),
        .rst(rst),
        .hsync(hsync),
        .vsync(vsync),
        .active_video(active_video),
        .line_start(line_start),
        .frame_start(frame_start),
        .active_x(active_x),
        .active_y(active_y)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(16),
        .ACTIVE_HEIGHT(16),
        .H_FRONT_PORCH(8),
        .FONT_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_font_init.hex"),
        .CHAR_MAP_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex")
    ) u_bitmap_text_renderer (
        .clk(clk),
        .rst(rst),
        .active_video(active_video),
        .line_start(line_start),
        .active_x(active_x),
        .active_y(active_y),
        .pixel_on(pixel_on)
    );

endmodule
`default_nettype wire

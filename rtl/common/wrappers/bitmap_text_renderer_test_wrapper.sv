`default_nettype none
module bitmap_text_renderer_test_wrapper (
    input wire logic clk,
    input wire logic rst,
    output logic video_de,
    output logic video_hs,
    output logic video_vs,
    output logic line_start,
    output logic frame_start,
    output logic [3:0] active_x,
    output logic [3:0] active_y,
    output logic pixel_on
);

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(16),
        .ACTIVE_HEIGHT(16),
        .H_FRONT_PORCH(1),
        .H_SYNC_WIDTH(1),
        .H_BACK_PORCH(8),
        .V_FRONT_PORCH(1),
        .V_SYNC_WIDTH(1),
        .V_BACK_PORCH(1),
        .FONT_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_font_init.hex"),
        .CHAR_MAP_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex")
    ) u_bitmap_text_renderer (
        .clk(clk),
        .rst(rst),
        .video_de(video_de),
        .video_hs(video_hs),
        .video_vs(video_vs),
        .line_start(line_start),
        .frame_start(frame_start),
        .active_x(active_x),
        .active_y(active_y),
        .pixel_on(pixel_on)
    );

endmodule
`default_nettype wire

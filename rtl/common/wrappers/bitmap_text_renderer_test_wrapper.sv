`default_nettype none
module bitmap_text_renderer_test_wrapper (
    input wire logic clk,
    input wire logic rst,
    output logic video_de,
    output logic video_hs,
    output logic video_vs,
    output logic [23:0] video_rgb
);

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(16),
        .ACTIVE_HEIGHT(16),
        .TILE_COLUMNS(2),
        .TILE_ROWS(2),
        .H_FRONT_PORCH(1),
        .H_SYNC_WIDTH(1),
        .H_BACK_PORCH(8),
        .V_FRONT_PORCH(1),
        .V_SYNC_WIDTH(1),
        .V_BACK_PORCH(1),
        .FONT_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_font_init.hex"),
        .CHAR_MAP_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex"),
        .PALETTE_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_palette_init.hex")
    ) u_bitmap_text_renderer (
        .clk(clk),
        .rst(rst),
        .scroll_x('0),
        .scroll_y('0),
        .video_de(video_de),
        .video_hs(video_hs),
        .video_vs(video_vs),
        .video_rgb(video_rgb)
    );

endmodule
`default_nettype wire

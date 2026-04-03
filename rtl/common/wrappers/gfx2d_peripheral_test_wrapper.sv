`default_nettype none

module gfx2d_peripheral_test_wrapper (
    input  wire logic        sys_clk,
    input  wire logic        video_clk,
    input  wire logic        rst,
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready,
    output logic [23:0]      video_rgb,
    output logic             video_de,
    output logic             video_skip,
    output logic             video_vs,
    output logic             video_hs
);

    gfx2d_peripheral #(
        .VIDEO_ACTIVE_WIDTH(16),
        .VIDEO_ACTIVE_HEIGHT(16),
        .VIDEO_H_FRONT_PORCH(1),
        .VIDEO_H_SYNC_WIDTH(1),
        .VIDEO_H_BACK_PORCH(8),
        .VIDEO_V_FRONT_PORCH(1),
        .VIDEO_V_SYNC_WIDTH(1),
        .VIDEO_V_BACK_PORCH(1),
        .VIDEO_HSYNC_ACTIVE_HIGH(1'b0),
        .VIDEO_VSYNC_ACTIVE_HIGH(1'b0),
        .TILE_WIDTH(8),
        .TILE_HEIGHT(8),
        .TILE_COLUMNS(2),
        .TILE_ROWS(2),
        .FONT_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_font_init.hex"),
        .CHAR_MAP_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex"),
        .PALETTE_INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_palette_init.hex")
    ) u_gfx2d_peripheral (
        .sys_clk(sys_clk),
        .video_clk(video_clk),
        .rst(rst),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready),
        .video_rgb(video_rgb),
        .video_de(video_de),
        .video_skip(video_skip),
        .video_vs(video_vs),
        .video_hs(video_hs)
    );

endmodule

`default_nettype wire

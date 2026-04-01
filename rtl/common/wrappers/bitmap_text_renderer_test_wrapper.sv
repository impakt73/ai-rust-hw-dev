`default_nettype none
module bitmap_text_renderer_test_wrapper (
    input wire logic clk,
    input wire logic rst,
    input wire logic [3:0] scroll_x,
    input wire logic [3:0] scroll_y,
    output logic video_de,
    output logic video_hs,
    output logic video_vs,
    output logic [23:0] video_rgb
);

    localparam int unsigned ACTIVE_WIDTH = 16;
    localparam int unsigned ACTIVE_HEIGHT = 16;
    localparam int unsigned TILE_WIDTH = 8;
    localparam int unsigned TILE_HEIGHT = 8;
    localparam int unsigned TILE_COLUMNS = 2;
    localparam int unsigned TILE_ROWS = 2;
    localparam int unsigned H_FRONT_PORCH = 1;
    localparam int unsigned H_SYNC_WIDTH = 1;
    localparam int unsigned H_BACK_PORCH = 8;
    localparam int unsigned V_FRONT_PORCH = 1;
    localparam int unsigned V_SYNC_WIDTH = 1;
    localparam int unsigned V_BACK_PORCH = 1;
    localparam bit HSYNC_ACTIVE_HIGH = 1'b0;
    localparam bit VSYNC_ACTIVE_HIGH = 1'b0;
    localparam int unsigned VIDEO_SIGNAL_DELAY_CYCLES = 9;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    localparam int unsigned FONT_ADDR_WIDTH = 8 + $clog2(TILE_HEIGHT) + $clog2(TILE_WIDTH);

    logic sync_video_de;
    logic sync_video_hs;
    logic sync_video_vs;
    logic [ACTIVE_X_WIDTH-1:0] sync_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] sync_active_y;
    logic [CHARMAP_ADDR_WIDTH-1:0] char_mem_addr;
    logic [7:0] char_mem_rdata;
    logic [FONT_ADDR_WIDTH-1:0] font_mem_addr;
    logic [7:0] font_mem_rdata;
    logic [7:0] palette_mem_addr;
    logic [23:0] palette_mem_rdata;
    logic [23:0] renderer_video_rgb;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_de_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_hs_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_vs_pipe;

    video_sync #(
        .H_ACTIVE(ACTIVE_WIDTH),
        .H_FRONT_PORCH(H_FRONT_PORCH),
        .H_SYNC_WIDTH(H_SYNC_WIDTH),
        .H_BACK_PORCH(H_BACK_PORCH),
        .V_ACTIVE(ACTIVE_HEIGHT),
        .V_FRONT_PORCH(V_FRONT_PORCH),
        .V_SYNC_WIDTH(V_SYNC_WIDTH),
        .V_BACK_PORCH(V_BACK_PORCH),
        .HSYNC_ACTIVE_HIGH(HSYNC_ACTIVE_HIGH),
        .VSYNC_ACTIVE_HIGH(VSYNC_ACTIVE_HIGH)
    ) u_video_sync (
        .clk(clk),
        .rst(rst),
        .hsync(sync_video_hs),
        .vsync(sync_video_vs),
        .active_video(sync_video_de),
        .line_start(),
        .frame_start(),
        .active_x(sync_active_x),
        .active_y(sync_active_y),
        .scan_x(),
        .scan_y()
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(CHARMAP_ADDR_WIDTH),
        .INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex")
    ) u_char_map_rom (
        .clk(clk),
        .addr(char_mem_addr),
        .rdata(char_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(FONT_ADDR_WIDTH),
        .INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_font_init.hex")
    ) u_font_rom (
        .clk(clk),
        .addr(font_mem_addr),
        .rdata(font_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(24),
        .ADDR_WIDTH(8),
        .INIT_FILE("../rtl/common/wrappers/bitmap_text_renderer_palette_init.hex")
    ) u_palette_rom (
        .clk(clk),
        .addr(palette_mem_addr),
        .rdata(palette_mem_rdata)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(ACTIVE_HEIGHT),
        .TILE_WIDTH(TILE_WIDTH),
        .TILE_HEIGHT(TILE_HEIGHT),
        .TILE_COLUMNS(TILE_COLUMNS),
        .TILE_ROWS(TILE_ROWS)
    ) u_bitmap_text_renderer (
        .clk(clk),
        .rst(rst),
        .screen_x(sync_active_x),
        .screen_y(sync_active_y),
        .scroll_x(scroll_x),
        .scroll_y(scroll_y),
        .char_mem_addr(char_mem_addr),
        .char_mem_rdata(char_mem_rdata),
        .font_mem_addr(font_mem_addr),
        .font_mem_rdata(font_mem_rdata),
        .palette_mem_addr(palette_mem_addr),
        .palette_mem_rdata(palette_mem_rdata),
        .video_rgb(renderer_video_rgb)
    );

    assign video_de = video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_hs = video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_vs = video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_rgb = video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1]
        ? renderer_video_rgb
        : '0;

    always_ff @(posedge clk) begin
        if (rst) begin
            video_de_pipe <= '0;
            video_hs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~HSYNC_ACTIVE_HIGH}};
            video_vs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VSYNC_ACTIVE_HIGH}};
        end else begin
            video_de_pipe <= {video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_de};
            video_hs_pipe <= {video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_hs};
            video_vs_pipe <= {video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_vs};
        end
    end

endmodule
`default_nettype wire

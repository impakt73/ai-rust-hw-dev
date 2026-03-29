`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles during scanout using a character map ROM and a
// CP437-compatible font ROM.
//
// This module owns the raster timing generator. The internal `video_sync`
// instance produces the current active-region coordinates, which directly drive
// the character/font/palette ROM lookup pipeline. The public video control
// signals are delayed through shift registers so they stay aligned with the
// RGB pixel value when the outputs leave the module.
//
// The registered timing outputs are shaped to match the Analogue Pocket video
// control interface:
//   - `video_de` is the registered active-video/display-enable qualifier
//   - `video_hs` / `video_vs` are the registered sync pulses
module bitmap_text_renderer #(
    parameter int unsigned ACTIVE_WIDTH = 640,
    parameter int unsigned ACTIVE_HEIGHT = 480,
    parameter int unsigned TILE_WIDTH = 8,
    parameter int unsigned TILE_HEIGHT = 8,
    parameter int unsigned H_FRONT_PORCH = 16,
    parameter int unsigned H_SYNC_WIDTH = 96,
    parameter int unsigned H_BACK_PORCH = 48,
    parameter int unsigned V_FRONT_PORCH = 10,
    parameter int unsigned V_SYNC_WIDTH = 2,
    parameter int unsigned V_BACK_PORCH = 33,
    parameter bit HSYNC_ACTIVE_HIGH = 1'b0,
    parameter bit VSYNC_ACTIVE_HIGH = 1'b0,
    parameter FONT_INIT_FILE = "",
    parameter CHAR_MAP_INIT_FILE = "",
    parameter PALETTE_INIT_FILE = ""
) (
    input wire logic clk,
    input wire logic rst,
    output logic video_de,
    output logic video_hs,
    output logic video_vs,
    output logic [23:0] video_rgb
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_COLUMN_INDEX_WIDTH = (TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH);
    localparam int unsigned TILE_ROW_SHIFT = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned TILE_COLUMN_SHIFT = (TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH);
    localparam int unsigned FONT_GLYPH_OFFSET_WIDTH =
        FONT_ROW_INDEX_WIDTH + FONT_COLUMN_INDEX_WIDTH;
    localparam int unsigned FONT_ROM_ADDR_WIDTH =
        8 + FONT_GLYPH_OFFSET_WIDTH;
    localparam int unsigned PALETTE_ROM_DATA_WIDTH = 24;
    localparam int unsigned PALETTE_ROM_ADDR_WIDTH = FONT_ROM_DATA_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    // The direct lookup path is:
    //   - 2 cycles: character-map sync_sprom latency
    //   - 2 cycles: font sync_sprom latency
    //   - 2 cycles: palette sync_sprom latency
    // Delay the public video control outputs by the same 6 cycles so they stay
    // aligned with the RGB data emerging from the palette ROM.
    localparam int unsigned VIDEO_SIGNAL_DELAY_CYCLES = 6;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH;
    localparam int unsigned TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT;
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);

    logic sync_video_de;
    logic sync_video_hs;
    logic sync_video_vs;
    logic [ACTIVE_X_WIDTH-1:0] sync_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] sync_active_y;

    logic [CHARMAP_ADDR_WIDTH-1:0] char_map_addr;
    logic [CHARMAP_DATA_WIDTH-1:0] char_map_rdata;
    logic [FONT_ROM_ADDR_WIDTH-1:0] font_addr;
    logic [FONT_ROM_DATA_WIDTH-1:0] font_glyph_rdata;
    logic [PALETTE_ROM_ADDR_WIDTH-1:0] palette_addr;
    logic [PALETTE_ROM_DATA_WIDTH-1:0] palette_rdata;

    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset;
    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset_d0;
    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset_d1;
    logic [TILE_COLUMN_WIDTH-1:0] tile_column;
    logic [TILE_ROW_WIDTH-1:0] tile_row;
    logic [CHARMAP_ADDR_WIDTH-1:0] tile_row_base_addr;
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
        .DATA_WIDTH(CHARMAP_DATA_WIDTH),
        .ADDR_WIDTH(CHARMAP_ADDR_WIDTH),
        .INIT_FILE(CHAR_MAP_INIT_FILE)
    ) u_char_map_rom (
        .clk(clk),
        .addr(char_map_addr),
        .rdata(char_map_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(FONT_ROM_DATA_WIDTH),
        .ADDR_WIDTH(FONT_ROM_ADDR_WIDTH),
        .INIT_FILE(FONT_INIT_FILE)
    ) u_font_rom (
        .clk(clk),
        .addr(font_addr),
        .rdata(font_glyph_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(PALETTE_ROM_DATA_WIDTH),
        .ADDR_WIDTH(PALETTE_ROM_ADDR_WIDTH),
        .INIT_FILE(PALETTE_INIT_FILE)
    ) u_palette_rom (
        .clk(clk),
        .addr(palette_addr),
        .rdata(palette_rdata)
    );

`ifndef SYNTHESIS
    initial begin
        if (ACTIVE_WIDTH == 0) begin
            $fatal(1, "bitmap_text_renderer: ACTIVE_WIDTH must be > 0");
        end
        if (ACTIVE_HEIGHT == 0) begin
            $fatal(1, "bitmap_text_renderer: ACTIVE_HEIGHT must be > 0");
        end
        if (TILE_WIDTH != 8) begin
            $fatal(1, "bitmap_text_renderer: TILE_WIDTH must be 8");
        end
        if (TILE_HEIGHT != 8) begin
            $fatal(1, "bitmap_text_renderer: TILE_HEIGHT must be 8");
        end
        if ((ACTIVE_WIDTH % TILE_WIDTH) != 0) begin
            $fatal(1, "bitmap_text_renderer: ACTIVE_WIDTH must be divisible by TILE_WIDTH");
        end
        if ((ACTIVE_HEIGHT % TILE_HEIGHT) != 0) begin
            $fatal(1, "bitmap_text_renderer: ACTIVE_HEIGHT must be divisible by TILE_HEIGHT");
        end
    end
`endif

    always_comb begin
        tile_column = TILE_COLUMN_WIDTH'(sync_active_x >> TILE_COLUMN_SHIFT);
        tile_row = TILE_ROW_WIDTH'(sync_active_y >> TILE_ROW_SHIFT);
        tile_row_base_addr = CHARMAP_ADDR_WIDTH'(tile_row * TILE_COLUMNS);
        glyph_offset = {
            FONT_ROW_INDEX_WIDTH'(sync_active_y[FONT_ROW_INDEX_WIDTH-1:0]),
            FONT_COLUMN_INDEX_WIDTH'(sync_active_x[FONT_COLUMN_INDEX_WIDTH-1:0])
        };
        // `sync_sprom` already registers its address/data path internally, so a
        // separate registered character-map address stage is unnecessary here.
        char_map_addr = tile_row_base_addr + CHARMAP_ADDR_WIDTH'(tile_column);
        font_addr = {char_map_rdata, glyph_offset_d1};
        palette_addr = font_glyph_rdata;
    end

    assign video_de = video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_hs = video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_vs = video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_rgb = video_de ? palette_rdata : '0;

    always_ff @(posedge clk) begin
        if (rst) begin
            video_de_pipe <= '0;
            video_hs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~HSYNC_ACTIVE_HIGH}};
            video_vs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VSYNC_ACTIVE_HIGH}};
            glyph_offset_d0 <= '0;
            glyph_offset_d1 <= '0;
        end else begin
            video_de_pipe <= {video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_de};
            video_hs_pipe <= {video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_hs};
            video_vs_pipe <= {video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_vs};
            glyph_offset_d0 <= glyph_offset;
            glyph_offset_d1 <= glyph_offset_d0;
        end
    end

endmodule
`default_nettype wire

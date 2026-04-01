`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles for an externally supplied screen coordinate.
//
// External logic owns the raster timing generator and all backing memories.
// This module only computes the lookup pipeline:
//   screen_x/screen_y -> char_mem_addr -> font_mem_addr -> palette_mem_addr -> video_rgb
// The fixed registered pipeline latency from coordinate input to `video_rgb` is
// 9 cycles, so callers must delay any associated sync/display-enable signals by
// the same amount if they need aligned video-control outputs.
module bitmap_text_renderer #(
    parameter int unsigned ACTIVE_WIDTH = 640,
    parameter int unsigned ACTIVE_HEIGHT = 480,
    parameter int unsigned TILE_WIDTH = 8,
    parameter int unsigned TILE_HEIGHT = 8,
    parameter int unsigned TILE_COLUMNS = 128,
    parameter int unsigned TILE_ROWS = 64
) (
    input wire logic clk,
    input wire logic rst,
    input wire logic [((ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH))-1:0] screen_x,
    input wire logic [((ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT))-1:0] screen_y,
    input wire logic [(((TILE_WIDTH * TILE_COLUMNS) <= 1) ? 1 : $clog2(TILE_WIDTH * TILE_COLUMNS))-1:0]
        scroll_x,
    input wire logic [(((TILE_HEIGHT * TILE_ROWS) <= 1) ? 1 : $clog2(TILE_HEIGHT * TILE_ROWS))-1:0]
        scroll_y,
    output logic [(((TILE_COLUMNS * TILE_ROWS) <= 1) ? 1 : $clog2(TILE_COLUMNS * TILE_ROWS))-1:0]
        char_mem_addr,
    input wire logic [7:0] char_mem_rdata,
    output logic [(8 + (((TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT)) +
        (((TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH)))))-1:0] font_mem_addr,
    input wire logic [7:0] font_mem_rdata,
    output logic [7:0] palette_mem_addr,
    input wire logic [23:0] palette_mem_rdata,
    output logic [23:0] video_rgb
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_COLUMN_INDEX_WIDTH = (TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH);
    localparam int unsigned TILE_ROW_SHIFT = (TILE_HEIGHT <= 1) ? 0 : $clog2(TILE_HEIGHT);
    localparam int unsigned TILE_COLUMN_SHIFT = (TILE_WIDTH <= 1) ? 0 : $clog2(TILE_WIDTH);
    localparam int unsigned FONT_GLYPH_OFFSET_WIDTH =
        FONT_ROW_INDEX_WIDTH + FONT_COLUMN_INDEX_WIDTH;
    localparam int unsigned FONT_ROM_ADDR_WIDTH =
        8 + FONT_GLYPH_OFFSET_WIDTH;
    localparam int unsigned PALETTE_ROM_DATA_WIDTH = 24;
    localparam int unsigned PALETTE_ROM_ADDR_WIDTH = FONT_ROM_DATA_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    // The registered lookup/output path is:
    //   - 1 cycle: registered scrolled tilemap-coordinate stage
    //   - 1 cycle: registered character-map address stage
    //   - 2 cycles: character-memory latency
    //   - 2 cycles: font-memory latency
    //   - 2 cycles: palette-memory latency
    //   - 1 cycle: registered `video_rgb` output stage
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned TILEMAP_WIDTH = TILE_WIDTH * TILE_COLUMNS;
    localparam int unsigned TILEMAP_HEIGHT = TILE_HEIGHT * TILE_ROWS;
    localparam int unsigned SCROLL_X_WIDTH = (TILEMAP_WIDTH <= 1) ? 1 : $clog2(TILEMAP_WIDTH);
    localparam int unsigned SCROLL_Y_WIDTH = (TILEMAP_HEIGHT <= 1) ? 1 : $clog2(TILEMAP_HEIGHT);
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);

    logic [SCROLL_X_WIDTH-1:0] scrolled_active_x;
    logic [SCROLL_Y_WIDTH-1:0] scrolled_active_y;

    logic [CHARMAP_ADDR_WIDTH-1:0] char_mem_addr_next;

    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset;
    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset_d0;
    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset_d1;
    logic [FONT_GLYPH_OFFSET_WIDTH-1:0] glyph_offset_d2;
    logic [TILE_COLUMN_WIDTH-1:0] tile_column;
    logic [TILE_ROW_WIDTH-1:0] tile_row;
    logic [CHARMAP_ADDR_WIDTH-1:0] tile_row_base_addr;
    logic [SCROLL_X_WIDTH:0] scroll_x_sum;
    logic [SCROLL_Y_WIDTH:0] scroll_y_sum;
    logic [SCROLL_X_WIDTH-1:0] scrolled_active_x_next;
    logic [SCROLL_Y_WIDTH-1:0] scrolled_active_y_next;

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
        if ((TILE_COLUMNS == 0) || ((TILE_COLUMNS & (TILE_COLUMNS - 1)) != 0)) begin
            $fatal(1, "bitmap_text_renderer: TILE_COLUMNS must be a non-zero power of two");
        end
        if ((TILE_ROWS == 0) || ((TILE_ROWS & (TILE_ROWS - 1)) != 0)) begin
            $fatal(1, "bitmap_text_renderer: TILE_ROWS must be a non-zero power of two");
        end
    end
`endif

    always_comb begin
        scroll_x_sum = {1'b0, SCROLL_X_WIDTH'(screen_x)} + {1'b0, scroll_x};
        scroll_y_sum = {1'b0, SCROLL_Y_WIDTH'(screen_y)} + {1'b0, scroll_y};
        // Truncating the carry bit wraps the scrolled coordinates within the
        // power-of-two tilemap dimensions without explicit compare/subtract logic.
        scrolled_active_x_next = scroll_x_sum[SCROLL_X_WIDTH-1:0];
        scrolled_active_y_next = scroll_y_sum[SCROLL_Y_WIDTH-1:0];
        tile_column = TILE_COLUMN_WIDTH'(scrolled_active_x >> TILE_COLUMN_SHIFT);
        tile_row = TILE_ROW_WIDTH'(scrolled_active_y >> TILE_ROW_SHIFT);
        tile_row_base_addr = CHARMAP_ADDR_WIDTH'(tile_row * TILE_COLUMNS);
        glyph_offset = {
            FONT_ROW_INDEX_WIDTH'(scrolled_active_y[FONT_ROW_INDEX_WIDTH-1:0]),
            FONT_COLUMN_INDEX_WIDTH'(scrolled_active_x[FONT_COLUMN_INDEX_WIDTH-1:0])
        };
        char_mem_addr_next = tile_row_base_addr + CHARMAP_ADDR_WIDTH'(tile_column);
        font_mem_addr = {char_mem_rdata, glyph_offset_d2};
        palette_mem_addr = font_mem_rdata;
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            scrolled_active_x <= '0;
            scrolled_active_y <= '0;
            char_mem_addr <= '0;
            glyph_offset_d0 <= '0;
            glyph_offset_d1 <= '0;
            glyph_offset_d2 <= '0;
            video_rgb <= '0;
        end else begin
            scrolled_active_x <= scrolled_active_x_next;
            scrolled_active_y <= scrolled_active_y_next;
            char_mem_addr <= char_mem_addr_next;
            glyph_offset_d0 <= glyph_offset;
            glyph_offset_d1 <= glyph_offset_d0;
            glyph_offset_d2 <= glyph_offset_d1;
            video_rgb <= palette_mem_rdata;
        end
    end

endmodule
`default_nettype wire

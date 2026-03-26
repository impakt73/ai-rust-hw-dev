`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles during scanout using a character map ROM and a
// CP437-compatible font ROM.
//
// This module owns the raster timing generator so every externally visible
// timing signal can be registered in the same stage as `pixel_on`. The internal
// `video_sync` instance produces the current scan position while a small
// prefetcher keeps the character/font ROM pipeline pointed several pixels ahead
// so the glyph pixel byte arriving this cycle already matches the current
// pixel. A final register stage captures the public timing bundle plus
// `pixel_on`
// together, keeping the output interface self-consistent for downstream video
// users while still giving the FPGA an extra timing-breaking register on the
// pixel path.
//
// The registered timing outputs are shaped to match the Analogue Pocket video
// control interface:
//   - `video_de` is the registered active-video/display-enable qualifier
//   - `video_hs` / `video_vs` are the registered sync pulses
// In addition, the module exposes registered active-region coordinates
// (`active_x`, `active_y`) and start-of-line/frame pulses (`line_start`,
// `frame_start`) because they are useful to internal consumers and focused
// tests.

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
    parameter CHAR_MAP_INIT_FILE = ""
) (
    input wire logic clk,
    input wire logic rst,
    output logic video_de,
    output logic video_hs,
    output logic video_vs,
    output logic line_start,
    output logic frame_start,
    output logic [((ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH)) - 1:0] active_x,
    output logic [((ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT)) - 1:0] active_y,
    output logic [7:0] pixel_on
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_COLUMN_INDEX_WIDTH = (TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH);
    localparam int unsigned FONT_ROM_ADDR_WIDTH =
        8 + FONT_ROW_INDEX_WIDTH + FONT_COLUMN_INDEX_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    // Back-porch budget for the renderer's per-pixel ROM prefetch pipeline:
    //   - 1 cycle: registered fetch-wrap/prefetch scan stage
    //   - 1 cycle: registered character-map request stage
    //   - 2 cycles: character-map sync_sprom latency
    //   - 2 cycles: font-pixel sync_sprom latency (`font_addr` combines
    //               `char_map_rdata` with the aligned glyph row/column)
    // The horizontal back porch must cover this 6-cycle prefetch lead so the
    // pipeline is fully primed before scanout enters the active region.
    localparam int unsigned FONT_PIPELINE_CYCLES = 6;
    localparam int unsigned H_TOTAL = ACTIVE_WIDTH + H_FRONT_PORCH + H_SYNC_WIDTH + H_BACK_PORCH;
    localparam int unsigned V_TOTAL = ACTIVE_HEIGHT + V_FRONT_PORCH + V_SYNC_WIDTH + V_BACK_PORCH;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned H_COUNTER_WIDTH = (H_TOTAL <= 1) ? 1 : $clog2(H_TOTAL);
    localparam int unsigned V_COUNTER_WIDTH = (V_TOTAL <= 1) ? 1 : $clog2(V_TOTAL);
    localparam logic [H_COUNTER_WIDTH-1:0] H_ACTIVE_START = H_COUNTER_WIDTH'(H_BACK_PORCH);
    localparam logic [V_COUNTER_WIDTH-1:0] V_ACTIVE_START = V_COUNTER_WIDTH'(V_BACK_PORCH);
    localparam int unsigned TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH;
    localparam int unsigned TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT;
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    // With H_BACK_PORCH >= FONT_PIPELINE_CYCLES the back porch provides enough
    // warm-up cycles to fully prime the prefetch pipeline before the first
    // active pixel of every line, including the very first line after reset.
    // Reset values for the pipeline stages are therefore simple zeros.
    localparam logic [H_COUNTER_WIDTH-1:0] FETCH_WRAP_START =
        H_COUNTER_WIDTH'(H_TOTAL - FONT_PIPELINE_CYCLES);

    logic sync_video_de;
    logic sync_video_hs;
    logic sync_video_vs;
    logic sync_line_start;
    logic sync_frame_start;
    logic [ACTIVE_X_WIDTH-1:0] sync_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] sync_active_y;
    logic [H_COUNTER_WIDTH-1:0] sync_scan_x;
    logic [V_COUNTER_WIDTH-1:0] sync_scan_y;

    logic [CHARMAP_ADDR_WIDTH-1:0] char_map_addr;
    logic [CHARMAP_DATA_WIDTH-1:0] char_map_rdata;
    logic [FONT_ROM_ADDR_WIDTH-1:0] font_addr;
    logic [FONT_ROM_DATA_WIDTH-1:0] font_glyph_rdata;

    logic [FONT_ROM_DATA_WIDTH-1:0] pixel_on_next;
    logic [H_COUNTER_WIDTH-1:0] fetch_scan_x_next;
    logic [V_COUNTER_WIDTH-1:0] fetch_scan_y_next;
    logic [H_COUNTER_WIDTH-1:0] fetch_scan_x_prefetch;
    logic [V_COUNTER_WIDTH-1:0] fetch_scan_y_prefetch;
    logic [FONT_ROW_INDEX_WIDTH-1:0] glyph_row_d0;
    logic [FONT_ROW_INDEX_WIDTH-1:0] glyph_row_d1;
    logic [FONT_ROW_INDEX_WIDTH-1:0] glyph_row_d2;
    logic [TILE_COLUMN_WIDTH-1:0] fetch_tile_column_prefetch;
    logic [TILE_ROW_WIDTH-1:0] fetch_tile_row_prefetch;
    logic [CHARMAP_ADDR_WIDTH-1:0] fetch_tile_row_base_addr_prefetch;
    logic [FONT_ROW_INDEX_WIDTH-1:0] font_glyph_row_prefetch;
    logic [CHARMAP_ADDR_WIDTH-1:0] char_map_addr_prefetch;
    logic [FONT_COLUMN_INDEX_WIDTH-1:0] glyph_column_d0;
    logic [FONT_COLUMN_INDEX_WIDTH-1:0] glyph_column_d1;
    logic [FONT_COLUMN_INDEX_WIDTH-1:0] glyph_column_d2;
    logic [H_COUNTER_WIDTH-1:0] fetch_scan_x_offset;
    logic [V_COUNTER_WIDTH-1:0] fetch_scan_y_offset;

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
        .line_start(sync_line_start),
        .frame_start(sync_frame_start),
        .active_x(sync_active_x),
        .active_y(sync_active_y),
        .scan_x(sync_scan_x),
        .scan_y(sync_scan_y)
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
        if (H_BACK_PORCH < FONT_PIPELINE_CYCLES) begin
            $fatal(1, "bitmap_text_renderer: H_BACK_PORCH must be >= %0d", FONT_PIPELINE_CYCLES);
        end
    end
`endif

    always_comb begin
        if (sync_scan_x >= FETCH_WRAP_START) begin
            fetch_scan_x_next = sync_scan_x - FETCH_WRAP_START;
            if (sync_scan_y == V_COUNTER_WIDTH'(V_TOTAL - 1)) begin
                fetch_scan_y_next = '0;
            end else begin
                fetch_scan_y_next = sync_scan_y + 1'b1;
            end
        end else begin
            fetch_scan_x_next = sync_scan_x + H_COUNTER_WIDTH'(FONT_PIPELINE_CYCLES);
            fetch_scan_y_next = sync_scan_y;
        end
    end

    always_comb begin
        fetch_scan_x_offset = fetch_scan_x_prefetch - H_ACTIVE_START;
        fetch_scan_y_offset = fetch_scan_y_prefetch - V_ACTIVE_START;
        fetch_tile_column_prefetch = TILE_COLUMN_WIDTH'(fetch_scan_x_offset >> 3);
        fetch_tile_row_prefetch = TILE_ROW_WIDTH'(fetch_scan_y_offset >> 3);
        fetch_tile_row_base_addr_prefetch = CHARMAP_ADDR_WIDTH'(fetch_tile_row_prefetch * TILE_COLUMNS);
        font_glyph_row_prefetch = FONT_ROW_INDEX_WIDTH'(
            fetch_scan_y_offset[FONT_ROW_INDEX_WIDTH-1:0]
        );
        char_map_addr_prefetch =
            fetch_tile_row_base_addr_prefetch + CHARMAP_ADDR_WIDTH'(fetch_tile_column_prefetch);
    end

    always_comb begin
        font_addr = {char_map_rdata, glyph_row_d2, glyph_column_d2};
        pixel_on_next = sync_video_de ? font_glyph_rdata : '0;
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            video_de <= 1'b0;
            video_hs <= ~HSYNC_ACTIVE_HIGH;
            video_vs <= ~VSYNC_ACTIVE_HIGH;
            line_start <= 1'b0;
            frame_start <= 1'b0;
            active_x <= '0;
            active_y <= '0;
            fetch_scan_x_prefetch <= '0;
            fetch_scan_y_prefetch <= '0;
            char_map_addr <= '0;
            glyph_row_d0 <= '0;
            glyph_row_d1 <= '0;
            glyph_row_d2 <= '0;
            glyph_column_d0 <= '0;
            glyph_column_d1 <= '0;
            glyph_column_d2 <= '0;
            pixel_on <= '0;
        end else begin
            fetch_scan_x_prefetch <= fetch_scan_x_next;
            fetch_scan_y_prefetch <= fetch_scan_y_next;
            char_map_addr <= char_map_addr_prefetch;
            glyph_row_d0 <= font_glyph_row_prefetch;
            glyph_row_d1 <= glyph_row_d0;
            glyph_row_d2 <= glyph_row_d1;
            glyph_column_d0 <= fetch_scan_x_offset[FONT_COLUMN_INDEX_WIDTH-1:0];
            glyph_column_d1 <= glyph_column_d0;
            glyph_column_d2 <= glyph_column_d1;
            video_de <= sync_video_de;
            video_hs <= sync_video_hs;
            video_vs <= sync_video_vs;
            line_start <= sync_line_start;
            frame_start <= sync_frame_start;
            active_x <= sync_active_x;
            active_y <= sync_active_y;
            pixel_on <= pixel_on_next;
        end
    end

endmodule
`default_nettype wire

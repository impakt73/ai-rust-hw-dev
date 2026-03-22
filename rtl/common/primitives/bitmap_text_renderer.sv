`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles during scanout using a character map ROM and a
// CP437-compatible font ROM.
//
// The renderer pipelines its ROM fetches so that each font-row fetch provides
// 8 pixel decisions for a single character row.
//
// Two synchronous ROMs sit in the fetch path:
//   1. character map ROM  -> returns an 8-bit character index
//   2. font row ROM       -> returns the 8-bit bitmap row for that character
//
// Each sync_sprom contributes two cycles of latency, so the renderer always
// fetches ahead of the currently displayed tile. The fetched row for the next
// tile is stored in `next_tile_row_bits`, then transferred into
// `current_tile_row_bits` when scanout reaches that tile.
//
// `pixel_on` itself is also registered for FPGA timing. To keep that registered
// output aligned with the externally supplied `active_video`/`active_x`/`active_y`
// signals, the module mirrors the upstream raster timing internally and
// computes the pixel value for the *next* scan position before registering it.
// This matches the strategy used by `video_sync`, where registered outputs are
// driven from next-state timing calculations instead of current-state values.

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
    parameter string FONT_INIT_FILE = "",
    parameter string CHAR_MAP_INIT_FILE = ""
) (
    input wire logic clk,
    input wire logic rst,
    input wire logic active_video,
    input wire logic line_start,
    input wire logic [((ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH)) - 1:0] active_x,
    input wire logic [((ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT)) - 1:0] active_y,
    output logic pixel_on
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_ROM_ADDR_WIDTH = 8 + FONT_ROW_INDEX_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    localparam int unsigned FONT_PIPELINE_CYCLES = 8;
    localparam int unsigned H_TOTAL = ACTIVE_WIDTH + H_FRONT_PORCH + H_SYNC_WIDTH + H_BACK_PORCH;
    localparam int unsigned V_TOTAL = ACTIVE_HEIGHT + V_FRONT_PORCH + V_SYNC_WIDTH + V_BACK_PORCH;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned H_COUNTER_WIDTH = (H_TOTAL <= 1) ? 1 : $clog2(H_TOTAL);
    localparam int unsigned V_COUNTER_WIDTH = (V_TOTAL <= 1) ? 1 : $clog2(V_TOTAL);
    localparam int unsigned TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH;
    localparam int unsigned TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT;
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    localparam logic [H_COUNTER_WIDTH-1:0] H_LAST = H_COUNTER_WIDTH'(H_TOTAL - 1);
    localparam logic [V_COUNTER_WIDTH-1:0] V_LAST = V_COUNTER_WIDTH'(V_TOTAL - 1);
    localparam logic [ACTIVE_Y_WIDTH-1:0] ACTIVE_HEIGHT_LAST = ACTIVE_Y_WIDTH'(ACTIVE_HEIGHT - 1);
    localparam logic [TILE_COLUMN_WIDTH-1:0] LAST_TILE_COLUMN =
        TILE_COLUMN_WIDTH'(TILE_COLUMNS - 1);
    localparam logic [2:0] PREFETCH_TRIGGER_PIXEL = 3'd0;

    logic [CHARMAP_ADDR_WIDTH-1:0] char_map_addr;
    logic [CHARMAP_DATA_WIDTH-1:0] char_map_rdata;
    logic [FONT_ROM_ADDR_WIDTH-1:0] font_addr;
    logic [FONT_ROM_DATA_WIDTH-1:0] font_glyph_rdata;

    logic active_video_d;
    logic [ACTIVE_Y_WIDTH-1:0] latched_active_y;
    logic [7:0] current_tile_row_bits;
    logic [7:0] next_tile_row_bits;
    logic next_tile_valid;
    logic pixel_on_next;
    logic render_active_video_next;
    logic char_req_valid_d0;
    logic char_req_valid_d1;
    logic char_req_valid_d2;
    logic [FONT_ROW_INDEX_WIDTH-1:0] char_req_glyph_row_d0;
    logic [FONT_ROW_INDEX_WIDTH-1:0] char_req_glyph_row_d1;
    logic [FONT_ROW_INDEX_WIDTH-1:0] char_req_glyph_row_d2;
    logic font_req_valid_d0;
    logic font_req_valid_d1;
    logic font_req_valid_d2;

    logic issue_char_request;
    logic [CHARMAP_ADDR_WIDTH-1:0] requested_char_map_addr;
    logic [FONT_ROW_INDEX_WIDTH-1:0] requested_glyph_row;
    logic [TILE_COLUMN_WIDTH-1:0] active_tile_column;
    logic [TILE_ROW_WIDTH-1:0] active_tile_row;
    logic [ACTIVE_Y_WIDTH-1:0] next_line_active_y;
    logic [TILE_ROW_WIDTH-1:0] next_line_tile_row;
    logic [FONT_ROW_INDEX_WIDTH-1:0] active_x_in_tile;
    logic [H_COUNTER_WIDTH-1:0] h_counter;
    logic [V_COUNTER_WIDTH-1:0] v_counter;
    logic [H_COUNTER_WIDTH-1:0] h_counter_next;
    logic [V_COUNTER_WIDTH-1:0] v_counter_next;
    logic [ACTIVE_X_WIDTH-1:0] render_active_x_next;
    logic [ACTIVE_Y_WIDTH-1:0] render_active_y_next;
    logic [FONT_ROW_INDEX_WIDTH-1:0] render_active_x_in_tile_next;
    logic expected_active_video;
    logic expected_line_start;
    logic [ACTIVE_X_WIDTH-1:0] expected_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] expected_active_y;

    function automatic logic [CHARMAP_ADDR_WIDTH-1:0] make_char_map_addr(
        input logic [TILE_ROW_WIDTH-1:0] tile_row,
        input logic [TILE_COLUMN_WIDTH-1:0] tile_column
    );
        int unsigned char_map_index;

        char_map_index = (tile_row * TILE_COLUMNS) + int'(tile_column);
        make_char_map_addr = CHARMAP_ADDR_WIDTH'(char_map_index);
    endfunction

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
        if (H_SYNC_WIDTH == 0) begin
            $fatal(1, "bitmap_text_renderer: H_SYNC_WIDTH must be > 0");
        end
        if (V_SYNC_WIDTH == 0) begin
            $fatal(1, "bitmap_text_renderer: V_SYNC_WIDTH must be > 0");
        end
        if (H_FRONT_PORCH < FONT_PIPELINE_CYCLES) begin
            $fatal(1, "bitmap_text_renderer: H_FRONT_PORCH must be >= %0d", FONT_PIPELINE_CYCLES);
        end
    end

    always_comb begin
        // Mirror the upstream raster generator so the registered `pixel_on`
        // output can be computed for the scan position that will become visible
        // immediately after the next rising edge.
        if (h_counter == H_LAST) begin
            h_counter_next = '0;
            if (v_counter == V_LAST) begin
                v_counter_next = '0;
            end else begin
                v_counter_next = v_counter + 1'b1;
            end
        end else begin
            h_counter_next = h_counter + 1'b1;
            v_counter_next = v_counter;
        end

        render_active_video_next =
            (h_counter_next < H_COUNTER_WIDTH'(ACTIVE_WIDTH)) &&
            (v_counter_next < V_COUNTER_WIDTH'(ACTIVE_HEIGHT));
        render_active_x_next = render_active_video_next ? ACTIVE_X_WIDTH'(h_counter_next) : '0;
        render_active_y_next = render_active_video_next ? ACTIVE_Y_WIDTH'(v_counter_next) : '0;
        render_active_x_in_tile_next = render_active_x_next[FONT_ROW_INDEX_WIDTH-1:0];
        expected_active_video =
            (h_counter < H_COUNTER_WIDTH'(ACTIVE_WIDTH)) &&
            (v_counter < V_COUNTER_WIDTH'(ACTIVE_HEIGHT));
        expected_line_start = (h_counter == '0);
        expected_active_x = expected_active_video ? ACTIVE_X_WIDTH'(h_counter) : '0;
        expected_active_y = expected_active_video ? ACTIVE_Y_WIDTH'(v_counter) : '0;

        // The externally supplied timing inputs describe the *current* pixel.
        // They still drive the fetch scheduler, which always works several
        // cycles ahead because the ROM path is fully synchronous.
        active_x_in_tile = active_x[2:0];
        active_tile_column = TILE_COLUMN_WIDTH'(active_x >> 3);
        active_tile_row = TILE_ROW_WIDTH'(active_y >> 3);

        if (latched_active_y == ACTIVE_HEIGHT_LAST) begin
            next_line_active_y = '0;
        end else begin
            next_line_active_y = latched_active_y + 1'b1;
        end
        next_line_tile_row = TILE_ROW_WIDTH'(next_line_active_y >> 3);

        issue_char_request = 1'b0;
        requested_char_map_addr = char_map_addr;
        requested_glyph_row = '0;

        if (active_video &&
                (active_x_in_tile == PREFETCH_TRIGGER_PIXEL) &&
                (active_tile_column != LAST_TILE_COLUMN)) begin
            // At pixel 0 of each tile, start fetching the next tile's font row.
            // Eight cycles later, that row will be waiting in `next_tile_row_bits`
            // when scanout arrives at the next tile boundary.
            issue_char_request = 1'b1;
            requested_char_map_addr =
                make_char_map_addr(active_tile_row, active_tile_column + 1'b1);
            requested_glyph_row = FONT_ROW_INDEX_WIDTH'(active_y[FONT_ROW_INDEX_WIDTH-1:0]);
        end else if (!active_video && active_video_d) begin
            // The first blanking cycle after an active line is used to fetch the
            // first tile of the next line. The module only requires the front
            // porch to be long enough for the full character-map + font-row ROM
            // pipeline.
            issue_char_request = 1'b1;
            requested_char_map_addr = make_char_map_addr(next_line_tile_row, '0);
            requested_glyph_row = FONT_ROW_INDEX_WIDTH'(
                next_line_active_y[FONT_ROW_INDEX_WIDTH-1:0]
            );
        end

        // Generate the value that should appear on the registered output after
        // the next rising edge. This keeps `pixel_on` aligned with the scan
        // coordinates supplied by the upstream timing generator.
        if (!render_active_video_next) begin
            pixel_on_next = 1'b0;
        end else if (render_active_x_in_tile_next == FONT_ROW_INDEX_WIDTH'(0)) begin
            if (font_req_valid_d2) begin
                pixel_on_next = font_glyph_rdata[7];
            end else begin
                pixel_on_next = next_tile_valid ? next_tile_row_bits[7] : 1'b0;
            end
        end else begin
            pixel_on_next =
                current_tile_row_bits[FONT_ROW_INDEX_WIDTH'(7)-render_active_x_in_tile_next];
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            h_counter <= H_LAST;
            v_counter <= V_LAST;
            active_video_d <= 1'b0;
            latched_active_y <= '0;
            next_tile_valid <= 1'b0;
            char_req_valid_d0 <= 1'b0;
            char_req_valid_d1 <= 1'b0;
            char_req_valid_d2 <= 1'b0;
            font_req_valid_d0 <= 1'b0;
            font_req_valid_d1 <= 1'b0;
            font_req_valid_d2 <= 1'b0;
            pixel_on <= 1'b0;
        end else begin
            h_counter <= h_counter_next;
            v_counter <= v_counter_next;
            active_video_d <= active_video;
            pixel_on <= pixel_on_next;

            // The internal timing mirror is what keeps the registered output
            // aligned to the external scan coordinates. Catch any parameter
            // mismatch early in simulation instead of silently rendering the
            // wrong pixels.
            if ((active_video != expected_active_video) ||
                    (line_start != expected_line_start) ||
                    (active_x != expected_active_x) ||
                    (active_y != expected_active_y)) begin
                $fatal(
                    1,
                    "bitmap_text_renderer: input timing does not match configured raster parameters"
                );
            end

            if (active_video) begin
                latched_active_y <= active_y;
            end

            if (issue_char_request) begin
                char_map_addr <= requested_char_map_addr;
            end

            char_req_valid_d0 <= issue_char_request;
            char_req_valid_d1 <= char_req_valid_d0;
            char_req_valid_d2 <= char_req_valid_d1;
            char_req_glyph_row_d0 <= requested_glyph_row;
            char_req_glyph_row_d1 <= char_req_glyph_row_d0;
            char_req_glyph_row_d2 <= char_req_glyph_row_d1;

            if (char_req_valid_d2) begin
                font_addr <= {char_map_rdata, FONT_ROW_INDEX_WIDTH'(char_req_glyph_row_d2)};
            end

            font_req_valid_d0 <= char_req_valid_d2;
            font_req_valid_d1 <= font_req_valid_d0;
            font_req_valid_d2 <= font_req_valid_d1;

            if (font_req_valid_d2) begin
                next_tile_row_bits <= font_glyph_rdata;
                next_tile_valid <= 1'b1;
            end

            // Move the prefetched row into the active row buffer on the cycle
            // that will become tile pixel 0 after this edge. That guarantees
            // pixels 1..7 can come from `current_tile_row_bits` while pixel 0
            // is already available on the registered `pixel_on` output.
            if (render_active_video_next &&
                    (render_active_x_in_tile_next == FONT_ROW_INDEX_WIDTH'(0)) &&
                    font_req_valid_d2) begin
                current_tile_row_bits <= font_glyph_rdata;
            end else if (render_active_video_next &&
                    (render_active_x_in_tile_next == FONT_ROW_INDEX_WIDTH'(0)) &&
                    next_tile_valid) begin
                current_tile_row_bits <= next_tile_row_bits;
            end
        end
    end

endmodule
`default_nettype wire

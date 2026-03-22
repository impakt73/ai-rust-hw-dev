`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles during scanout using a character map ROM and a
// CP437-compatible font ROM.
//
// This module owns the raster timing generator so every externally visible
// timing signal can be registered in the same stage as `pixel_on`. The internal
// `video_sync` instance produces the current scan position, the ROM pipeline
// computes the pixel value for that position, and a final register stage delays
// the timing bundle plus `pixel_on` together. That keeps the output interface
// self-consistent for downstream video users while still giving the FPGA an
// extra timing-breaking register on the pixel path.
//
// The delayed timing outputs are shaped to match the Analogue Pocket video
// control interface:
//   - `video_de` is the delayed active-video/display-enable qualifier
//   - `video_hs` / `video_vs` are the delayed sync pulses
// The module also forwards delayed scan coordinates and start-of-line/frame
// pulses because they are useful to internal consumers and focused tests.

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
    parameter string FONT_INIT_FILE = "",
    parameter string CHAR_MAP_INIT_FILE = ""
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
    output logic pixel_on
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_ROM_ADDR_WIDTH = 8 + FONT_ROW_INDEX_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    localparam int unsigned FONT_PIPELINE_CYCLES = 8;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH;
    localparam int unsigned TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT;
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    localparam logic [ACTIVE_Y_WIDTH-1:0] ACTIVE_HEIGHT_LAST = ACTIVE_Y_WIDTH'(ACTIVE_HEIGHT - 1);
    localparam logic [TILE_COLUMN_WIDTH-1:0] LAST_TILE_COLUMN =
        TILE_COLUMN_WIDTH'(TILE_COLUMNS - 1);
    localparam logic [2:0] PREFETCH_TRIGGER_PIXEL = 3'd0;

    logic sync_video_de;
    logic sync_video_hs;
    logic sync_video_vs;
    logic sync_line_start;
    logic sync_frame_start;
    logic [ACTIVE_X_WIDTH-1:0] sync_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] sync_active_y;

    logic [CHARMAP_ADDR_WIDTH-1:0] char_map_addr;
    logic [CHARMAP_DATA_WIDTH-1:0] char_map_rdata;
    logic [FONT_ROM_ADDR_WIDTH-1:0] font_addr;
    logic [FONT_ROM_DATA_WIDTH-1:0] font_glyph_rdata;

    logic sync_video_de_d;
    logic [ACTIVE_Y_WIDTH-1:0] latched_active_y;
    logic [7:0] current_tile_row_bits;
    logic [7:0] next_tile_row_bits;
    logic next_tile_valid;
    logic pixel_on_next;
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

    function automatic logic [CHARMAP_ADDR_WIDTH-1:0] make_char_map_addr(
        input logic [TILE_ROW_WIDTH-1:0] tile_row,
        input logic [TILE_COLUMN_WIDTH-1:0] tile_column
    );
        int unsigned char_map_index;

        char_map_index = (tile_row * TILE_COLUMNS) + int'(tile_column);
        make_char_map_addr = CHARMAP_ADDR_WIDTH'(char_map_index);
    endfunction

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
        .active_y(sync_active_y)
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
        if (H_FRONT_PORCH < FONT_PIPELINE_CYCLES) begin
            $fatal(1, "bitmap_text_renderer: H_FRONT_PORCH must be >= %0d", FONT_PIPELINE_CYCLES);
        end
    end

    always_comb begin
        active_x_in_tile = sync_active_x[2:0];
        active_tile_column = TILE_COLUMN_WIDTH'(sync_active_x >> 3);
        active_tile_row = TILE_ROW_WIDTH'(sync_active_y >> 3);

        if (latched_active_y == ACTIVE_HEIGHT_LAST) begin
            next_line_active_y = '0;
        end else begin
            next_line_active_y = latched_active_y + 1'b1;
        end
        next_line_tile_row = TILE_ROW_WIDTH'(next_line_active_y >> 3);

        issue_char_request = 1'b0;
        requested_char_map_addr = char_map_addr;
        requested_glyph_row = FONT_ROW_INDEX_WIDTH'(0);

        // Fetch ahead of the currently displayed tile so the synchronous
        // character-map ROM and font ROM have enough latency budget to return
        // the next 8-pixel row before scanout reaches the next tile boundary.
        if (sync_video_de &&
                (active_x_in_tile == PREFETCH_TRIGGER_PIXEL) &&
                (active_tile_column != LAST_TILE_COLUMN)) begin
            issue_char_request = 1'b1;
            requested_char_map_addr =
                make_char_map_addr(active_tile_row, active_tile_column + 1'b1);
            requested_glyph_row =
                FONT_ROW_INDEX_WIDTH'(sync_active_y[FONT_ROW_INDEX_WIDTH-1:0]);
        end else if (!sync_video_de && sync_video_de_d) begin
            // The first blanking cycle after an active line is used to fetch the
            // first tile row of the next line. The required front porch matches
            // the total character-map + font ROM pipeline depth.
            issue_char_request = 1'b1;
            requested_char_map_addr = make_char_map_addr(next_line_tile_row, '0);
            requested_glyph_row = FONT_ROW_INDEX_WIDTH'(
                next_line_active_y[FONT_ROW_INDEX_WIDTH-1:0]
            );
        end

        if (!sync_video_de) begin
            pixel_on_next = 1'b0;
        end else if (active_x_in_tile == FONT_ROW_INDEX_WIDTH'(0)) begin
            // Pixel 0 of each tile uses the just-arrived font row directly when
            // possible, otherwise the row previously prefetched into
            // `next_tile_row_bits` is reused. Pixels 1..7 come from the
            // `current_tile_row_bits` shift-like lookup below.
            if (font_req_valid_d2) begin
                pixel_on_next = font_glyph_rdata[7];
            end else begin
                pixel_on_next = next_tile_valid ? next_tile_row_bits[7] : 1'b0;
            end
        end else begin
            pixel_on_next = current_tile_row_bits[FONT_ROW_INDEX_WIDTH'(7)-active_x_in_tile];
        end
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
            sync_video_de_d <= 1'b0;
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
            // Delay the full timing bundle by one cycle so the registered pixel
            // output and the externally visible video timing stay aligned.
            video_de <= sync_video_de;
            video_hs <= sync_video_hs;
            video_vs <= sync_video_vs;
            line_start <= sync_line_start;
            frame_start <= sync_frame_start;
            active_x <= sync_active_x;
            active_y <= sync_active_y;
            pixel_on <= pixel_on_next;
            sync_video_de_d <= sync_video_de;

            if (sync_video_de) begin
                latched_active_y <= sync_active_y;
            end

            if (sync_line_start) begin
                current_tile_row_bits <= '0;
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

            // Move the prefetched tile row into the active buffer on the cycle
            // where the current scan position is tile pixel 0. That way the
            // output register captures pixel 0 on this edge, while pixels 1..7
            // are ready in `current_tile_row_bits` for the following cycles.
            if (sync_video_de &&
                    (active_x_in_tile == FONT_ROW_INDEX_WIDTH'(0)) &&
                    font_req_valid_d2) begin
                current_tile_row_bits <= font_glyph_rdata;
            end else if (sync_video_de &&
                    (active_x_in_tile == FONT_ROW_INDEX_WIDTH'(0)) &&
                    next_tile_valid) begin
                current_tile_row_bits <= next_tile_row_bits;
            end
        end
    end

endmodule
`default_nettype wire

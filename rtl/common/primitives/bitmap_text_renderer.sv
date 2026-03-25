`default_nettype none
// Bitmap Text Renderer
// Renders 8x8 character tiles during scanout using a character map ROM and a
// CP437-compatible font ROM.
//
// This module owns the raster timing generator so every externally visible
// timing signal can be registered in the same stage as `pixel_on`. The internal
// `video_sync` instance produces the current scan position, the ROM pipeline
// continuously fetches the character/font byte for each pixel position, and a
// final register stage delays the timing bundle plus `pixel_on` together. That
// keeps the output interface self-consistent for downstream video users while
// still giving the FPGA an extra timing-breaking register on the pixel path.
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
    output logic pixel_on
);

    localparam int unsigned FONT_ROM_DATA_WIDTH = 8;
    localparam int unsigned FONT_ROW_INDEX_WIDTH = (TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT);
    localparam int unsigned FONT_ROM_ADDR_WIDTH = 8 + FONT_ROW_INDEX_WIDTH;
    localparam int unsigned CHARMAP_DATA_WIDTH = 8;
    // Front-porch budget for the renderer's continuous per-pixel ROM pipeline:
    //   - 2 cycles: character-map sync_sprom latency
    //   - 2 cycles: font-row sync_sprom latency after font_addr is issued
    // Horizontal blanking must cover this 4-cycle latency so tile 0 of the next
    // line is ready before scanout re-enters the active region.
    localparam int unsigned FONT_PIPELINE_CYCLES = 4;
    localparam int unsigned ACTIVE_X_WIDTH = (ACTIVE_WIDTH <= 1) ? 1 : $clog2(ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH = (ACTIVE_HEIGHT <= 1) ? 1 : $clog2(ACTIVE_HEIGHT);
    localparam int unsigned TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH;
    localparam int unsigned TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT;
    localparam int unsigned TILE_COLUMN_WIDTH = (TILE_COLUMNS <= 1) ? 1 : $clog2(TILE_COLUMNS);
    localparam int unsigned TILE_ROW_WIDTH = (TILE_ROWS <= 1) ? 1 : $clog2(TILE_ROWS);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH = (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    localparam logic [ACTIVE_Y_WIDTH-1:0] ACTIVE_HEIGHT_LAST = ACTIVE_Y_WIDTH'(ACTIVE_HEIGHT - 1);

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

    logic [ACTIVE_Y_WIDTH-1:0] latched_active_y;
    logic pixel_on_next;
    logic [3:0] video_de_pipe;
    logic [3:0] video_hs_pipe;
    logic [3:0] video_vs_pipe;
    logic [3:0] line_start_pipe;
    logic [3:0] frame_start_pipe;
    logic [ACTIVE_X_WIDTH-1:0] active_x_d0;
    logic [ACTIVE_X_WIDTH-1:0] active_x_d1;
    logic [ACTIVE_X_WIDTH-1:0] active_x_d2;
    logic [ACTIVE_X_WIDTH-1:0] active_x_d3;
    logic [ACTIVE_Y_WIDTH-1:0] active_y_d0;
    logic [ACTIVE_Y_WIDTH-1:0] active_y_d1;
    logic [ACTIVE_Y_WIDTH-1:0] active_y_d2;
    logic [ACTIVE_Y_WIDTH-1:0] active_y_d3;
    logic [FONT_ROW_INDEX_WIDTH-1:0] glyph_row_d0;
    logic [FONT_ROW_INDEX_WIDTH-1:0] glyph_row_d1;
    logic [TILE_COLUMN_WIDTH-1:0] active_tile_column;
    logic [TILE_ROW_WIDTH-1:0] active_tile_row;
    logic [ACTIVE_Y_WIDTH-1:0] next_line_active_y;
    logic [TILE_ROW_WIDTH-1:0] next_line_tile_row;
    logic [FONT_ROW_INDEX_WIDTH-1:0] font_glyph_row;
    logic [FONT_ROW_INDEX_WIDTH-1:0] pixel_bit_index;

    function automatic logic [CHARMAP_ADDR_WIDTH-1:0] make_char_map_addr(
        input logic [TILE_ROW_WIDTH-1:0] tile_row,
        input logic [TILE_COLUMN_WIDTH-1:0] tile_column
    );
        int unsigned char_map_index;

        char_map_index = (tile_row * TILE_COLUMNS) + 32'(tile_column);
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
        if (H_FRONT_PORCH < FONT_PIPELINE_CYCLES) begin
            $fatal(1, "bitmap_text_renderer: H_FRONT_PORCH must be >= %0d", FONT_PIPELINE_CYCLES);
        end
    end
`endif

    always_comb begin
        active_tile_column = TILE_COLUMN_WIDTH'(sync_active_x >> 3);
        active_tile_row = TILE_ROW_WIDTH'(sync_active_y >> 3);

        if (latched_active_y == ACTIVE_HEIGHT_LAST) begin
            next_line_active_y = '0;
        end else begin
            next_line_active_y = latched_active_y + 1'b1;
        end
        next_line_tile_row = TILE_ROW_WIDTH'(next_line_active_y >> 3);

        // During active scanout, issue a fetch for the current pixel's tile.
        // During blanking, continuously fetch tile 0 of the upcoming line so
        // the guaranteed front porch covers the full 4-cycle ROM pipeline.
        if (sync_video_de) begin
            char_map_addr = make_char_map_addr(active_tile_row, active_tile_column);
            font_glyph_row = FONT_ROW_INDEX_WIDTH'(sync_active_y[FONT_ROW_INDEX_WIDTH-1:0]);
        end else begin
            char_map_addr = make_char_map_addr(next_line_tile_row, '0);
            font_glyph_row = FONT_ROW_INDEX_WIDTH'(
                next_line_active_y[FONT_ROW_INDEX_WIDTH-1:0]
            );
        end

        font_addr = {char_map_rdata, glyph_row_d1};
        pixel_bit_index = FONT_ROW_INDEX_WIDTH'(7) - active_x_d3[FONT_ROW_INDEX_WIDTH-1:0];
        pixel_on_next = video_de_pipe[3] ? font_glyph_rdata[pixel_bit_index] : 1'b0;
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
            video_de_pipe <= '0;
            video_hs_pipe <= {4{~HSYNC_ACTIVE_HIGH}};
            video_vs_pipe <= {4{~VSYNC_ACTIVE_HIGH}};
            line_start_pipe <= '0;
            frame_start_pipe <= '0;
            active_x_d0 <= '0;
            active_x_d1 <= '0;
            active_x_d2 <= '0;
            active_x_d3 <= '0;
            active_y_d0 <= '0;
            active_y_d1 <= '0;
            active_y_d2 <= '0;
            active_y_d3 <= '0;
            glyph_row_d0 <= '0;
            glyph_row_d1 <= '0;
            latched_active_y <= ACTIVE_HEIGHT_LAST;
            pixel_on <= 1'b0;
        end else begin
            video_de_pipe <= {video_de_pipe[2:0], sync_video_de};
            video_hs_pipe <= {video_hs_pipe[2:0], sync_video_hs};
            video_vs_pipe <= {video_vs_pipe[2:0], sync_video_vs};
            line_start_pipe <= {line_start_pipe[2:0], sync_line_start};
            frame_start_pipe <= {frame_start_pipe[2:0], sync_frame_start};
            active_x_d0 <= sync_active_x;
            active_x_d1 <= active_x_d0;
            active_x_d2 <= active_x_d1;
            active_x_d3 <= active_x_d2;
            active_y_d0 <= sync_active_y;
            active_y_d1 <= active_y_d0;
            active_y_d2 <= active_y_d1;
            active_y_d3 <= active_y_d2;
            glyph_row_d0 <= font_glyph_row;
            glyph_row_d1 <= glyph_row_d0;
            if (sync_video_de) begin
                latched_active_y <= sync_active_y;
            end
            video_de <= video_de_pipe[3];
            video_hs <= video_hs_pipe[3];
            video_vs <= video_vs_pipe[3];
            line_start <= line_start_pipe[3];
            frame_start <= frame_start_pipe[3];
            active_x <= active_x_d3;
            active_y <= active_y_d3;
            pixel_on <= pixel_on_next;
        end
    end

endmodule
`default_nettype wire

`default_nettype none

module gfx2d_peripheral #(
    parameter int unsigned VIDEO_ACTIVE_WIDTH = 256,
    parameter int unsigned VIDEO_ACTIVE_HEIGHT = 224,
    parameter int unsigned VIDEO_H_FRONT_PORCH = 10,
    parameter int unsigned VIDEO_H_SYNC_WIDTH = 1,
    parameter int unsigned VIDEO_H_BACK_PORCH = 133,
    parameter int unsigned VIDEO_V_FRONT_PORCH = 10,
    parameter int unsigned VIDEO_V_SYNC_WIDTH = 1,
    parameter int unsigned VIDEO_V_BACK_PORCH = 277,
    parameter bit VIDEO_HSYNC_ACTIVE_HIGH = 1'b1,
    parameter bit VIDEO_VSYNC_ACTIVE_HIGH = 1'b1,
    parameter int unsigned TILE_WIDTH = 8,
    parameter int unsigned TILE_HEIGHT = 8,
    parameter int unsigned TILE_COLUMNS = 32,
    parameter int unsigned TILE_ROWS = 32,
    parameter int unsigned BUS_CDC_SYNC_STAGES = 3
) (
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
    output logic             video_hs,
    output logic             video_vs,
    output logic             video_skip
);

    localparam logic [15:0] REG_SCROLL_X = 16'h0000;
    localparam logic [15:0] REG_SCROLL_Y = 16'h0004;
    localparam logic [15:0] REG_CONTROL = 16'h0008;
    localparam logic [15:0] REG_FRAME_INDEX = 16'h000C;
    localparam logic [15:0] CHAR_MAP_BASE_OFFSET = 16'h1000;
    localparam logic [15:0] FONT_BASE_OFFSET = 16'h2000;
    localparam logic [15:0] PALETTE_BASE_OFFSET = 16'h6000;
    localparam logic [63:0] PERIPHERAL_APERTURE_BYTES = 64'd65536;
    // bitmap_text_renderer contributes 8 cycles to video_rgb, then this module
    // registers all four video outputs together for the final aligned stage.
    localparam int unsigned VIDEO_SIGNAL_DELAY_CYCLES = 8;
    localparam int unsigned ACTIVE_X_WIDTH =
        (VIDEO_ACTIVE_WIDTH <= 1) ? 1 : $clog2(VIDEO_ACTIVE_WIDTH);
    localparam int unsigned ACTIVE_Y_WIDTH =
        (VIDEO_ACTIVE_HEIGHT <= 1) ? 1 : $clog2(VIDEO_ACTIVE_HEIGHT);
    localparam int unsigned CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS;
    localparam int unsigned CHARMAP_ADDR_WIDTH =
        (CHARMAP_DEPTH <= 1) ? 1 : $clog2(CHARMAP_DEPTH);
    localparam int unsigned FONT_ADDR_WIDTH =
        8 + (((TILE_HEIGHT <= 1) ? 1 : $clog2(TILE_HEIGHT)) +
        ((TILE_WIDTH <= 1) ? 1 : $clog2(TILE_WIDTH)));
    localparam int unsigned FONT_DEPTH = (1 << FONT_ADDR_WIDTH);
    localparam int unsigned PALETTE_ADDR_WIDTH = 8;
    localparam int unsigned PALETTE_DEPTH = (1 << PALETTE_ADDR_WIDTH);
    localparam int unsigned CHARMAP_SPAN_BYTES = CHARMAP_DEPTH;
    localparam int unsigned FONT_SPAN_BYTES = FONT_DEPTH;
    localparam int unsigned PALETTE_SPAN_BYTES = PALETTE_DEPTH * 4;
    localparam logic [63:0] CHAR_MAP_BASE_OFFSET_U = {48'h0, CHAR_MAP_BASE_OFFSET};
    localparam logic [63:0] FONT_BASE_OFFSET_U = {48'h0, FONT_BASE_OFFSET};
    localparam logic [63:0] PALETTE_BASE_OFFSET_U = {48'h0, PALETTE_BASE_OFFSET};
    localparam logic [15:0] CHAR_MAP_END_OFFSET = 16'(CHAR_MAP_BASE_OFFSET) + 16'(CHARMAP_DEPTH);
    localparam logic [15:0] FONT_END_OFFSET = 16'(FONT_BASE_OFFSET) + 16'(FONT_DEPTH);
    localparam logic [15:0] PALETTE_END_OFFSET = 16'(PALETTE_BASE_OFFSET) + 16'(PALETTE_DEPTH * 4);
    localparam int unsigned SCROLL_X_WIDTH =
        ((TILE_WIDTH * TILE_COLUMNS) <= 1) ? 1 : $clog2(TILE_WIDTH * TILE_COLUMNS);
    localparam int unsigned SCROLL_Y_WIDTH =
        ((TILE_HEIGHT * TILE_ROWS) <= 1) ? 1 : $clog2(TILE_HEIGHT * TILE_ROWS);

    logic reset_n_video_sync;
    logic video_rst;

    logic [31:0] periph_mem_a_addr;
    logic [31:0] periph_mem_a_wdata;
    logic        periph_mem_a_we;
    logic [1:0]  periph_mem_a_size;
    logic        periph_mem_a_valid;
    logic        periph_mem_a_ready;
    logic [31:0] periph_mem_d_rdata;
    logic        periph_mem_d_valid;
    logic        periph_mem_d_ready;
    logic [15:0] periph_addr_offset;
    logic        periph_byte_access;
    logic        periph_word_access;
    logic        periph_reg_access;
    logic        periph_char_map_access;
    logic        periph_font_access;
    logic        periph_palette_access;

    logic        periph_mem_a_handshake;
    logic        periph_mem_d_handshake;
    logic [31:0] response_data;
    logic        response_pending;
    logic [31:0] scroll_x_reg;
    logic [31:0] scroll_y_reg;
    logic        video_enable_reg;
    logic        renderer_video_enable;
    logic [31:0] frame_index_reg;
    logic [SCROLL_X_WIDTH-1:0] renderer_scroll_x;
    logic [SCROLL_Y_WIDTH-1:0] renderer_scroll_y;

    logic        sync_video_de;
    logic        sync_video_hs;
    logic        sync_video_vs;
    logic        sync_frame_start;
    logic        sync_vblank_start;
    logic [ACTIVE_X_WIDTH-1:0] sync_active_x;
    logic [ACTIVE_Y_WIDTH-1:0] sync_active_y;
    logic [CHARMAP_ADDR_WIDTH-1:0] char_ram_waddr;
    logic [CHARMAP_ADDR_WIDTH-1:0] char_ram_raddr;
    logic [7:0] char_ram_wdata;
    logic       char_ram_we;
    logic [CHARMAP_ADDR_WIDTH-1:0] char_mem_addr;
    logic [7:0] char_mem_rdata;
    logic [FONT_ADDR_WIDTH-1:0] font_ram_waddr;
    logic [FONT_ADDR_WIDTH-1:0] font_ram_raddr;
    logic [7:0] font_ram_wdata;
    logic       font_ram_we;
    logic [FONT_ADDR_WIDTH-1:0] font_mem_addr;
    logic [7:0] font_mem_rdata;
    logic [PALETTE_ADDR_WIDTH-1:0] palette_ram_waddr;
    logic [PALETTE_ADDR_WIDTH-1:0] palette_ram_raddr;
    logic [23:0] palette_ram_wdata;
    logic        palette_ram_we;
    logic [7:0] palette_mem_addr;
    logic [23:0] palette_mem_rdata;
    logic [23:0] renderer_video_rgb;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_de_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_hs_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] video_vs_pipe;

`ifndef SYNTHESIS
    initial begin
        if (!(CHAR_MAP_BASE_OFFSET < FONT_BASE_OFFSET)) begin
            $fatal(1, "gfx2d_peripheral requires char map window base before font window base");
        end
        if (!(FONT_BASE_OFFSET < PALETTE_BASE_OFFSET)) begin
            $fatal(1, "gfx2d_peripheral requires font window base before palette window base");
        end
        if ((CHAR_MAP_BASE_OFFSET_U + 64'(CHARMAP_SPAN_BYTES)) > FONT_BASE_OFFSET_U) begin
            $fatal(1, "gfx2d_peripheral char map window overlaps font window");
        end
        if ((FONT_BASE_OFFSET_U + 64'(FONT_SPAN_BYTES)) > PALETTE_BASE_OFFSET_U) begin
            $fatal(1, "gfx2d_peripheral font window overlaps palette window");
        end
        if ((PALETTE_BASE_OFFSET_U + 64'(PALETTE_SPAN_BYTES)) > PERIPHERAL_APERTURE_BYTES) begin
            $fatal(1, "gfx2d_peripheral palette window exceeds 64 KiB peripheral aperture");
        end
    end
`endif

    assign periph_mem_a_handshake = periph_mem_a_valid && periph_mem_a_ready;
    assign periph_mem_d_handshake = periph_mem_d_valid && periph_mem_d_ready;
    assign periph_addr_offset = periph_mem_a_addr[15:0];
    // Keep the MMIO path single-issue so the zero/non-zero response payload
    // stays paired with exactly one accepted request.
    assign periph_mem_a_ready = !video_rst && !response_pending;
    assign periph_mem_d_rdata = response_data;
    assign periph_mem_d_valid = response_pending;
    assign periph_byte_access = (periph_mem_a_size == 2'b00);
    assign periph_word_access = (periph_mem_a_size == 2'b10) && (periph_mem_a_addr[1:0] == 2'b00);
    assign periph_reg_access = periph_word_access && (periph_addr_offset < CHAR_MAP_BASE_OFFSET);
    assign periph_char_map_access =
        periph_byte_access &&
        (periph_addr_offset >= CHAR_MAP_BASE_OFFSET) &&
        (periph_addr_offset < CHAR_MAP_END_OFFSET);
    assign periph_font_access =
        periph_byte_access &&
        (periph_addr_offset >= FONT_BASE_OFFSET) &&
        (periph_addr_offset < FONT_END_OFFSET);
    assign periph_palette_access =
        periph_word_access &&
        (periph_addr_offset >= PALETTE_BASE_OFFSET) &&
        (periph_addr_offset < PALETTE_END_OFFSET);

    assign char_ram_we = periph_mem_a_handshake && periph_mem_a_we && periph_char_map_access;
    assign char_ram_waddr = CHARMAP_ADDR_WIDTH'(periph_addr_offset - CHAR_MAP_BASE_OFFSET);
    assign char_ram_wdata = periph_mem_a_wdata[7:0];
    assign char_ram_raddr = char_mem_addr;

    assign font_ram_we = periph_mem_a_handshake && periph_mem_a_we && periph_font_access;
    assign font_ram_waddr = FONT_ADDR_WIDTH'(periph_addr_offset - FONT_BASE_OFFSET);
    assign font_ram_wdata = periph_mem_a_wdata[7:0];
    assign font_ram_raddr = font_mem_addr;

    assign palette_ram_we = periph_mem_a_handshake && periph_mem_a_we && periph_palette_access;
    assign palette_ram_waddr = PALETTE_ADDR_WIDTH'((periph_addr_offset - PALETTE_BASE_OFFSET) >> 2);
    assign palette_ram_wdata = periph_mem_a_wdata[23:0];
    assign palette_ram_raddr = palette_mem_addr;

    assign video_skip = 1'b0;

    ff_sync #(
        .STAGES(BUS_CDC_SYNC_STAGES),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) video_reset_sync (
        .clk(video_clk),
        .rst(1'b0),
        .din(!rst),
        .dout(reset_n_video_sync)
    );

    assign video_rst = !reset_n_video_sync;

    bus_cdc_bridge #(
        .ADDR_WIDTH(32),
        .DATA_WIDTH(32),
        .SIZE_WIDTH(2),
        .SYNC_STAGES(BUS_CDC_SYNC_STAGES)
    ) u_bus_cdc_bridge (
        .sys_clk(sys_clk),
        .periph_clk(video_clk),
        .sys_rst(rst),
        .periph_rst(video_rst),
        .sys_mem_a_addr(mem_a_addr),
        .sys_mem_a_wdata(mem_a_wdata),
        .sys_mem_a_we(mem_a_we),
        .sys_mem_a_size(mem_a_size),
        .sys_mem_a_valid(mem_a_valid),
        .sys_mem_a_ready(mem_a_ready),
        .sys_mem_d_rdata(mem_d_rdata),
        .sys_mem_d_valid(mem_d_valid),
        .sys_mem_d_ready(mem_d_ready),
        .periph_mem_a_addr(periph_mem_a_addr),
        .periph_mem_a_wdata(periph_mem_a_wdata),
        .periph_mem_a_we(periph_mem_a_we),
        .periph_mem_a_size(periph_mem_a_size),
        .periph_mem_a_valid(periph_mem_a_valid),
        .periph_mem_a_ready(periph_mem_a_ready),
        .periph_mem_d_rdata(periph_mem_d_rdata),
        .periph_mem_d_valid(periph_mem_d_valid),
        .periph_mem_d_ready(periph_mem_d_ready)
    );

    video_sync #(
        .H_ACTIVE(VIDEO_ACTIVE_WIDTH),
        .H_FRONT_PORCH(VIDEO_H_FRONT_PORCH),
        .H_SYNC_WIDTH(VIDEO_H_SYNC_WIDTH),
        .H_BACK_PORCH(VIDEO_H_BACK_PORCH),
        .V_ACTIVE(VIDEO_ACTIVE_HEIGHT),
        .V_FRONT_PORCH(VIDEO_V_FRONT_PORCH),
        .V_SYNC_WIDTH(VIDEO_V_SYNC_WIDTH),
        .V_BACK_PORCH(VIDEO_V_BACK_PORCH),
        .HSYNC_ACTIVE_HIGH(VIDEO_HSYNC_ACTIVE_HIGH),
        .VSYNC_ACTIVE_HIGH(VIDEO_VSYNC_ACTIVE_HIGH)
    ) u_video_sync (
        .clk(video_clk),
        .rst(video_rst),
        .hsync(sync_video_hs),
        .vsync(sync_video_vs),
        .active_video(sync_video_de),
        .line_start(),
        .frame_start(sync_frame_start),
        .hblank_start(),
        .vblank_start(sync_vblank_start),
        .active_x(sync_active_x),
        .active_y(sync_active_y),
        .scan_x(),
        .scan_y()
    );

    sync_dpram #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(CHARMAP_ADDR_WIDTH)
    ) u_char_map_ram (
        .wclk(video_clk),
        .rclk(video_clk),
        .we(char_ram_we),
        .waddr(char_ram_waddr),
        .wdata(char_ram_wdata),
        .raddr(char_ram_raddr),
        .rdata(char_mem_rdata)
    );

    sync_dpram #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(FONT_ADDR_WIDTH)
    ) u_font_ram (
        .wclk(video_clk),
        .rclk(video_clk),
        .we(font_ram_we),
        .waddr(font_ram_waddr),
        .wdata(font_ram_wdata),
        .raddr(font_ram_raddr),
        .rdata(font_mem_rdata)
    );

    sync_dpram #(
        .DATA_WIDTH(24),
        .ADDR_WIDTH(PALETTE_ADDR_WIDTH)
    ) u_palette_ram (
        .wclk(video_clk),
        .rclk(video_clk),
        .we(palette_ram_we),
        .waddr(palette_ram_waddr),
        .wdata(palette_ram_wdata),
        .raddr(palette_ram_raddr),
        .rdata(palette_mem_rdata)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .TILE_WIDTH(TILE_WIDTH),
        .TILE_HEIGHT(TILE_HEIGHT),
        .TILE_COLUMNS(TILE_COLUMNS),
        .TILE_ROWS(TILE_ROWS)
    ) u_bitmap_text_renderer (
        .clk(video_clk),
        .rst(video_rst),
        .screen_x(sync_active_x),
        .screen_y(sync_active_y),
        .scroll_x(renderer_scroll_x),
        .scroll_y(renderer_scroll_y),
        .char_mem_addr(char_mem_addr),
        .char_mem_rdata(char_mem_rdata),
        .font_mem_addr(font_mem_addr),
        .font_mem_rdata(font_mem_rdata),
        .palette_mem_addr(palette_mem_addr),
        .palette_mem_rdata(palette_mem_rdata),
        .video_rgb(renderer_video_rgb)
    );

    always_ff @(posedge video_clk) begin
        if (video_rst) begin
            scroll_x_reg <= 32'h0000_0000;
            scroll_y_reg <= 32'h0000_0000;
            video_enable_reg <= 1'b0;
            renderer_video_enable <= 1'b0;
            frame_index_reg <= 32'h0000_0000;
            renderer_scroll_x <= '0;
            renderer_scroll_y <= '0;
            response_pending <= 1'b0;
            video_de_pipe <= '0;
            video_hs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VIDEO_HSYNC_ACTIVE_HIGH}};
            video_vs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VIDEO_VSYNC_ACTIVE_HIGH}};
            video_de <= 1'b0;
            video_hs <= ~VIDEO_HSYNC_ACTIVE_HIGH;
            video_vs <= ~VIDEO_VSYNC_ACTIVE_HIGH;
            video_rgb <= 24'h00_00_00;
        end else begin
            video_de_pipe <= {video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_de};
            video_hs_pipe <= {video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_hs};
            video_vs_pipe <= {video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_vs};
            video_de <= video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
            video_hs <= video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
            video_vs <= video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
            video_rgb <=
                (renderer_video_enable && video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1]) ?
                renderer_video_rgb :
                24'h00_00_00;

            if (sync_frame_start) begin
                frame_index_reg <= frame_index_reg + 1'b1;
            end

            if (periph_mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (sync_vblank_start) begin
                renderer_scroll_x <= scroll_x_reg[SCROLL_X_WIDTH-1:0];
                renderer_scroll_y <= scroll_y_reg[SCROLL_Y_WIDTH-1:0];
                renderer_video_enable <= video_enable_reg;
            end

            if (periph_mem_a_handshake) begin
                // response_data intentionally is not reset because response_pending
                // marks when the payload is meaningful.
                response_data <= 32'h0000_0000;
                response_pending <= 1'b1;

                if (periph_reg_access) begin
                    if (periph_mem_a_we) begin
                        case (periph_addr_offset)
                            REG_SCROLL_X: scroll_x_reg <= periph_mem_a_wdata;
                            REG_SCROLL_Y: scroll_y_reg <= periph_mem_a_wdata;
                            REG_CONTROL: video_enable_reg <= periph_mem_a_wdata[0];
                            default: begin
                            end
                        endcase
                    end else begin
                        case (periph_addr_offset)
                            REG_SCROLL_X: response_data <= scroll_x_reg;
                            REG_SCROLL_Y: response_data <= scroll_y_reg;
                            REG_CONTROL: response_data <= {31'h0000_0000, video_enable_reg};
                            REG_FRAME_INDEX: response_data <= frame_index_reg;
                            default: response_data <= 32'h0000_0000;
                        endcase
                    end
                end
            end
        end
    end

endmodule

`default_nettype wire

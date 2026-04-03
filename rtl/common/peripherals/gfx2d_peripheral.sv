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
    parameter int unsigned BUS_CDC_SYNC_STAGES = 3,
    parameter FONT_INIT_FILE = "rtl/common/wrappers/bitmap_text_renderer_font_init.hex",
    parameter CHAR_MAP_INIT_FILE = "rtl/common/wrappers/bitmap_text_renderer_char_map_init.hex",
    parameter PALETTE_INIT_FILE = "rtl/common/wrappers/bitmap_text_renderer_palette_init.hex"
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

    localparam logic [4:0] REG_SCROLL_X = 5'h00;
    localparam logic [4:0] REG_SCROLL_Y = 5'h04;
    localparam int unsigned VIDEO_SIGNAL_DELAY_CYCLES = 9;
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

    logic        periph_mem_a_handshake;
    logic        periph_mem_d_handshake;
    logic [31:0] response_data;
    logic        response_pending;
    logic [31:0] scroll_x_reg;
    logic [31:0] scroll_y_reg;

    logic        sync_video_de;
    logic        sync_video_hs;
    logic        sync_video_vs;
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

    function automatic logic [31:0] apply_write_mask(
        input logic [31:0] current_value,
        input logic [31:0] write_value,
        input logic [1:0]  access_size,
        input logic [1:0]  byte_offset
    );
        logic [31:0] shifted_wdata;
        logic [31:0] write_mask;
        begin
            shifted_wdata = write_value << {byte_offset, 3'b000};

            case (access_size)
                2'b00: write_mask = 32'h0000_00FF << {byte_offset, 3'b000};
                2'b01: write_mask = 32'h0000_FFFF << {byte_offset, 3'b000};
                2'b10: write_mask = 32'hFFFF_FFFF << {byte_offset, 3'b000};
                default: write_mask = 32'h0000_0000;
            endcase

            apply_write_mask = (current_value & ~write_mask) | (shifted_wdata & write_mask);
        end
    endfunction

    assign periph_mem_a_handshake = periph_mem_a_valid && periph_mem_a_ready;
    assign periph_mem_d_handshake = periph_mem_d_valid && periph_mem_d_ready;
    assign periph_mem_a_ready = !response_pending;
    assign periph_mem_d_rdata = response_data;
    assign periph_mem_d_valid = response_pending;

    assign video_de = video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_hs = video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_vs = video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign video_rgb = video_de ? renderer_video_rgb : 24'h00_00_00;
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
        .rst(rst),
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
        .frame_start(),
        .hblank_start(),
        .vblank_start(),
        .active_x(sync_active_x),
        .active_y(sync_active_y),
        .scan_x(),
        .scan_y()
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(CHARMAP_ADDR_WIDTH),
        .INIT_FILE(CHAR_MAP_INIT_FILE)
    ) u_char_map_rom (
        .clk(video_clk),
        .addr(char_mem_addr),
        .rdata(char_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(FONT_ADDR_WIDTH),
        .INIT_FILE(FONT_INIT_FILE)
    ) u_font_rom (
        .clk(video_clk),
        .addr(font_mem_addr),
        .rdata(font_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(24),
        .ADDR_WIDTH(8),
        .INIT_FILE(PALETTE_INIT_FILE)
    ) u_palette_rom (
        .clk(video_clk),
        .addr(palette_mem_addr),
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
        .scroll_x(scroll_x_reg[SCROLL_X_WIDTH-1:0]),
        .scroll_y(scroll_y_reg[SCROLL_Y_WIDTH-1:0]),
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
            response_data <= 32'h0000_0000;
            response_pending <= 1'b0;
            video_de_pipe <= '0;
            video_hs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VIDEO_HSYNC_ACTIVE_HIGH}};
            video_vs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{~VIDEO_VSYNC_ACTIVE_HIGH}};
        end else begin
            video_de_pipe <= {video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_de};
            video_hs_pipe <= {video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_hs};
            video_vs_pipe <= {video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0], sync_video_vs};

            if (periph_mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (periph_mem_a_handshake) begin
                response_data <= 32'h0000_0000;
                response_pending <= 1'b1;

                if (periph_mem_a_we) begin
                    case (periph_mem_a_addr[4:0])
                        REG_SCROLL_X: begin
                            scroll_x_reg <= apply_write_mask(
                                scroll_x_reg,
                                periph_mem_a_wdata,
                                periph_mem_a_size,
                                periph_mem_a_addr[1:0]
                            );
                        end
                        REG_SCROLL_Y: begin
                            scroll_y_reg <= apply_write_mask(
                                scroll_y_reg,
                                periph_mem_a_wdata,
                                periph_mem_a_size,
                                periph_mem_a_addr[1:0]
                            );
                        end
                        default: begin
                        end
                    endcase
                end else begin
                    case (periph_mem_a_addr[4:0])
                        REG_SCROLL_X: response_data <= scroll_x_reg;
                        REG_SCROLL_Y: response_data <= scroll_y_reg;
                        default: response_data <= 32'h0000_0000;
                    endcase
                end
            end
        end
    end

endmodule

`default_nettype wire

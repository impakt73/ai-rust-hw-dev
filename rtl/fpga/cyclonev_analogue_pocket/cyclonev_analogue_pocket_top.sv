`default_nettype none

module cyclonev_analogue_pocket_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 9600,
    // Original Source: https://github.com/viler-int10h/vga-text-mode-fonts/blob/master/FONTS/PC-OTHER/ATI8X8.F08
    // bitmap_text_renderer_font_init.hex stores the same font with one 8-bit
    // entry per pixel in row-major order.
    parameter string FONT_INIT_FILE = "./core/bitmap_text_renderer_font_init.hex",
    parameter string CHAR_MAP_INIT_FILE = "./core/bitmap_text_renderer_char_map_init.hex",
    parameter string PALETTE_INIT_FILE = "./core/bitmap_text_renderer_palette_init.hex",
    parameter string AUDIO_INIT_FILE = "./core/sine_table_init.hex"
) (
    input  wire logic       clk,
    input  wire logic       clk_video,
    input  wire logic       audio_mclk,
    input  wire logic       audio_sclk,
    input  wire logic [31:0] cont1_key,
    input  wire logic       reset_n,
    input  wire logic       serial_rx,
    output logic            serial_tx,
    output logic            rst_out,
    output logic [23:0]     video_rgb,
    output logic            video_de,
    output logic            video_skip,
    output logic            video_vs,
    output logic            video_hs,
    output logic            audio_dac,
    output logic            audio_lrclk
);
    logic rst;
    logic reset_n_video_sync;
    logic reset_n_audio_sync;
    logic video_rst;
    logic audio_rst;
    logic        bitmap_video_de;
    logic        bitmap_video_hs;
    logic        bitmap_video_vs;
    logic [23:0] bitmap_video_rgb;
    logic [31:0] cont1_key_video;
    logic        face_a_audio;
    logic        bitmap_video_vs_prev;
    logic [7:0]  scroll_x_reg;
    logic [7:0]  scroll_y_reg;
    logic [23:0] video_rgb_reg;
    logic        video_de_reg;
    logic        video_skip_reg;
    logic        video_vs_reg;
    logic        video_hs_reg;
    logic signed [15:0] tone_sample;
    logic               tone_sample_valid;
    logic [4:0]         tone_sample_valid_pipe;

    localparam int unsigned VIDEO_ACTIVE_WIDTH = 256;
    localparam int unsigned VIDEO_ACTIVE_HEIGHT = 224;
    localparam int unsigned VIDEO_TOTAL_WIDTH = 400;
    localparam int unsigned VIDEO_TOTAL_HEIGHT = 512;
    localparam int unsigned VIDEO_H_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_H_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_H_BACK_PORCH =
        VIDEO_TOTAL_WIDTH - VIDEO_ACTIVE_WIDTH - VIDEO_H_FRONT_PORCH - VIDEO_H_SYNC_WIDTH;
    localparam int unsigned VIDEO_V_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_V_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_V_BACK_PORCH =
        VIDEO_TOTAL_HEIGHT - VIDEO_ACTIVE_HEIGHT - VIDEO_V_FRONT_PORCH - VIDEO_V_SYNC_WIDTH;
    localparam int unsigned DPAD_UP_BIT = 0;
    localparam int unsigned DPAD_DOWN_BIT = 1;
    localparam int unsigned DPAD_LEFT_BIT = 2;
    localparam int unsigned DPAD_RIGHT_BIT = 3;
    localparam int unsigned FACE_A_BIT = 4;
    localparam int unsigned AUDIO_PHASE_WIDTH = 32;
    localparam int unsigned AUDIO_TABLE_SIZE = 1024;
    localparam int unsigned TONE_GENERATOR_LATENCY = 5;
    localparam int unsigned I2S_OUTPUT_SAMPLE_WIDTH = 31;
    // sine_table reconstructs a full TABLE_SIZE waveform from a quarter-wave ROM,
    // so the Pocket init file intentionally contains AUDIO_TABLE_SIZE/4 entries.
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_440HZ = 32'd615165;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            rst <= 1'b1;
        end else begin
            rst <= 1'b0;
        end
    end

    ff_sync #(
        .STAGES(3),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) video_reset_sync (
        .clk(clk_video),
        .rst(1'b0),
        .din(reset_n),
        .dout(reset_n_video_sync)
    );

    assign video_rst = !reset_n_video_sync;

    ff_sync #(
        .STAGES(3),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) audio_reset_sync (
        .clk(audio_sclk),
        .rst(1'b0),
        .din(reset_n),
        .dout(reset_n_audio_sync)
    );

    assign audio_rst = !reset_n_audio_sync;

    ff_sync #(
        .STAGES(3),
        .WIDTH(32)
    ) video_dpad_sync (
        .clk(clk_video),
        .rst(video_rst),
        .din(cont1_key),
        .dout(cont1_key_video)
    );

    ff_sync #(
        .STAGES(3),
        .WIDTH(1)
    ) audio_face_a_sync (
        .clk(audio_sclk),
        .rst(audio_rst),
        .din(cont1_key[FACE_A_BIT]),
        .dout(face_a_audio)
    );

    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(74_250_000),
        .RESET_CYCLES(74_250_000),
        .BAUD_RATE(BAUD_RATE)
    ) repo_top_inst (
        .sys_clk(clk),
        .rst(rst),
        .usb_rx(serial_rx),
        .usb_tx(serial_tx),
        .led_out(),
        .sys_led_out(),
        .rst_core(rst_out)
    );

    // tone_generator has a fixed multi-cycle lookup latency, so delay the
    // sideband valid flag until the output sample is fully populated.
    always_ff @(posedge audio_sclk) begin
        if (audio_rst) begin
            tone_sample_valid_pipe <= '0;
        end else begin
            tone_sample_valid_pipe <= {
                tone_sample_valid_pipe[TONE_GENERATOR_LATENCY-2:0],
                face_a_audio
            };
        end
    end

    assign tone_sample_valid = tone_sample_valid_pipe[TONE_GENERATOR_LATENCY-1];

    tone_generator #(
        .PHASE_WIDTH (AUDIO_PHASE_WIDTH),
        .TABLE_SIZE  (AUDIO_TABLE_SIZE),
        .SAMPLE_WIDTH(16),
        .INIT_FILE   (AUDIO_INIT_FILE)
    ) pocket_tone_generator (
        .clk        (audio_sclk),
        .rst        (audio_rst),
        .tuning_word(AUDIO_TUNING_WORD_440HZ),
        .sample     (tone_sample)
    );

    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH (16),
        .OUTPUT_SAMPLE_WIDTH(I2S_OUTPUT_SAMPLE_WIDTH)
    ) pocket_i2s_serializer (
        .clk         (audio_sclk),
        .rst         (audio_rst),
        .sample_data (tone_sample),
        .sample_valid(tone_sample_valid),
        .sample_ready(),  // the tone free-runs; face_a gates validity, so backpressure is unused
        .i2s_bclk    (),
        .i2s_lrclk   (audio_lrclk),
        .i2s_sd      (audio_dac)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .TILE_COLUMNS(32),
        .TILE_ROWS(32),
        .H_FRONT_PORCH(VIDEO_H_FRONT_PORCH),
        .H_SYNC_WIDTH(VIDEO_H_SYNC_WIDTH),
        .H_BACK_PORCH(VIDEO_H_BACK_PORCH),
        .V_FRONT_PORCH(VIDEO_V_FRONT_PORCH),
        .V_SYNC_WIDTH(VIDEO_V_SYNC_WIDTH),
        .V_BACK_PORCH(VIDEO_V_BACK_PORCH),
        .HSYNC_ACTIVE_HIGH(1'b1),
        .VSYNC_ACTIVE_HIGH(1'b1),
        .FONT_INIT_FILE(FONT_INIT_FILE),
        .CHAR_MAP_INIT_FILE(CHAR_MAP_INIT_FILE),
        .PALETTE_INIT_FILE(PALETTE_INIT_FILE)
    ) pocket_bitmap_text_renderer (
        .clk(clk_video),
        .rst(video_rst),
        .scroll_x(scroll_x_reg),
        .scroll_y(scroll_y_reg),
        .video_de(bitmap_video_de),
        .video_hs(bitmap_video_hs),
        .video_vs(bitmap_video_vs),
        .video_rgb(bitmap_video_rgb)
    );

    always_ff @(posedge clk_video) begin
        if (video_rst) begin
            bitmap_video_vs_prev <= 1'b0;
            scroll_x_reg <= 8'd0;
            scroll_y_reg <= 8'd0;
            video_rgb_reg <= 24'h00_00_00;
            video_de_reg <= 1'b0;
            video_skip_reg <= 1'b0;
            video_vs_reg <= 1'b0;
            video_hs_reg <= 1'b0;
        end else begin
            bitmap_video_vs_prev <= bitmap_video_vs;
            if (bitmap_video_vs && !bitmap_video_vs_prev) begin
                if (cont1_key_video[DPAD_LEFT_BIT] && !cont1_key_video[DPAD_RIGHT_BIT]) begin
                    scroll_x_reg <= scroll_x_reg - 8'd1;
                end else if (cont1_key_video[DPAD_RIGHT_BIT] && !cont1_key_video[DPAD_LEFT_BIT]) begin
                    scroll_x_reg <= scroll_x_reg + 8'd1;
                end

                if (cont1_key_video[DPAD_UP_BIT] && !cont1_key_video[DPAD_DOWN_BIT]) begin
                    scroll_y_reg <= scroll_y_reg - 8'd1;
                end else if (cont1_key_video[DPAD_DOWN_BIT] && !cont1_key_video[DPAD_UP_BIT]) begin
                    scroll_y_reg <= scroll_y_reg + 8'd1;
                end
            end
            video_rgb_reg <= bitmap_video_rgb;
            video_de_reg <= bitmap_video_de;
            video_skip_reg <= 1'b0;
            video_vs_reg <= bitmap_video_vs;
            video_hs_reg <= bitmap_video_hs;
        end
    end

    assign video_rgb = video_rgb_reg;
    assign video_de = video_de_reg;
    assign video_skip = video_skip_reg;
    assign video_vs = video_vs_reg;
    assign video_hs = video_hs_reg;
endmodule

`default_nettype wire

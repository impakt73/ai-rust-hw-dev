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
    input  wire logic       audio_mclk,  // Reserved MCLK input for the Pocket audio interface
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
    localparam int unsigned VIDEO_SIGNAL_DELAY_CYCLES = 9;
    logic rst;
    logic reset_n_video_sync;
    logic reset_n_audio_sync;
    logic video_rst;
    logic audio_rst;
    logic        bitmap_sync_video_de;
    logic        bitmap_sync_video_hs;
    logic        bitmap_sync_video_vs;
    logic [((VIDEO_ACTIVE_WIDTH <= 1) ? 1 : $clog2(VIDEO_ACTIVE_WIDTH))-1:0] bitmap_sync_active_x;
    logic [((VIDEO_ACTIVE_HEIGHT <= 1) ? 1 : $clog2(VIDEO_ACTIVE_HEIGHT))-1:0]
        bitmap_sync_active_y;
    logic        bitmap_video_de;
    logic        bitmap_video_hs;
    logic        bitmap_video_vs;
    logic [23:0] bitmap_video_rgb;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] bitmap_video_de_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] bitmap_video_hs_pipe;
    logic [VIDEO_SIGNAL_DELAY_CYCLES-1:0] bitmap_video_vs_pipe;
    logic [9:0]  bitmap_char_mem_addr;
    logic [7:0]  bitmap_char_mem_rdata;
    logic [13:0] bitmap_font_mem_addr;
    logic [7:0]  bitmap_font_mem_rdata;
    logic [7:0]  bitmap_palette_mem_addr;
    logic [23:0] bitmap_palette_mem_rdata;
    logic [31:0] cont1_key_video;
    logic        face_a_audio;
    logic        face_b_audio;
    logic        face_x_audio;
    logic        face_y_audio;
    logic        trig_l_audio;
    logic        trig_r_audio;
    logic        bitmap_video_vs_prev;
    logic [7:0]  scroll_x_reg;
    logic [7:0]  scroll_y_reg;
    logic [23:0] video_rgb_reg;
    logic        video_de_reg;
    logic        video_skip_reg;
    logic        video_vs_reg;
    logic        video_hs_reg;
    localparam int unsigned AUDIO_PHASE_WIDTH = 32;
    localparam int unsigned AUDIO_TABLE_SIZE = 1024;
    localparam int unsigned I2S_OUTPUT_SAMPLE_WIDTH = 31;
    logic signed [15:0] tone_sample;
    logic signed [15:0] i2s_sample_data;
    logic signed [15:0] tone_sample_hold;
    logic               tone_sample_hold_valid;
    logic               i2s_sample_ready;
    logic               tone_sample_valid;
    logic               tone_zero_cross;

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
    localparam int unsigned FACE_B_BIT = 5;
    localparam int unsigned FACE_X_BIT = 6;
    localparam int unsigned FACE_Y_BIT = 7;
    localparam int unsigned TRIG_L1_BIT = 8;
    localparam int unsigned TRIG_R1_BIT = 9;
    // sine_table reconstructs a full TABLE_SIZE waveform from a quarter-wave ROM,
    // so the Pocket init file intentionally contains AUDIO_TABLE_SIZE/4 entries.
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_A4 = 32'd615165;
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_G4 = 32'd548049;
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_D4 = 32'd410573;
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_C4 = 32'd365779;
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_E4 = 32'd460853;
    localparam logic [AUDIO_PHASE_WIDTH-1:0] AUDIO_TUNING_WORD_F4 = 32'd488256;

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
    ) video_cont1_key_sync (
        .clk(clk_video),
        .rst(video_rst),
        .din(cont1_key),
        .dout(cont1_key_video)
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
        .HSYNC_ACTIVE_HIGH(1'b1),
        .VSYNC_ACTIVE_HIGH(1'b1)
    ) pocket_video_sync (
        .clk(clk_video),
        .rst(video_rst),
        .hsync(bitmap_sync_video_hs),
        .vsync(bitmap_sync_video_vs),
        .active_video(bitmap_sync_video_de),
        .line_start(),
        .frame_start(),
        .hblank_start(),
        .vblank_start(),
        .active_x(bitmap_sync_active_x),
        .active_y(bitmap_sync_active_y),
        .scan_x(),
        .scan_y()
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(10),
        .INIT_FILE(CHAR_MAP_INIT_FILE)
    ) pocket_bitmap_char_map_rom (
        .clk(clk_video),
        .addr(bitmap_char_mem_addr),
        .rdata(bitmap_char_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(8),
        .ADDR_WIDTH(14),
        .INIT_FILE(FONT_INIT_FILE)
    ) pocket_bitmap_font_rom (
        .clk(clk_video),
        .addr(bitmap_font_mem_addr),
        .rdata(bitmap_font_mem_rdata)
    );

    sync_sprom #(
        .DATA_WIDTH(24),
        .ADDR_WIDTH(8),
        .INIT_FILE(PALETTE_INIT_FILE)
    ) pocket_bitmap_palette_rom (
        .clk(clk_video),
        .addr(bitmap_palette_mem_addr),
        .rdata(bitmap_palette_mem_rdata)
    );

    // Synchronize all audio-domain button bits as a packed vector to reduce duplication.
    // Bit order: [5]=TRIG_R1, [4]=TRIG_L1, [3]=FACE_Y, [2]=FACE_X, [1]=FACE_B, [0]=FACE_A
    logic [5:0] audio_buttons_sync;

    ff_sync #(
        .STAGES(3),
        .WIDTH(6)
    ) audio_buttons_key_sync (
        .clk(audio_sclk),
        .rst(audio_rst),
        .din({cont1_key[TRIG_R1_BIT], cont1_key[TRIG_L1_BIT],
              cont1_key[FACE_Y_BIT],  cont1_key[FACE_X_BIT],
              cont1_key[FACE_B_BIT],  cont1_key[FACE_A_BIT]}),
        .dout(audio_buttons_sync)
    );

    assign face_a_audio = audio_buttons_sync[0];
    assign face_b_audio = audio_buttons_sync[1];
    assign face_x_audio = audio_buttons_sync[2];
    assign face_y_audio = audio_buttons_sync[3];
    assign trig_l_audio = audio_buttons_sync[4];
    assign trig_r_audio = audio_buttons_sync[5];

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

    logic audio_en;
    logic audio_en_next;
    logic audio_sample_en;
    logic audio_en_update;
    logic [AUDIO_PHASE_WIDTH-1:0] audio_tuning_word;

    always_comb begin
        audio_en_next = face_a_audio || face_b_audio || face_x_audio || face_y_audio || trig_l_audio || trig_r_audio;
        if (face_a_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_A4;
        end else if (face_b_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_C4;
        end else if (face_x_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_G4;
        end else if (face_y_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_D4;
        end else if (trig_l_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_E4;
        end else if (trig_r_audio) begin
            audio_tuning_word = AUDIO_TUNING_WORD_F4;
        end else begin
            audio_tuning_word = AUDIO_TUNING_WORD_A4;  // default to A4 to avoid needing a separate mute implementation
        end
    end

    // audio_lrclk still reflects the previous slot until the serializer reloads on
    // this clock edge, so audio_lrclk=1 means the serializer is about to load the
    // first channel of the next stereo pair and should see a fresh sample. Before
    // the hold register has been seeded after reset, always bypass it so the first
    // stereo frame does not transmit zeros.
    assign i2s_sample_data = (audio_lrclk || !tone_sample_hold_valid) ? tone_sample : tone_sample_hold;
    assign audio_en_update = i2s_sample_ready && tone_zero_cross && tone_sample_valid;
    assign audio_sample_en = audio_en_update ? audio_en_next : audio_en;

    always_ff @(posedge audio_sclk) begin
        if (audio_rst) begin
            tone_sample_hold_valid <= 1'b0;
        end else if (i2s_sample_ready) begin
            if (!tone_sample_hold_valid || audio_lrclk) begin
                tone_sample_hold <= tone_sample;
            end
            tone_sample_hold_valid <= 1'b1;
        end
    end

    always_ff @(posedge audio_sclk) begin
        if (audio_rst) begin
            audio_en <= 1'b0;
        end else if (audio_en_update) begin
            audio_en <= audio_en_next;
        end
    end

    tone_generator #(
        .PHASE_WIDTH (AUDIO_PHASE_WIDTH),
        .TABLE_SIZE  (AUDIO_TABLE_SIZE),
        .SAMPLE_WIDTH(16),
        .INIT_FILE   (AUDIO_INIT_FILE)
    ) pocket_tone_generator (
        .clk        (audio_sclk),
        .rst        (audio_rst),
        .tuning_word(audio_tuning_word),
        .sample     (tone_sample),
        .zero_cross (tone_zero_cross),
        .valid      (tone_sample_valid)
    );

    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH (16),
        .OUTPUT_SAMPLE_WIDTH(I2S_OUTPUT_SAMPLE_WIDTH)
    ) pocket_i2s_serializer (
        .clk         (audio_sclk),
        .rst         (audio_rst),
        .sample_data (i2s_sample_data),
        .sample_valid(audio_sample_en && tone_sample_valid),
        .sample_ready(i2s_sample_ready),
        .i2s_bclk    (),
        .i2s_lrclk   (audio_lrclk),
        .i2s_sd      (audio_dac)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .TILE_COLUMNS(32),
        .TILE_ROWS(32)
    ) pocket_bitmap_text_renderer (
        .clk(clk_video),
        .rst(video_rst),
        .screen_x(bitmap_sync_active_x),
        .screen_y(bitmap_sync_active_y),
        .scroll_x(scroll_x_reg),
        .scroll_y(scroll_y_reg),
        .char_mem_addr(bitmap_char_mem_addr),
        .char_mem_rdata(bitmap_char_mem_rdata),
        .font_mem_addr(bitmap_font_mem_addr),
        .font_mem_rdata(bitmap_font_mem_rdata),
        .palette_mem_addr(bitmap_palette_mem_addr),
        .palette_mem_rdata(bitmap_palette_mem_rdata),
        .video_rgb(bitmap_video_rgb)
    );

    always_ff @(posedge clk_video) begin
        if (video_rst) begin
            bitmap_video_vs_prev <= 1'b0;
            bitmap_video_de_pipe <= '0;
            bitmap_video_hs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{1'b0}};
            bitmap_video_vs_pipe <= {VIDEO_SIGNAL_DELAY_CYCLES{1'b0}};
            scroll_x_reg <= 8'd0;
            scroll_y_reg <= 8'd0;
            video_rgb_reg <= 24'h00_00_00;
            video_de_reg <= 1'b0;
            video_skip_reg <= 1'b0;
            video_vs_reg <= 1'b0;
            video_hs_reg <= 1'b0;
        end else begin
            bitmap_video_de_pipe <= {
                bitmap_video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0],
                bitmap_sync_video_de
            };
            bitmap_video_hs_pipe <= {
                bitmap_video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0],
                bitmap_sync_video_hs
            };
            bitmap_video_vs_pipe <= {
                bitmap_video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-2:0],
                bitmap_sync_video_vs
            };
            bitmap_video_vs_prev <= bitmap_video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
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
            video_rgb_reg <= bitmap_video_de ? bitmap_video_rgb : 24'h00_00_00;
            video_de_reg <= bitmap_video_de;
            video_skip_reg <= 1'b0;
            video_vs_reg <= bitmap_video_vs;
            video_hs_reg <= bitmap_video_hs;
        end
    end

    assign bitmap_video_de = bitmap_video_de_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign bitmap_video_hs = bitmap_video_hs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];
    assign bitmap_video_vs = bitmap_video_vs_pipe[VIDEO_SIGNAL_DELAY_CYCLES-1];

    assign video_rgb = video_rgb_reg;
    assign video_de = video_de_reg;
    assign video_skip = video_skip_reg;
    assign video_vs = video_vs_reg;
    assign video_hs = video_hs_reg;
endmodule

`default_nettype wire

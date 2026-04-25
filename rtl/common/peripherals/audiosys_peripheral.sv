`default_nettype none

module audiosys_peripheral #(
    parameter int unsigned AUDIO_PHASE_WIDTH = 32,
    parameter int unsigned AUDIO_TABLE_SIZE = 1024,
    parameter int unsigned AUDIO_FIFO_DEPTH = 1024,
    parameter int unsigned I2S_OUTPUT_SAMPLE_WIDTH = 31,
    parameter int unsigned BUS_CDC_SYNC_STAGES = 3,
    parameter INIT_FILE = ""
) (
    input  wire logic        sys_clk,
    input  wire logic        audio_clk,
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
    output logic             audio_dac,
    output logic             audio_lrclk,
    output logic             fifo_low_water_irq
);

    localparam logic [4:0] REG_MODE        = 5'h00;
    localparam logic [4:0] REG_TUNING_WORD = 5'h04;
    localparam logic [4:0] REG_FIFO_SAMPLE = 5'h08;
    localparam logic [4:0] REG_FIFO_SPACE  = 5'h0C;

    localparam logic [1:0] AUDIO_MODE_OFF  = 2'd0;
    localparam logic [1:0] AUDIO_MODE_TONE = 2'd1;
    localparam logic [1:0] AUDIO_MODE_FIFO = 2'd2;
    localparam int unsigned AUDIO_FIFO_COUNT_WIDTH = $clog2(AUDIO_FIFO_DEPTH) + 1;

    logic reset_n_audio_sync;
    logic audio_rst;

    logic [31:0] periph_mem_a_addr;
    logic [31:0] periph_mem_a_wdata;
    logic        periph_mem_a_we;
    logic [1:0]  periph_mem_a_size;
    logic        periph_mem_a_valid;
    logic        periph_mem_a_ready;
    logic [31:0] periph_mem_d_rdata;
    logic        periph_mem_d_valid;
    logic        periph_mem_d_ready;
    logic        periph_word_access;

    logic        periph_mem_a_handshake;
    logic        periph_mem_d_handshake;
    logic [31:0] response_data;
    logic        response_pending;

    logic [AUDIO_PHASE_WIDTH-1:0] tuning_word_reg;
    logic [1:0]                   audio_mode_req_reg;
    logic [1:0]                   audio_mode_active;
    logic                         audio_frame_boundary;
    logic                         tone_mode_switch_ok;
    logic signed [15:0]           tone_sample;
    logic signed [15:0]           i2s_sample_data;
    logic signed [15:0]           tone_sample_hold;
    logic                         tone_sample_hold_valid;
    logic                         i2s_sample_ready;
    logic                         tone_sample_valid;
    logic                         tone_zero_cross;

    logic                         fifo_wr_valid;
    logic                         fifo_wr_ready;
    logic [31:0]                  fifo_wdata;
    logic                         fifo_rd_valid;
    logic                         fifo_rd_ready;
    logic [31:0]                  fifo_rdata;
    logic [AUDIO_FIFO_COUNT_WIDTH-1:0] fifo_count;
    logic [AUDIO_FIFO_COUNT_WIDTH-1:0] fifo_space_count;
    logic signed [15:0]           fifo_right_hold;
    logic                         fifo_frame_valid;
    logic                         fifo_left_reload;
    logic                         fifo_low_water_audio;

    function automatic logic [1:0] sanitize_audio_mode(input logic [31:0] mode_value);
        logic [1:0] mode_bits;
        begin
            mode_bits = mode_value[1:0];
            case (mode_bits)
                AUDIO_MODE_OFF,
                AUDIO_MODE_TONE,
                AUDIO_MODE_FIFO: sanitize_audio_mode = mode_bits;
                default: sanitize_audio_mode = AUDIO_MODE_OFF;
            endcase
        end
    endfunction

    initial begin
        if ((AUDIO_FIFO_DEPTH < 2) || ((AUDIO_FIFO_DEPTH & (AUDIO_FIFO_DEPTH - 1)) != 0)) begin
            $fatal(
                1,
                "audiosys_peripheral: AUDIO_FIFO_DEPTH must be power of 2 and >= 2, got %0d",
                AUDIO_FIFO_DEPTH
            );
        end
    end

    assign periph_mem_a_handshake = periph_mem_a_valid && periph_mem_a_ready;
    assign periph_mem_d_handshake = periph_mem_d_valid && periph_mem_d_ready;
    assign periph_mem_d_rdata = response_data;
    assign periph_mem_d_valid = response_pending;
    assign periph_word_access = (periph_mem_a_size == 2'b10) && (periph_mem_a_addr[1:0] == 2'b00);

    assign periph_mem_a_ready =
        !audio_rst
        && !response_pending;

    ff_sync #(
        .STAGES(BUS_CDC_SYNC_STAGES),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) audio_reset_sync (
        .clk(audio_clk),
        .rst(1'b0),
        .din(!rst),
        .dout(reset_n_audio_sync)
    );

    assign audio_rst = !reset_n_audio_sync;

    bus_cdc_bridge #(
        .ADDR_WIDTH(32),
        .DATA_WIDTH(32),
        .SIZE_WIDTH(2),
        .SYNC_STAGES(BUS_CDC_SYNC_STAGES)
    ) u_bus_cdc_bridge (
        .sys_clk(sys_clk),
        .periph_clk(audio_clk),
        .sys_rst(rst),
        .periph_rst(audio_rst),
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

    assign audio_frame_boundary = i2s_sample_ready && audio_lrclk;
    assign tone_mode_switch_ok = audio_frame_boundary && tone_zero_cross && tone_sample_valid;

    assign fifo_wdata = periph_mem_a_wdata;
    assign fifo_wr_valid =
        periph_mem_a_handshake
        && periph_word_access
        && periph_mem_a_we
        && (periph_mem_a_addr[4:0] == REG_FIFO_SAMPLE)
        && (audio_mode_req_reg == AUDIO_MODE_FIFO);
    assign fifo_rd_ready =
        (audio_mode_active == AUDIO_MODE_FIFO)
        && i2s_sample_ready
        && fifo_left_reload;
    assign fifo_space_count = AUDIO_FIFO_COUNT_WIDTH'(AUDIO_FIFO_DEPTH) - fifo_count;
    // i2s_serializer presents sample_ready while audio_lrclk still reflects the channel that
    // just completed, so reloading on audio_lrclk==1 aligns the next FIFO pop with the upcoming
    // left-channel word and avoids an extra retry term.
    assign fifo_left_reload = audio_lrclk;
    assign fifo_low_water_audio =
        (audio_mode_active == AUDIO_MODE_FIFO)
        && (fifo_count < AUDIO_FIFO_COUNT_WIDTH'(AUDIO_FIFO_DEPTH / 2));

    always_comb begin
        unique case (audio_mode_active)
            AUDIO_MODE_TONE: begin
                i2s_sample_data =
                    (audio_lrclk || !tone_sample_hold_valid) ? tone_sample : tone_sample_hold;
            end
            AUDIO_MODE_FIFO: begin
                if (fifo_left_reload) begin
                    if (fifo_rd_valid) begin
                        i2s_sample_data = fifo_rdata[31:16];
                    end else begin
                        i2s_sample_data = '0;
                    end
                end else begin
                    // Suppress stale right-channel hold data until a valid stereo frame has
                    // actually been popped from the FIFO.
                    i2s_sample_data = fifo_frame_valid ? fifo_right_hold : '0;
                end
            end
            default: begin
                i2s_sample_data = '0;
            end
        endcase
    end

    always_ff @(posedge audio_clk) begin
        if (audio_rst) begin
            tuning_word_reg <= '0;
            audio_mode_req_reg <= AUDIO_MODE_OFF;
            response_pending <= 1'b0;
        end else begin
            if (periph_mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (periph_mem_a_handshake) begin
                response_data <= 32'h0000_0000;
                response_pending <= 1'b1;

                if (periph_word_access) begin
                    if (periph_mem_a_we) begin
                        unique case (periph_mem_a_addr[4:0])
                            REG_MODE: audio_mode_req_reg <= sanitize_audio_mode(periph_mem_a_wdata);
                            REG_TUNING_WORD: tuning_word_reg <= periph_mem_a_wdata;
                            default: begin
                            end
                        endcase
                    end else begin
                        unique case (periph_mem_a_addr[4:0])
                            REG_MODE: response_data <= {30'h0000_0000, audio_mode_req_reg};
                            REG_TUNING_WORD: response_data <= tuning_word_reg;
                            REG_FIFO_SPACE: response_data <= {{(32-AUDIO_FIFO_COUNT_WIDTH){1'b0}}, fifo_space_count};
                            default: response_data <= 32'h0000_0000;
                        endcase
                    end
                end
            end
        end
    end

    always_ff @(posedge audio_clk) begin
        if (audio_rst) begin
            tone_sample_hold_valid <= 1'b0;
        end else if (i2s_sample_ready) begin
            if (!tone_sample_hold_valid || audio_lrclk) begin
                tone_sample_hold <= tone_sample;
            end
            tone_sample_hold_valid <= 1'b1;
        end
    end

    always_ff @(posedge audio_clk) begin
        if (audio_rst) begin
            audio_mode_active <= AUDIO_MODE_OFF;
        end else if (audio_mode_active != audio_mode_req_reg) begin
            if (((audio_mode_active == AUDIO_MODE_TONE) && (audio_mode_req_reg == AUDIO_MODE_FIFO))
                || ((audio_mode_active == AUDIO_MODE_FIFO) && (audio_mode_req_reg == AUDIO_MODE_TONE))) begin
                if (tone_mode_switch_ok) begin
                    audio_mode_active <= audio_mode_req_reg;
                end
            end else if (audio_frame_boundary) begin
                audio_mode_active <= audio_mode_req_reg;
            end
        end
    end

    always_ff @(posedge audio_clk) begin
        if (audio_rst) begin
            fifo_frame_valid <= 1'b0;
        end else if (audio_mode_active != AUDIO_MODE_FIFO) begin
            fifo_frame_valid <= 1'b0;
        end else if (i2s_sample_ready && fifo_left_reload) begin
            if (fifo_rd_valid) begin
                fifo_right_hold <= fifo_rdata[15:0];
                fifo_frame_valid <= 1'b1;
            end else begin
                fifo_frame_valid <= 1'b0;
            end
        end
    end

    sync_fifo #(
        .WIDTH(32),
        .DEPTH(AUDIO_FIFO_DEPTH)
    ) u_audio_fifo (
        .clk(audio_clk),
        .rst(audio_rst),
        .wr_valid(fifo_wr_valid),
        .wr_ready(fifo_wr_ready),
        .wdata(fifo_wdata),
        .rd_valid(fifo_rd_valid),
        .rd_ready(fifo_rd_ready),
        .rdata(fifo_rdata),
        .count(fifo_count)
    );

    ff_sync #(
        .STAGES(BUS_CDC_SYNC_STAGES),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) audiosys_irq_sync (
        .clk(sys_clk),
        .rst(rst),
        .din(fifo_low_water_audio),
        .dout(fifo_low_water_irq)
    );

    tone_generator #(
        .PHASE_WIDTH (AUDIO_PHASE_WIDTH),
        .TABLE_SIZE  (AUDIO_TABLE_SIZE),
        .SAMPLE_WIDTH(16),
        .INIT_FILE   (INIT_FILE)
    ) u_tone_generator (
        .clk        (audio_clk),
        .rst        (audio_rst),
        .tuning_word(tuning_word_reg),
        .sample     (tone_sample),
        .zero_cross (tone_zero_cross),
        .valid      (tone_sample_valid)
    );

    i2s_serializer #(
        .INPUT_SAMPLE_WIDTH (16),
        .OUTPUT_SAMPLE_WIDTH(I2S_OUTPUT_SAMPLE_WIDTH)
    ) u_i2s_serializer (
        .clk         (audio_clk),
        .rst         (audio_rst),
        .sample_data (i2s_sample_data),
        .sample_valid(
            (audio_mode_active == AUDIO_MODE_TONE) ? tone_sample_valid : (audio_mode_active != AUDIO_MODE_OFF)
        ),
        .sample_ready(i2s_sample_ready),
        .i2s_bclk    (),
        .i2s_lrclk   (audio_lrclk),
        .i2s_sd      (audio_dac)
    );

endmodule

`default_nettype wire

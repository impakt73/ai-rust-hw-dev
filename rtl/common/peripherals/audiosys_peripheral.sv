`default_nettype none

module audiosys_peripheral #(
    parameter int unsigned AUDIO_PHASE_WIDTH = 32,
    parameter int unsigned AUDIO_TABLE_SIZE = 1024,
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
    output logic             audio_lrclk
);

    localparam logic [4:0] REG_CONTROL = 5'h00;
    localparam logic [4:0] REG_TUNING_WORD = 5'h04;

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
    logic                         audio_enable_req_reg;
    logic                         audio_en;
    logic                         audio_sample_en;
    logic                         audio_en_update;
    logic signed [15:0]           tone_sample;
    logic signed [15:0]           i2s_sample_data;
    logic signed [15:0]           tone_sample_hold;
    logic                         tone_sample_hold_valid;
    logic                         i2s_sample_ready;
    logic                         tone_sample_valid;
    logic                         tone_zero_cross;

    assign periph_mem_a_handshake = periph_mem_a_valid && periph_mem_a_ready;
    assign periph_mem_d_handshake = periph_mem_d_valid && periph_mem_d_ready;
    assign periph_mem_a_ready = !audio_rst && !response_pending;
    assign periph_mem_d_rdata = response_data;
    assign periph_mem_d_valid = response_pending;
    assign periph_word_access = (periph_mem_a_size == 2'b10) && (periph_mem_a_addr[1:0] == 2'b00);

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

    // audio_lrclk still reflects the previous slot until the serializer reloads on
    // this clock edge, so audio_lrclk=1 means the serializer is about to load the
    // first channel of the next stereo pair and should see a fresh sample. Before
    // the hold register has been seeded after reset, always bypass it so the first
    // stereo frame does not transmit zeros.
    assign i2s_sample_data = (audio_lrclk || !tone_sample_hold_valid) ? tone_sample : tone_sample_hold;
    assign audio_en_update = i2s_sample_ready && tone_zero_cross && tone_sample_valid;
    assign audio_sample_en = audio_en_update ? audio_enable_req_reg : audio_en;

    always_ff @(posedge audio_clk) begin
        if (audio_rst) begin
            tuning_word_reg <= '0;
            audio_enable_req_reg <= 1'b0;
            response_pending <= 1'b0;
        end else begin
            if (periph_mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (periph_mem_a_handshake) begin
                // response_data intentionally is not reset because response_pending
                // marks when the payload is meaningful.
                response_data <= 32'h0000_0000;
                response_pending <= 1'b1;

                if (periph_word_access) begin
                    if (periph_mem_a_we) begin
                        case (periph_mem_a_addr[4:0])
                            REG_TUNING_WORD: tuning_word_reg <= periph_mem_a_wdata;
                            REG_CONTROL: audio_enable_req_reg <= periph_mem_a_wdata[0];
                            default: begin
                            end
                        endcase
                    end else begin
                        case (periph_mem_a_addr[4:0])
                            REG_TUNING_WORD: response_data <= tuning_word_reg;
                            REG_CONTROL: response_data <= {31'h0000_0000, audio_enable_req_reg};
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
            audio_en <= 1'b0;
        end else if (audio_en_update) begin
            audio_en <= audio_enable_req_reg;
        end
    end

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
        .sample_valid(audio_sample_en && tone_sample_valid),
        .sample_ready(i2s_sample_ready),
        .i2s_bclk    (),
        .i2s_lrclk   (audio_lrclk),
        .i2s_sd      (audio_dac)
    );

endmodule

`default_nettype wire

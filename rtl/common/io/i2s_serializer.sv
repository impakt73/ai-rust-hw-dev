module i2s_serializer #(
    parameter int INPUT_SAMPLE_WIDTH = 16,
    parameter int OUTPUT_SAMPLE_WIDTH = 16
) (
    input  logic                          clk,
    input  logic                          rst_n,
    input  logic [INPUT_SAMPLE_WIDTH-1:0] sample_data,
    input  logic                          sample_valid,
    output logic                          sample_ready,
    output logic                          i2s_bclk,
    output logic                          i2s_lrclk,
    output logic                          i2s_sd
);

    localparam int BIT_INDEX_WIDTH = (OUTPUT_SAMPLE_WIDTH <= 1) ? 1 : $clog2(OUTPUT_SAMPLE_WIDTH);
    localparam int PAD_BITS = (OUTPUT_SAMPLE_WIDTH > INPUT_SAMPLE_WIDTH) ?
        (OUTPUT_SAMPLE_WIDTH - INPUT_SAMPLE_WIDTH) : 0;

    logic [OUTPUT_SAMPLE_WIDTH-1:0] shift_reg;
    logic [OUTPUT_SAMPLE_WIDTH-1:0] formatted_sample;
    logic [BIT_INDEX_WIDTH-1:0] bit_index;
    logic reload_pending;
    logic next_channel;

    // The serializer does not divide or synthesize a new bit clock.
    // The caller must provide clk at the desired I2S bit-clock rate.
    assign i2s_bclk = clk;
    assign sample_ready = reload_pending;

    generate
        if (INPUT_SAMPLE_WIDTH >= OUTPUT_SAMPLE_WIDTH) begin : gen_truncate_input
            // Extract the OUTPUT_SAMPLE_WIDTH most-significant input bits.
            assign formatted_sample = sample_data[INPUT_SAMPLE_WIDTH-1 -: OUTPUT_SAMPLE_WIDTH];
        end else begin : gen_pad_input
            assign formatted_sample = {sample_data, {PAD_BITS{1'b0}}};
        end
    endgenerate

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            shift_reg <= '0;
            bit_index <= '0;
            reload_pending <= 1'b1;
            next_channel <= 1'b0;
            i2s_lrclk <= 1'b0;
            i2s_sd <= 1'b0;
        end else if (reload_pending) begin
            shift_reg <= sample_valid ? formatted_sample : '0;
            bit_index <= '0;
            reload_pending <= 1'b0;
            i2s_lrclk <= next_channel;
            // Standard I2S changes LRCLK one bit clock before the next word's MSB.
            // This zero drive is that alignment slot before shifting the next sample.
            i2s_sd <= 1'b0;
        end else begin
            i2s_sd <= shift_reg[OUTPUT_SAMPLE_WIDTH-1];
            if (OUTPUT_SAMPLE_WIDTH > 1) begin
                shift_reg <= {shift_reg[OUTPUT_SAMPLE_WIDTH-2:0], 1'b0};
            end else begin
                shift_reg <= '0;
            end

            // After OUTPUT_SAMPLE_WIDTH bits have been transmitted, request a reload.
            if (bit_index == BIT_INDEX_WIDTH'(OUTPUT_SAMPLE_WIDTH - 1)) begin
                reload_pending <= 1'b1;
                next_channel <= ~next_channel;
            end else begin
                bit_index <= bit_index + 1'b1;
            end
        end
    end

endmodule

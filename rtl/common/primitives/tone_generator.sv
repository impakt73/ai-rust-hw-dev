`default_nettype none
// Tone Generator
// Generates sine-wave tone samples using a tuning-word-driven phase accumulator.
//
// Parameters:
//   PHASE_WIDTH  - Phase accumulator width in bits
//   TABLE_SIZE   - Full sine table size (power-of-2, >= 8)
//   SAMPLE_WIDTH - Output sample width in bits
//   INIT_FILE    - Sine table ROM init file passed to sine_table
//
// Interface:
//   clk         - System clock
//   rst         - Synchronous active-high reset
//   tuning_word - Per-cycle phase increment
//   sample      - Pipelined sine sample output
module tone_generator #(
    parameter int unsigned PHASE_WIDTH  = 32,
    parameter int          TABLE_SIZE   = 1024,
    parameter int          SAMPLE_WIDTH = 16,
    parameter              INIT_FILE    = ""
) (
    input  wire logic                           clk,
    input  wire logic                           rst,
    input  wire logic [PHASE_WIDTH-1:0]         tuning_word,
    output      logic signed [SAMPLE_WIDTH-1:0] sample,
    output      logic                           zero_cross
);

    localparam int TABLE_ADDR_WIDTH = $clog2(TABLE_SIZE);
    localparam int SINE_TABLE_LATENCY = 4;

    logic [PHASE_WIDTH-1:0]         phase_acc;
    logic [PHASE_WIDTH+TABLE_ADDR_WIDTH-1:0] phase_index_window;
    logic [TABLE_ADDR_WIDTH-1:0]    table_index;
    logic                           zero_cross_pre;
    logic [SINE_TABLE_LATENCY-2:0]  zero_cross_pipe;

    initial begin
        if (PHASE_WIDTH == 0) begin
            $fatal(1, "tone_generator: PHASE_WIDTH must be >= 1");
        end
        if (TABLE_SIZE < 8) begin
            $fatal(1, "tone_generator: TABLE_SIZE must be >= 8, got %0d", TABLE_SIZE);
        end
        if ((TABLE_SIZE & (TABLE_SIZE - 1)) != 0) begin
            $fatal(1, "tone_generator: TABLE_SIZE must be a power of 2, got %0d", TABLE_SIZE);
        end
        if (PHASE_WIDTH < TABLE_ADDR_WIDTH) begin
            $fatal(
                1,
                "tone_generator: PHASE_WIDTH (%0d) must be >= table address width (%0d)",
                PHASE_WIDTH,
                TABLE_ADDR_WIDTH
            );
        end
        if (SAMPLE_WIDTH < 2) begin
            $fatal(1, "tone_generator: SAMPLE_WIDTH must be >= 2, got %0d", SAMPLE_WIDTH);
        end
    end

    assign phase_index_window = {phase_acc, {TABLE_ADDR_WIDTH{1'b0}}};
    assign table_index = phase_index_window[PHASE_WIDTH+TABLE_ADDR_WIDTH-1 -: TABLE_ADDR_WIDTH];
    assign zero_cross_pre = (table_index[TABLE_ADDR_WIDTH-2:0] == '0);

    always_ff @(posedge clk) begin
        if (rst) begin
            phase_acc <= '0;
        end else begin
            phase_acc <= phase_acc + tuning_word;
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            zero_cross_pipe <= '0;
            zero_cross      <= 1'b0;
        end else begin
            zero_cross_pipe <= {zero_cross_pipe[SINE_TABLE_LATENCY-3:0], zero_cross_pre};
            zero_cross      <= zero_cross_pipe[SINE_TABLE_LATENCY-2];
        end
    end

    sine_table #(
        .TABLE_SIZE   (TABLE_SIZE),
        .SAMPLE_WIDTH (SAMPLE_WIDTH),
        .INIT_FILE    (INIT_FILE)
    ) u_sine_table (
        .clk    (clk),
        .index  (table_index),
        .sample (sample)
    );

endmodule
`default_nettype wire

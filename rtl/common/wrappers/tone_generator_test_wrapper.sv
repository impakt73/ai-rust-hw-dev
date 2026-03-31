`default_nettype none
module tone_generator_test_wrapper (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [31:0] tuning_word,
    output      logic [15:0] sample
);

    tone_generator #(
        .PHASE_WIDTH (32),
        .TABLE_SIZE  (1024),
        .SAMPLE_WIDTH(16),
        .INIT_FILE   ("../rtl/common/wrappers/sine_table_test_init.hex")
    ) u_tone_generator (
        .clk          (clk),
        .rst          (rst),
        .tuning_word  (tuning_word),
        .zero_crossing(),
        .sample       (sample)
    );

endmodule
`default_nettype wire

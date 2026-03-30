`default_nettype none
// Test wrapper for sine_table:
//   TABLE_SIZE=1024, SAMPLE_WIDTH=16, 256-entry quarter-wave ROM.
module sine_table_test_wrapper (
    input  wire logic        clk,
    input  wire logic [9:0]  index,
    output      logic [15:0] sample
);

    sine_table #(
        .TABLE_SIZE  (1024),
        .SAMPLE_WIDTH(16),
        .INIT_FILE   ("../rtl/common/wrappers/sine_table_test_init.hex")
    ) u_sine_table (
        .clk   (clk),
        .index (index),
        .sample(sample)
    );

endmodule
`default_nettype wire

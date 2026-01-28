// Stub Divider Module for FPGA Builds with M Extension Disabled
// This module is never instantiated when ENABLE_M_EXT=0
// It exists only to satisfy Yosys elaboration requirements

module div_unit #(
    parameter int WIDTH = 32
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        start,
    input  logic        is_signed,
    input  logic        rem_sel,
    input  logic [WIDTH-1:0] dividend,
    input  logic [WIDTH-1:0] divisor,
    output logic [WIDTH-1:0] result,
    output logic             ready
);
    // Stub implementation - all outputs tied to zero
    assign result = 32'h0;
    assign ready = 1'b0;
endmodule

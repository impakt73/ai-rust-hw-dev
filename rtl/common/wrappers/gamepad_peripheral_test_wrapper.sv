`default_nettype none

// Test wrapper for gamepad_peripheral.
// Exposes all ports (including gamepad_in) directly for Verilator simulation.

module gamepad_peripheral_test_wrapper (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [9:0]  gamepad_in,
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready
);

    gamepad_peripheral u_gamepad_peripheral (
        .clk(clk),
        .rst(rst),
        .gamepad_in(gamepad_in),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready)
    );

endmodule

`default_nettype wire

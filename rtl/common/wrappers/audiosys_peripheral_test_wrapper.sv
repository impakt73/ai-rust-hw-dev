`default_nettype none

module audiosys_peripheral_test_wrapper (
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

    audiosys_peripheral #(
        .INIT_FILE("../rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/sine_table_init.hex")
    ) u_audiosys_peripheral (
        .sys_clk(sys_clk),
        .audio_clk(audio_clk),
        .rst(rst),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready),
        .audio_dac(audio_dac),
        .audio_lrclk(audio_lrclk)
    );

endmodule

`default_nettype wire

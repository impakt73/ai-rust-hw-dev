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
    output logic             audio_lrclk,
    output logic             fifo_low_water_irq,
    output logic [1:0]       debug_audio_mode_active,
    output logic             debug_i2s_sample_ready,
    output logic signed [15:0] debug_i2s_sample_data,
    output logic signed [15:0] debug_fifo_right_hold,
    output logic             debug_fifo_frame_valid,
    output logic [31:0]      debug_fifo_count,
    output logic [31:0]      debug_fifo_space
);

    audiosys_peripheral #(
        .AUDIO_FIFO_DEPTH(8),
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
        .audio_lrclk(audio_lrclk),
        .fifo_low_water_irq(fifo_low_water_irq)
    );

    assign debug_audio_mode_active = u_audiosys_peripheral.audio_mode_active;
    assign debug_i2s_sample_ready = u_audiosys_peripheral.i2s_sample_ready;
    assign debug_i2s_sample_data = u_audiosys_peripheral.i2s_sample_data;
    assign debug_fifo_right_hold = u_audiosys_peripheral.fifo_right_hold;
    assign debug_fifo_frame_valid = u_audiosys_peripheral.fifo_frame_valid;
    assign debug_fifo_count = 32'(u_audiosys_peripheral.fifo_count);
    assign debug_fifo_space = 32'(u_audiosys_peripheral.fifo_space_count);

endmodule

`default_nettype wire

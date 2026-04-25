`default_nettype none

module top_sim_test_wrapper (
    input  wire logic        clk,
    input  wire logic        video_clk,
    input  wire logic        audio_clk,
    input  wire logic        sdram_clk,
    input  wire logic        rst,
    output logic [7:0]       host_tx_data,
    output logic             host_tx_valid,
    input  wire logic        host_tx_ready,
    input  wire logic [7:0]  host_rx_data,
    input  wire logic        host_rx_valid,
    output logic             host_rx_ready,
    input  wire logic        com_err,
    output logic [7:0]       led_out,
    output logic [7:0]       sys_led_out,
    output logic             halted,
    output logic             instr_complete,
    output logic [31:0]      debug_rs1_data,
    output logic [31:0]      debug_rs2_data,
    output logic [31:0]      debug_rd_data,
    output logic [31:0]      debug_pc,
    output logic [31:0]      debug_instruction,
    output logic [31:0]      debug_current_pc,
    output logic [31:0]      debug_current_instruction,
    output logic [3:0]       debug_fsm_state,
    output logic             rst_out,
    output logic             cpu_booting,
    output logic [31:0]      halted_value,
    output logic [23:0]      video_rgb,
    output logic             video_de,
    output logic             video_skip,
    output logic             video_vs,
    output logic             video_hs,
    output logic             audio_dac,
    output logic             audio_lrclk,
    output logic             sdram_word_rd,
    output logic             sdram_word_wr,
    output logic [23:0]      sdram_word_addr,
    output logic [31:0]      sdram_word_data,
    input  wire logic [31:0] sdram_word_q,
    input  wire logic        sdram_word_busy,
    input  wire logic        ext_cpu_boot,
    input  wire logic [31:0] ext_cpu_boot_addr,
    input  wire logic [9:0]  gamepad_in,
    input  wire logic [31:0] apf_bridge_addr,
    input  wire logic        apf_bridge_rd,
    output logic             apf_bridge_rd_ready,
    input  wire logic        apf_bridge_wr,
    output logic             apf_bridge_wr_ready,
    input  wire logic [31:0] apf_bridge_wr_data,
    output logic [31:0]      apf_bridge_rd_data
);

    logic [7:0] audio_clk_divider;
    logic       audio_clk_slow;

    always_ff @(posedge clk) begin
        if (rst) begin
            audio_clk_divider <= '0;
            audio_clk_slow <= 1'b0;
        end else if (audio_clk_divider == 8'hFF) begin
            audio_clk_divider <= '0;
            // Reference the external audio_clk input in a functionally inert way so
            // the wrapper port remains live during linting.
            audio_clk_slow <= ~audio_clk_slow ^ (audio_clk & 1'b0);
        end else begin
            audio_clk_divider <= audio_clk_divider + 1'b1;
        end
    end

    top #(
        .ENABLE_AUDIOSYS(1'b1),
        .AUDIOSYS_FIFO_DEPTH(2)
    ) u_top (
        .clk(clk),
        .video_clk(video_clk),
        .audio_clk(audio_clk_slow),
        .sdram_clk(sdram_clk),
        .rst(rst),
        .host_tx_data(host_tx_data),
        .host_tx_valid(host_tx_valid),
        .host_tx_ready(host_tx_ready),
        .host_rx_data(host_rx_data),
        .host_rx_valid(host_rx_valid),
        .host_rx_ready(host_rx_ready),
        .com_err(com_err),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .halted(halted),
        .instr_complete(instr_complete),
        .debug_rs1_data(debug_rs1_data),
        .debug_rs2_data(debug_rs2_data),
        .debug_rd_data(debug_rd_data),
        .debug_pc(debug_pc),
        .debug_instruction(debug_instruction),
        .debug_current_pc(debug_current_pc),
        .debug_current_instruction(debug_current_instruction),
        .debug_fsm_state(debug_fsm_state),
        .rst_out(rst_out),
        .cpu_booting(cpu_booting),
        .halted_value(halted_value),
        .video_rgb(video_rgb),
        .video_de(video_de),
        .video_skip(video_skip),
        .video_vs(video_vs),
        .video_hs(video_hs),
        .audio_dac(audio_dac),
        .audio_lrclk(audio_lrclk),
        .sdram_word_rd(sdram_word_rd),
        .sdram_word_wr(sdram_word_wr),
        .sdram_word_addr(sdram_word_addr),
        .sdram_word_data(sdram_word_data),
        .sdram_word_q(sdram_word_q),
        .sdram_word_busy(sdram_word_busy),
        .ext_cpu_boot(ext_cpu_boot),
        .ext_cpu_boot_addr(ext_cpu_boot_addr),
        .gamepad_in(gamepad_in),
        .apf_bridge_addr(apf_bridge_addr),
        .apf_bridge_rd(apf_bridge_rd),
        .apf_bridge_rd_ready(apf_bridge_rd_ready),
        .apf_bridge_wr(apf_bridge_wr),
        .apf_bridge_wr_ready(apf_bridge_wr_ready),
        .apf_bridge_wr_data(apf_bridge_wr_data),
        .apf_bridge_rd_data(apf_bridge_rd_data)
    );

endmodule

`default_nettype wire

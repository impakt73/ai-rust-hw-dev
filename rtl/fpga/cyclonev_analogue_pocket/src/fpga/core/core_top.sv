`default_nettype none

module core_top (
    input  wire logic        clk_74a,
    input  wire logic        clk_74b,
    input  wire logic        reset_n,
    inout  wire logic [7:0]  cart_tran_bank2,
    output logic             cart_tran_bank2_dir,
    inout  wire logic [7:0]  cart_tran_bank3,
    output logic             cart_tran_bank3_dir,
    inout  wire logic [7:0]  cart_tran_bank1,
    output logic             cart_tran_bank1_dir,
    inout  wire logic [7:4]  cart_tran_bank0,
    output logic             cart_tran_bank0_dir,
    inout  wire logic        cart_tran_pin30,
    output logic             cart_tran_pin30_dir,
    output logic             cart_pin30_pwroff_reset,
    inout  wire logic        cart_tran_pin31,
    output logic             cart_tran_pin31_dir,
    input  wire logic        port_ir_rx,
    output logic             port_ir_tx,
    output logic             port_ir_rx_disable,
    inout  wire logic        port_tran_si,
    output logic             port_tran_si_dir,
    inout  wire logic        port_tran_so,
    output logic             port_tran_so_dir,
    inout  wire logic        port_tran_sck,
    output logic             port_tran_sck_dir,
    inout  wire logic        port_tran_sd,
    output logic             port_tran_sd_dir,
    output logic [21:16]     cram0_a,
    inout  wire logic [15:0] cram0_dq,
    input  wire logic        cram0_wait,
    output logic             cram0_clk,
    output logic             cram0_adv_n,
    output logic             cram0_cre,
    output logic             cram0_ce0_n,
    output logic             cram0_ce1_n,
    output logic             cram0_oe_n,
    output logic             cram0_we_n,
    output logic             cram0_ub_n,
    output logic             cram0_lb_n,
    output logic [21:16]     cram1_a,
    inout  wire logic [15:0] cram1_dq,
    input  wire logic        cram1_wait,
    output logic             cram1_clk,
    output logic             cram1_adv_n,
    output logic             cram1_cre,
    output logic             cram1_ce0_n,
    output logic             cram1_ce1_n,
    output logic             cram1_oe_n,
    output logic             cram1_we_n,
    output logic             cram1_ub_n,
    output logic             cram1_lb_n,
    output logic [12:0]      dram_a,
    output logic [1:0]       dram_ba,
    inout  wire logic [15:0] dram_dq,
    output logic [1:0]       dram_dqm,
    output logic             dram_clk,
    output logic             dram_cke,
    output logic             dram_ras_n,
    output logic             dram_cas_n,
    output logic             dram_we_n,
    output logic [16:0]      sram_a,
    inout  wire logic [15:0] sram_dq,
    output logic             sram_oe_n,
    output logic             sram_we_n,
    output logic             sram_ub_n,
    output logic             sram_lb_n,
    input  wire logic        vblank,
    output logic             dbg_tx,
    input  wire logic        dbg_rx,
    output logic             user1,
    input  wire logic        user2,
    inout  wire logic        aux_sda,
    output logic             aux_scl,
    output logic             vpll_feed,
    output logic [23:0]      video_rgb,
    output logic             video_rgb_clock,
    output logic             video_rgb_clock_90,
    output logic             video_de,
    output logic             video_skip,
    output logic             video_vs,
    output logic             video_hs,
    output logic             audio_mclk,
    input  wire logic        audio_adc,
    output logic             audio_dac,
    output logic             audio_lrck,
    output logic             bridge_endian_little,
    input  wire logic [31:0] bridge_addr,
    input  wire logic        bridge_rd,
    output logic [31:0]      bridge_rd_data,
    input  wire logic        bridge_wr,
    input  wire logic [31:0] bridge_wr_data,
    input  wire logic [31:0] cont1_key,
    input  wire logic [31:0] cont2_key,
    input  wire logic [31:0] cont3_key,
    input  wire logic [31:0] cont4_key,
    input  wire logic [31:0] cont1_joy,
    input  wire logic [31:0] cont2_joy,
    input  wire logic [31:0] cont3_joy,
    input  wire logic [31:0] cont4_joy,
    input  wire logic [15:0] cont1_trig,
    input  wire logic [15:0] cont2_trig,
    input  wire logic [15:0] cont3_trig,
    input  wire logic [15:0] cont4_trig
);
    logic [7:0] led_out;
    logic [7:0] sys_led_out;
    logic halted;
    logic instr_complete;
    logic rst_out;
    logic cpu_booting;
    logic [31:0] halted_value;

    analogue_pocket_repo_top repo_top_inst (
        .clk(clk_74a),
        .reset_n(reset_n),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .halted(halted),
        .instr_complete(instr_complete),
        .rst_out(rst_out),
        .cpu_booting(cpu_booting),
        .halted_value(halted_value)
    );

    assign bridge_endian_little = 1'b0;

    always_comb begin
        unique case (bridge_addr)
            32'h1000_0000: bridge_rd_data = {24'h0, led_out};
            32'h1000_0004: bridge_rd_data = {24'h0, sys_led_out};
            32'h1000_0008: bridge_rd_data = {29'h0, cpu_booting, halted, instr_complete};
            32'h1000_000C: bridge_rd_data = halted_value;
            default: bridge_rd_data = 32'h0;
        endcase
    end

    assign cart_tran_bank2 = 8'hZZ;
    assign cart_tran_bank2_dir = 1'b0;
    assign cart_tran_bank3 = 8'hZZ;
    assign cart_tran_bank3_dir = 1'b0;
    assign cart_tran_bank1 = 8'hZZ;
    assign cart_tran_bank1_dir = 1'b0;
    assign cart_tran_bank0 = 4'hF;
    assign cart_tran_bank0_dir = 1'b1;
    assign cart_tran_pin30 = 1'bz;
    assign cart_tran_pin30_dir = 1'b0;
    assign cart_pin30_pwroff_reset = 1'b0;
    assign cart_tran_pin31 = 1'bz;
    assign cart_tran_pin31_dir = 1'b0;
    assign port_ir_tx = 1'b0;
    assign port_ir_rx_disable = 1'b1;
    assign port_tran_si = 1'bz;
    assign port_tran_si_dir = 1'b0;
    assign port_tran_so = 1'bz;
    assign port_tran_so_dir = 1'b0;
    assign port_tran_sck = 1'bz;
    assign port_tran_sck_dir = 1'b0;
    assign port_tran_sd = 1'bz;
    assign port_tran_sd_dir = 1'b0;

    assign cram0_a = '0;
    assign cram0_dq = {16{1'bz}};
    assign cram0_clk = 1'b0;
    assign cram0_adv_n = 1'b1;
    assign cram0_cre = 1'b0;
    assign cram0_ce0_n = 1'b1;
    assign cram0_ce1_n = 1'b1;
    assign cram0_oe_n = 1'b1;
    assign cram0_we_n = 1'b1;
    assign cram0_ub_n = 1'b1;
    assign cram0_lb_n = 1'b1;

    assign cram1_a = '0;
    assign cram1_dq = {16{1'bz}};
    assign cram1_clk = 1'b0;
    assign cram1_adv_n = 1'b1;
    assign cram1_cre = 1'b0;
    assign cram1_ce0_n = 1'b1;
    assign cram1_ce1_n = 1'b1;
    assign cram1_oe_n = 1'b1;
    assign cram1_we_n = 1'b1;
    assign cram1_ub_n = 1'b1;
    assign cram1_lb_n = 1'b1;

    assign dram_a = '0;
    assign dram_ba = '0;
    assign dram_dq = {16{1'bz}};
    assign dram_dqm = '0;
    assign dram_clk = 1'b0;
    assign dram_cke = 1'b0;
    assign dram_ras_n = 1'b1;
    assign dram_cas_n = 1'b1;
    assign dram_we_n = 1'b1;

    assign sram_a = '0;
    assign sram_dq = {16{1'bz}};
    assign sram_oe_n = 1'b1;
    assign sram_we_n = 1'b1;
    assign sram_ub_n = 1'b1;
    assign sram_lb_n = 1'b1;

    assign dbg_tx = 1'b1;
    assign user1 = sys_led_out[0];
    assign aux_sda = 1'bz;
    assign aux_scl = 1'b1;
    assign vpll_feed = 1'b0;

    assign video_rgb = {sys_led_out, led_out, {6'b0, cpu_booting, halted}};
    assign video_rgb_clock = clk_74a;
    assign video_rgb_clock_90 = clk_74b;
    assign video_de = 1'b1;
    assign video_skip = 1'b0;
    assign video_vs = 1'b0;
    assign video_hs = 1'b0;
    assign audio_mclk = clk_74a;
    assign audio_dac = 1'b0;
    assign audio_lrck = 1'b0;
endmodule

`default_nettype wire

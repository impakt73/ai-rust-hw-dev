`default_nettype none

module apf_top (
    input  wire        clk_74a,
    input  wire        clk_74b,
    inout  wire [7:0]  cart_tran_bank2,
    output wire        cart_tran_bank2_dir,
    inout  wire [7:0]  cart_tran_bank3,
    output wire        cart_tran_bank3_dir,
    inout  wire [7:0]  cart_tran_bank1,
    output wire        cart_tran_bank1_dir,
    inout  wire [7:4]  cart_tran_bank0,
    output wire        cart_tran_bank0_dir,
    inout  wire        cart_tran_pin30,
    output wire        cart_tran_pin30_dir,
    output wire        cart_pin30_pwroff_reset,
    inout  wire        cart_tran_pin31,
    output wire        cart_tran_pin31_dir,
    input  wire        port_ir_rx,
    output wire        port_ir_tx,
    output wire        port_ir_rx_disable,
    inout  wire        port_tran_si,
    output wire        port_tran_si_dir,
    inout  wire        port_tran_so,
    output wire        port_tran_so_dir,
    inout  wire        port_tran_sck,
    output wire        port_tran_sck_dir,
    inout  wire        port_tran_sd,
    output wire        port_tran_sd_dir,
    inout  wire [11:0] scal_vid,
    inout  wire        scal_clk,
    inout  wire        scal_de,
    inout  wire        scal_skip,
    inout  wire        scal_vs,
    inout  wire        scal_hs,
    output wire        scal_audmclk,
    input  wire        scal_audadc,
    output wire        scal_auddac,
    output wire        scal_audlrck,
    inout  wire        bridge_spimosi,
    inout  wire        bridge_spimiso,
    inout  wire        bridge_spiclk,
    input  wire        bridge_spiss,
    inout  wire        bridge_1wire,
    output wire [21:16] cram0_a,
    inout  wire [15:0]  cram0_dq,
    input  wire         cram0_wait,
    output wire         cram0_clk,
    output wire         cram0_adv_n,
    output wire         cram0_cre,
    output wire         cram0_ce0_n,
    output wire         cram0_ce1_n,
    output wire         cram0_oe_n,
    output wire         cram0_we_n,
    output wire         cram0_ub_n,
    output wire         cram0_lb_n,
    output wire [21:16] cram1_a,
    inout  wire [15:0]  cram1_dq,
    input  wire         cram1_wait,
    output wire         cram1_clk,
    output wire         cram1_adv_n,
    output wire         cram1_cre,
    output wire         cram1_ce0_n,
    output wire         cram1_ce1_n,
    output wire         cram1_oe_n,
    output wire         cram1_we_n,
    output wire         cram1_ub_n,
    output wire         cram1_lb_n,
    output wire [12:0]  dram_a,
    output wire [1:0]   dram_ba,
    inout  wire [15:0]  dram_dq,
    output wire [1:0]   dram_dqm,
    output wire         dram_clk,
    output wire         dram_cke,
    output wire         dram_ras_n,
    output wire         dram_cas_n,
    output wire         dram_we_n,
    output wire [16:0]  sram_a,
    inout  wire [15:0]  sram_dq,
    output wire         sram_oe_n,
    output wire         sram_we_n,
    output wire         sram_ub_n,
    output wire         sram_lb_n,
    input  wire         vblank,
    output wire         dbg_tx,
    input  wire         dbg_rx,
    output wire         user1,
    input  wire         user2,
    inout  wire         bist,
    output wire         vpll_feed,
    inout  wire         aux_sda,
    output wire         aux_scl
);
    reg [24:0] count;
    reg reset_n;

    wire [23:0] video_rgb;
    wire        video_rgb_clock;
    wire        video_rgb_clock_90;
    wire        video_de;
    wire        video_skip;
    wire        video_vs;
    wire        video_hs;
    wire        audio_mclk;
    wire        audio_dac;
    wire        audio_lrck;
    wire        bridge_endian_little;
    wire [31:0] bridge_addr;
    wire        bridge_rd;
    wire [31:0] bridge_rd_data;
    wire        bridge_wr;
    wire [31:0] bridge_wr_data;
    wire [31:0] cont1_key;
    wire [31:0] cont2_key;
    wire [31:0] cont3_key;
    wire [31:0] cont4_key;
    wire [31:0] cont1_joy;
    wire [31:0] cont2_joy;
    wire [31:0] cont3_joy;
    wire [31:0] cont4_joy;
    wire [15:0] cont1_trig;
    wire [15:0] cont2_trig;
    wire [15:0] cont3_trig;
    wire [15:0] cont4_trig;

    assign bist = 1'bz;
    assign bridge_addr = 32'h0;
    assign bridge_rd = 1'b0;
    assign bridge_wr = 1'b0;
    assign bridge_wr_data = 32'h0;
    assign cont1_key = 32'h0;
    assign cont2_key = 32'h0;
    assign cont3_key = 32'h0;
    assign cont4_key = 32'h0;
    assign cont1_joy = 32'h0;
    assign cont2_joy = 32'h0;
    assign cont3_joy = 32'h0;
    assign cont4_joy = 32'h0;
    assign cont1_trig = 16'h0;
    assign cont2_trig = 16'h0;
    assign cont3_trig = 16'h0;
    assign cont4_trig = 16'h0;
    assign bridge_spimosi = 1'bz;
    assign bridge_spimiso = 1'bz;
    assign bridge_spiclk = 1'bz;
    assign bridge_1wire = 1'bz;
    assign scal_vid = video_rgb[23:12];
    assign scal_clk = video_rgb_clock;
    assign scal_de = video_de;
    assign scal_skip = video_skip;
    assign scal_vs = video_vs;
    assign scal_hs = video_hs;
    assign scal_audmclk = audio_mclk;
    assign scal_auddac = audio_dac;
    assign scal_audlrck = audio_lrck;

    initial begin
        count = 25'd0;
        reset_n = 1'b0;
    end

    always @(posedge clk_74a) begin
        count <= count + 1'b1;
        if (count[15]) begin
            reset_n <= 1'b1;
        end
    end

    core_top ic (
        .clk_74a(clk_74a),
        .clk_74b(clk_74b),
        .reset_n(reset_n),
        .cart_tran_bank2(cart_tran_bank2),
        .cart_tran_bank2_dir(cart_tran_bank2_dir),
        .cart_tran_bank3(cart_tran_bank3),
        .cart_tran_bank3_dir(cart_tran_bank3_dir),
        .cart_tran_bank1(cart_tran_bank1),
        .cart_tran_bank1_dir(cart_tran_bank1_dir),
        .cart_tran_bank0(cart_tran_bank0),
        .cart_tran_bank0_dir(cart_tran_bank0_dir),
        .cart_tran_pin30(cart_tran_pin30),
        .cart_tran_pin30_dir(cart_tran_pin30_dir),
        .cart_pin30_pwroff_reset(cart_pin30_pwroff_reset),
        .cart_tran_pin31(cart_tran_pin31),
        .cart_tran_pin31_dir(cart_tran_pin31_dir),
        .port_ir_rx(port_ir_rx),
        .port_ir_tx(port_ir_tx),
        .port_ir_rx_disable(port_ir_rx_disable),
        .port_tran_si(port_tran_si),
        .port_tran_si_dir(port_tran_si_dir),
        .port_tran_so(port_tran_so),
        .port_tran_so_dir(port_tran_so_dir),
        .port_tran_sck(port_tran_sck),
        .port_tran_sck_dir(port_tran_sck_dir),
        .port_tran_sd(port_tran_sd),
        .port_tran_sd_dir(port_tran_sd_dir),
        .cram0_a(cram0_a),
        .cram0_dq(cram0_dq),
        .cram0_wait(cram0_wait),
        .cram0_clk(cram0_clk),
        .cram0_adv_n(cram0_adv_n),
        .cram0_cre(cram0_cre),
        .cram0_ce0_n(cram0_ce0_n),
        .cram0_ce1_n(cram0_ce1_n),
        .cram0_oe_n(cram0_oe_n),
        .cram0_we_n(cram0_we_n),
        .cram0_ub_n(cram0_ub_n),
        .cram0_lb_n(cram0_lb_n),
        .cram1_a(cram1_a),
        .cram1_dq(cram1_dq),
        .cram1_wait(cram1_wait),
        .cram1_clk(cram1_clk),
        .cram1_adv_n(cram1_adv_n),
        .cram1_cre(cram1_cre),
        .cram1_ce0_n(cram1_ce0_n),
        .cram1_ce1_n(cram1_ce1_n),
        .cram1_oe_n(cram1_oe_n),
        .cram1_we_n(cram1_we_n),
        .cram1_ub_n(cram1_ub_n),
        .cram1_lb_n(cram1_lb_n),
        .dram_a(dram_a),
        .dram_ba(dram_ba),
        .dram_dq(dram_dq),
        .dram_dqm(dram_dqm),
        .dram_clk(dram_clk),
        .dram_cke(dram_cke),
        .dram_ras_n(dram_ras_n),
        .dram_cas_n(dram_cas_n),
        .dram_we_n(dram_we_n),
        .sram_a(sram_a),
        .sram_dq(sram_dq),
        .sram_oe_n(sram_oe_n),
        .sram_we_n(sram_we_n),
        .sram_ub_n(sram_ub_n),
        .sram_lb_n(sram_lb_n),
        .vblank(vblank),
        .dbg_tx(dbg_tx),
        .dbg_rx(dbg_rx),
        .user1(user1),
        .user2(user2),
        .aux_sda(aux_sda),
        .aux_scl(aux_scl),
        .vpll_feed(vpll_feed),
        .video_rgb(video_rgb),
        .video_rgb_clock(video_rgb_clock),
        .video_rgb_clock_90(video_rgb_clock_90),
        .video_de(video_de),
        .video_skip(video_skip),
        .video_vs(video_vs),
        .video_hs(video_hs),
        .audio_mclk(audio_mclk),
        .audio_adc(scal_audadc),
        .audio_dac(audio_dac),
        .audio_lrck(audio_lrck),
        .bridge_endian_little(bridge_endian_little),
        .bridge_addr(bridge_addr),
        .bridge_rd(bridge_rd),
        .bridge_rd_data(bridge_rd_data),
        .bridge_wr(bridge_wr),
        .bridge_wr_data(bridge_wr_data),
        .cont1_key(cont1_key),
        .cont2_key(cont2_key),
        .cont3_key(cont3_key),
        .cont4_key(cont4_key),
        .cont1_joy(cont1_joy),
        .cont2_joy(cont2_joy),
        .cont3_joy(cont3_joy),
        .cont4_joy(cont4_joy),
        .cont1_trig(cont1_trig),
        .cont2_trig(cont2_trig),
        .cont3_trig(cont3_trig),
        .cont4_trig(cont4_trig)
    );
endmodule

`default_nettype wire

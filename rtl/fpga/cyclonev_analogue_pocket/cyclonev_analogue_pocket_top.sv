`default_nettype none

module cyclonev_analogue_pocket_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int BAUD_RATE = 9600,
    parameter string AUDIO_INIT_FILE = "./core/sine_table_init.hex"
) (
    input  wire logic       clk,
    input  wire logic       clk_video,
    input  wire logic       audio_mclk,  // Reserved MCLK input for the Pocket audio interface
    input  wire logic       audio_sclk,
    input  wire logic [31:0] cont1_key,
    input  wire logic [31:0] bridge_addr,
    input  wire logic       bridge_rd,
    output logic            bridge_rd_ready,
    input  wire logic       bridge_wr,
    output logic            bridge_wr_ready,
    input  wire logic [31:0] bridge_wr_data,
    input  wire logic       rst,
    input  wire logic       serial_rx,
    output logic            serial_tx,
    output logic            rst_out,
    output logic [31:0]     bridge_rd_data,
    output logic [23:0]     video_rgb,
    output logic            video_de,
    output logic            video_skip,
    output logic            video_vs,
    output logic            video_hs,
    output logic            audio_dac,
    output logic            audio_lrclk,
    // Analogue Pocket OS notify signals for external CPU boot control
    input  wire logic       play_cartridge,  // HIGH during cartridge-play mode; ext boot fires when play_cartridge is LOW and status_running is HIGH
    input  wire logic       status_running   // HIGH when core is running; ext boot requires this HIGH
);
    localparam int unsigned VIDEO_ACTIVE_WIDTH = 256;
    localparam int unsigned VIDEO_ACTIVE_HEIGHT = 224;
    localparam int unsigned VIDEO_TOTAL_WIDTH = 400;
    localparam int unsigned VIDEO_TOTAL_HEIGHT = 512;
    localparam int unsigned VIDEO_H_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_H_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_H_BACK_PORCH =
        VIDEO_TOTAL_WIDTH - VIDEO_ACTIVE_WIDTH - VIDEO_H_FRONT_PORCH - VIDEO_H_SYNC_WIDTH;
    localparam int unsigned VIDEO_V_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_V_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_V_BACK_PORCH =
        VIDEO_TOTAL_HEIGHT - VIDEO_ACTIVE_HEIGHT - VIDEO_V_FRONT_PORCH - VIDEO_V_SYNC_WIDTH;
    localparam int unsigned POCKET_RESET_CYCLES = 74_250_000 / 1000;  // Hold reset for 1 ms
    // Boot address driven into the CPU when the OS signals non-cartridge mode.
    // 32'h7000_0000 is the base of the SRAM peripheral window (slave 1 in the
    // registered_bus map at rtl/common/top.sv) where user SRAM images live.
    localparam logic [31:0] SRAM_BOOT_ADDR = 32'h7000_0000;

    // -----------------------------------------------------------------------
    // External CPU boot control registers
    // On reset both signals are 0. Once running: assert ext_cpu_boot whenever
    // play_cartridge is LOW, status_running is HIGH, and the CPU still reports
    // it is in the booting state; this boots the CPU into the SRAM image at
    // SRAM_BOOT_ADDR.
    // -----------------------------------------------------------------------
    logic        ext_boot_r;
    logic [31:0] ext_boot_addr_r;
    logic        cpu_is_booting;

    assign ext_boot_addr_r = SRAM_BOOT_ADDR;

    always_ff @(posedge clk) begin
        if (rst) begin
            ext_boot_r <= 1'b0;
        end else begin
            ext_boot_r <= !play_cartridge && status_running && cpu_is_booting;
        end
    end

    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .ENABLE_GFX2D(1'b1),
        .ENABLE_AUDIOSYS(1'b1),
        .ENABLE_APF_BUS_BRIDGE(1'b1),
        .ENABLE_GAMEPAD(1'b1),
        .CLK_FREQ_HZ(74_250_000),
        .RESET_CYCLES(POCKET_RESET_CYCLES),
        .BAUD_RATE(BAUD_RATE),
        .GFX2D_VIDEO_ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .GFX2D_VIDEO_ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .GFX2D_VIDEO_H_FRONT_PORCH(VIDEO_H_FRONT_PORCH),
        .GFX2D_VIDEO_H_SYNC_WIDTH(VIDEO_H_SYNC_WIDTH),
        .GFX2D_VIDEO_H_BACK_PORCH(VIDEO_H_BACK_PORCH),
        .GFX2D_VIDEO_V_FRONT_PORCH(VIDEO_V_FRONT_PORCH),
        .GFX2D_VIDEO_V_SYNC_WIDTH(VIDEO_V_SYNC_WIDTH),
        .GFX2D_VIDEO_V_BACK_PORCH(VIDEO_V_BACK_PORCH),
        .GFX2D_VIDEO_HSYNC_ACTIVE_HIGH(1'b1),
        .GFX2D_VIDEO_VSYNC_ACTIVE_HIGH(1'b1),
        .GFX2D_TILE_COLUMNS(32),
        .GFX2D_TILE_ROWS(32),
        .AUDIOSYS_INIT_FILE(AUDIO_INIT_FILE)
    ) repo_top_inst (
        .sys_clk(clk),
        .video_clk(clk_video),
        .audio_clk(audio_sclk),
        .rst(rst),
        .usb_rx(serial_rx),
        .usb_tx(serial_tx),
        .led_out(),
        .sys_led_out(),
        .rst_core(rst_out),
        // Bit mapping of cont1_key[9:0] → gamepad_in[9:0]:
        //   [0]=dpad_up [1]=dpad_down [2]=dpad_left [3]=dpad_right
        //   [4]=btn_a   [5]=btn_b     [6]=btn_x      [7]=btn_y
        //   [8]=trig_l  [9]=trig_r
        .gamepad_in(cont1_key[9:0]),
        .ext_cpu_boot(ext_boot_r),
        .ext_cpu_boot_addr(ext_boot_addr_r),
        .cpu_is_booting(cpu_is_booting),
        .apf_bridge_addr(bridge_addr),
        .apf_bridge_rd(bridge_rd),
        .apf_bridge_rd_ready(bridge_rd_ready),
        .apf_bridge_wr(bridge_wr),
        .apf_bridge_wr_ready(bridge_wr_ready),
        .apf_bridge_wr_data(bridge_wr_data),
        .apf_bridge_rd_data(bridge_rd_data),
        .video_rgb(video_rgb),
        .video_de(video_de),
        .video_skip(video_skip),
        .video_vs(video_vs),
        .video_hs(video_hs),
        .audio_dac(audio_dac),
        .audio_lrclk(audio_lrclk)
    );

endmodule

`default_nettype wire

`default_nettype none

module analogue_pocket_repo_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter string FONT_INIT_FILE = "./core/bitmap_text_renderer_font_init.hex",
    parameter string CHAR_MAP_INIT_FILE = "./core/bitmap_text_renderer_char_map_init.hex"
) (
    input  wire logic       clk,
    input  wire logic       clk_video,
    input  wire logic       reset_n,
    output logic [7:0]      led_out,
    output logic [7:0]      sys_led_out,
    output logic            halted,
    output logic            instr_complete,
    output logic            rst_out,
    output logic            cpu_booting,
    output logic [31:0]     halted_value,
    output logic [23:0]     video_rgb,
    output logic            video_de,
    output logic            video_skip,
    output logic            video_vs,
    output logic            video_hs
);
    logic rst;
    logic reset_n_video_sync;
    logic video_rst;
    logic [7:0] host_tx_data_unused;
    logic       host_tx_valid_unused;
    logic       host_rx_ready_unused;
    logic [31:0] debug_rs1_data_unused;
    logic [31:0] debug_rs2_data_unused;
    logic [31:0] debug_rd_data_unused;
    logic [31:0] debug_pc_unused;
    logic [31:0] debug_instruction_unused;
    logic [31:0] debug_current_pc_unused;
    logic [31:0] debug_current_instruction_unused;
    logic [3:0]  debug_fsm_state_unused;
    logic        bitmap_video_de;
    logic        bitmap_video_hs;
    logic        bitmap_video_vs;
    logic        bitmap_pixel_on;
    logic [23:0] video_rgb_reg;
    logic        video_de_reg;
    logic        video_skip_reg;
    logic        video_vs_reg;
    logic        video_hs_reg;

    localparam int unsigned VIDEO_ACTIVE_WIDTH = 320;
    localparam int unsigned VIDEO_ACTIVE_HEIGHT = 240;
    localparam int unsigned VIDEO_H_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_H_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_H_BACK_PORCH = 69;
    localparam int unsigned VIDEO_V_FRONT_PORCH = 10;
    localparam int unsigned VIDEO_V_SYNC_WIDTH = 1;
    localparam int unsigned VIDEO_V_BACK_PORCH = 261;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            rst <= 1'b1;
        end else begin
            rst <= 1'b0;
        end
    end

    ff_sync #(
        .STAGES(3),
        .WIDTH(1),
        .RESET_VALUE(1'b0)
    ) video_reset_sync (
        .clk(clk_video),
        .rst(1'b0),
        .din(reset_n),
        .dout(reset_n_video_sync)
    );

    assign video_rst = !reset_n_video_sync;

    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(74_250_000),
        .RESET_CYCLES(74_250_000)
    ) repo_top_inst (
        .clk(clk),
        .rst(rst),
        .host_tx_data(host_tx_data_unused),
        .host_tx_valid(host_tx_valid_unused),
        .host_tx_ready(1'b1),
        .host_rx_data(8'h00),
        .host_rx_valid(1'b0),
        .host_rx_ready(host_rx_ready_unused),
        .com_err(1'b0),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .halted(halted),
        .instr_complete(instr_complete),
        .debug_rs1_data(debug_rs1_data_unused),
        .debug_rs2_data(debug_rs2_data_unused),
        .debug_rd_data(debug_rd_data_unused),
        .debug_pc(debug_pc_unused),
        .debug_instruction(debug_instruction_unused),
        .debug_current_pc(debug_current_pc_unused),
        .debug_current_instruction(debug_current_instruction_unused),
        .debug_fsm_state(debug_fsm_state_unused),
        .rst_out(rst_out),
        .cpu_booting(cpu_booting),
        .halted_value(halted_value)
    );

    bitmap_text_renderer #(
        .ACTIVE_WIDTH(VIDEO_ACTIVE_WIDTH),
        .ACTIVE_HEIGHT(VIDEO_ACTIVE_HEIGHT),
        .H_FRONT_PORCH(VIDEO_H_FRONT_PORCH),
        .H_SYNC_WIDTH(VIDEO_H_SYNC_WIDTH),
        .H_BACK_PORCH(VIDEO_H_BACK_PORCH),
        .V_FRONT_PORCH(VIDEO_V_FRONT_PORCH),
        .V_SYNC_WIDTH(VIDEO_V_SYNC_WIDTH),
        .V_BACK_PORCH(VIDEO_V_BACK_PORCH),
        .HSYNC_ACTIVE_HIGH(1'b1),
        .VSYNC_ACTIVE_HIGH(1'b1),
        .FONT_INIT_FILE(FONT_INIT_FILE),
        .CHAR_MAP_INIT_FILE(CHAR_MAP_INIT_FILE)
    ) pocket_bitmap_text_renderer (
        .clk(clk_video),
        .rst(video_rst),
        .video_de(bitmap_video_de),
        .video_hs(bitmap_video_hs),
        .video_vs(bitmap_video_vs),
        .line_start(),
        .frame_start(),
        .active_x(),
        .active_y(),
        .pixel_on(bitmap_pixel_on)
    );

    always_ff @(posedge clk_video) begin
        if (video_rst) begin
            video_de_reg <= 1'b0;
            video_skip_reg <= 1'b0;
            video_vs_reg <= 1'b0;
            video_hs_reg <= 1'b0;
        end else begin
            video_rgb_reg <= bitmap_pixel_on ? 24'hFF_FF_FF : 24'h00_00_00;
            video_de_reg <= bitmap_video_de;
            video_skip_reg <= 1'b0;
            video_vs_reg <= bitmap_video_vs;
            video_hs_reg <= bitmap_video_hs;
        end
    end

    assign video_rgb = video_rgb_reg;
    assign video_de = video_de_reg;
    assign video_skip = video_skip_reg;
    assign video_vs = video_vs_reg;
    assign video_hs = video_hs_reg;
endmodule

`default_nettype wire

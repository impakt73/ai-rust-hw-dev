// FPGA Top-Level Module for iCE Pi Zero (ECP5-25F)
// Wraps RISC-V CPU with host communication via USB serial

module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0
) (
    input  logic clk,
    input  logic rst_n_btn,
    output logic led,
    input  logic usb_rx,
    output logic usb_tx
);
    // iCE Pi Zero board clock is 50 MHz
    logic sys_clk;
    assign sys_clk = clk;

    // Synchronize reset button (active low) to system clock domain
    logic rst_n_btn_sync2;
    ff_sync #(
        .STAGES(2),
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(sys_clk),
        .rst_n(1'b1),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );

    logic reset_request;
    assign reset_request = ~rst_n_btn_sync2;

    logic [7:0] host_tx_data;
    logic       host_tx_valid;
    logic       host_tx_ready;
    logic [7:0] host_rx_data;
    logic       host_rx_valid;
    logic       host_rx_ready;
    logic       com_err;

    logic [7:0]  led_out;
    logic [7:0]  sys_led_out;
    logic        halted;
    logic        instr_complete;
    logic [31:0] debug_rs1_data;
    logic [31:0] debug_rs2_data;
    logic [31:0] debug_rd_data;
    logic [31:0] debug_pc;
    logic [31:0] debug_instruction;
    logic [31:0] debug_current_pc;
    logic [31:0] debug_current_instruction;
    logic [3:0]  debug_fsm_state;
    logic        rst_n;

    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(50_000_000),
        .RESET_CYCLES(50_000_000)
    ) cpu_inst (
        .clk(sys_clk),
        .rst_n(rst_n_btn_sync2),
        .reset_request(reset_request),
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
        .rst_n_out(rst_n)
    );

    uart #(
        .CLK_FREQ_HZ(50_000_000),
        .BAUD_RATE(1_000_000)
    ) host_uart_inst (
        .clk(sys_clk),
        .rst_n(rst_n),
        .tx_data(host_tx_data),
        .tx_valid(host_tx_valid),
        .tx_ready(host_tx_ready),
        .rx_data(host_rx_data),
        .rx_valid(host_rx_valid),
        .rx_ready(host_rx_ready),
        .rx_error(com_err),
        .rx_error_clr(1'b0),
        .tx_out(usb_tx),
        .rx_in(usb_rx)
    );

    assign led = sys_led_out[0];

endmodule

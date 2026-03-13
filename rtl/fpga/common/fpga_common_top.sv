`default_nettype none
module fpga_common_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0,
    parameter int CLK_FREQ_HZ = 25_000_000,
    parameter int RESET_CYCLES = 25_000_000
) (
    input wire logic       sys_clk,
    input wire logic       rst_n,
    input wire logic       usb_rx,
    output logic       usb_tx,
    output logic [7:0] led_out,
    output logic [7:0] sys_led_out,
    output logic       rst_n_core
);
    logic [7:0] host_tx_data;
    logic       host_tx_valid;
    logic       host_tx_ready;
    logic [7:0] host_rx_data;
    logic       host_rx_valid;
    logic       host_rx_ready;
    logic       com_err;
    logic       halted;
    logic       instr_complete;
    logic [31:0] debug_rs1_data;
    logic [31:0] debug_rs2_data;
    logic [31:0] debug_rd_data;
    logic [31:0] debug_pc;
    logic [31:0] debug_instruction;
    logic [31:0] debug_current_pc;
    logic [31:0] debug_current_instruction;
    logic [3:0]  debug_fsm_state;

    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .RESET_CYCLES(RESET_CYCLES)
    ) cpu_inst (
        .clk(sys_clk),
        .rst_n(rst_n),
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
        .rst_n_out(rst_n_core)
    );

    uart #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .BAUD_RATE(1_000_000)
    ) host_uart_inst (
        .clk(sys_clk),
        .rst_n(rst_n_core),
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

endmodule
`default_nettype wire

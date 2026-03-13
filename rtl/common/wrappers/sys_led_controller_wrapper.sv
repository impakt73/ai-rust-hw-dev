`default_nettype none
module sys_led_controller_wrapper (
    input wire logic       clk,
    input wire logic       rst_n,
    input wire logic       cpu_booting,
    input wire logic       cpu_halted,
    input wire logic       instr_complete,
    input wire logic       sys_bus_handshake,
    input wire logic       host_bus_rx_handshake,
    input wire logic       host_bus_tx_handshake,
    input wire logic       com_err,
    output logic [7:0] sys_led
);

    sys_led_controller #(
        .CLK_FREQ_HZ(16)
    ) u_sys_led_controller (
        .clk(clk),
        .rst_n(rst_n),
        .cpu_booting(cpu_booting),
        .cpu_halted(cpu_halted),
        .instr_complete(instr_complete),
        .sys_bus_handshake(sys_bus_handshake),
        .host_bus_rx_handshake(host_bus_rx_handshake),
        .host_bus_tx_handshake(host_bus_tx_handshake),
        .com_err(com_err),
        .sys_led(sys_led)
    );

endmodule
`default_nettype wire

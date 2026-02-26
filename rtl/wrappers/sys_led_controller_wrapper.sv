module sys_led_controller_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       cpu_booting,
    input  logic       cpu_halted,
    input  logic       instr_complete,
    input  logic       sys_bus_handshake,
    input  logic       host_bus_rx_handshake,
    input  logic       host_bus_tx_handshake,
    input  logic       com_err,
    output logic [7:0] sys_led
);

    sys_led_controller #(
        .CLK_FREQ_HZ(4)
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

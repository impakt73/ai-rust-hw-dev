module sys_led_controller_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       cpu_booting,
    input  logic       cpu_halted,
    output logic [7:0] sys_led
);

    sys_led_controller #(
        .CLK_FREQ_HZ(4)
    ) u_sys_led_controller (
        .clk(clk),
        .rst_n(rst_n),
        .cpu_booting(cpu_booting),
        .cpu_halted(cpu_halted),
        .sys_led(sys_led)
    );

endmodule

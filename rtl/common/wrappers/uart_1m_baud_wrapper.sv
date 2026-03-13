`default_nettype none
module uart_1m_baud_wrapper (
    input wire logic       clk,
    input wire logic       rst_n,
    input wire logic [7:0] tx_data,
    input wire logic       tx_valid,
    output logic       tx_ready,
    output logic [7:0] rx_data,
    output logic       rx_valid,
    input wire logic       rx_ready,
    output logic       rx_error,
    input wire logic       rx_error_clr,
    output logic       tx_out,
    input wire logic       rx_in
);
    uart #(
        .CLK_FREQ_HZ(25_000_000),
        .BAUD_RATE(1_000_000)
    ) uart_1m_inst (
        .clk(clk),
        .rst_n(rst_n),
        .tx_data(tx_data),
        .tx_valid(tx_valid),
        .tx_ready(tx_ready),
        .rx_data(rx_data),
        .rx_valid(rx_valid),
        .rx_ready(rx_ready),
        .rx_error(rx_error),
        .rx_error_clr(rx_error_clr),
        .tx_out(tx_out),
        .rx_in(rx_in)
    );
endmodule
`default_nettype wire

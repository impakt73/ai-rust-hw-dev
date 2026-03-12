`default_nettype none
// UART Validation Top Module for Alchitry Cu v1
// Validation modes:
//   0: External serial pin loopback (usb_rx -> usb_tx)
//   1: UART echo (usb_rx -> uart -> immediate tx)
//   2: UART echo with sync FIFO buffering

module uart_validation_top #(
    parameter int VALIDATION_MODE = 2,
    parameter int CLK_FREQ_HZ = 25_000_000,
    parameter int BAUD_RATE = 1_000_000,
    parameter int FIFO_DEPTH = 8
) (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,

    // Reset button (active low)
    input  logic       rst_n_btn,

    // LED outputs (8 LEDs on Alchitry Cu main board)
    output logic [7:0] led,

    // USB Serial
    input  logic       usb_rx,
    output logic       usb_tx
);

    // ============================================================
    // PLL Configuration - Generate 25 MHz from 100 MHz input
    // ============================================================
    logic pll_clk_global;
    logic pll_locked;

    SB_PLL40_CORE #(
        .FEEDBACK_PATH("SIMPLE"),
        .DIVR(4'b0000),
        .DIVF(7'b0000111),
        .DIVQ(3'b101),
        .FILTER_RANGE(3'b001)
    ) pll_inst (
        .REFERENCECLK(clk),
        .PLLOUTCORE(),
        .PLLOUTGLOBAL(pll_clk_global),
        .LOCK(pll_locked),
        .BYPASS(1'b0),
        .RESETB(1'b1)
    );

    logic sys_clk;
    assign sys_clk = pll_clk_global;

    // ============================================================
    // Reset Synchronization
    // ============================================================
    logic pll_locked_sync2;
    ff_sync #(
        .WIDTH(1)
    ) pll_locked_sync_inst (
        .clk(sys_clk),
        .rst_n(1'b1),
        .din(pll_locked),
        .dout(pll_locked_sync2)
    );

    logic rst_n_btn_sync2;
    ff_sync #(
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(sys_clk),
        .rst_n(1'b1),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );

    logic rst_n;
    assign rst_n = pll_locked_sync2 & rst_n_btn_sync2;

    // ============================================================
    // UART + FIFO Signals
    // ============================================================
    logic [7:0] uart_tx_data;
    logic       uart_tx_valid;
    logic       uart_tx_ready;
    logic [7:0] uart_rx_data;
    logic       uart_rx_valid;
    logic       uart_rx_ready;
    logic       uart_rx_error;

    logic [7:0] fifo_rdata;
    logic       fifo_wr_ready;
    logic       fifo_rd_valid;
    logic [$clog2(FIFO_DEPTH):0] fifo_count;
    logic       fifo_wr_valid;
    logic       fifo_rd_ready;

    // ============================================================
    // Validation Mode Selection
    // ============================================================
    generate
        if (VALIDATION_MODE == 0) begin : gen_pin_loopback
            assign usb_tx = usb_rx;
            assign led = 8'h01;
        end else begin : gen_uart_modes
            uart #(
                .CLK_FREQ_HZ(CLK_FREQ_HZ),
                .BAUD_RATE(BAUD_RATE)
            ) uart_inst (
                .clk(sys_clk),
                .rst_n(rst_n),
                .tx_data(uart_tx_data),
                .tx_valid(uart_tx_valid),
                .tx_ready(uart_tx_ready),
                .rx_data(uart_rx_data),
                .rx_valid(uart_rx_valid),
                .rx_ready(uart_rx_ready),
                .rx_error(uart_rx_error),
                .rx_error_clr(1'b0),
                .tx_out(usb_tx),
                .rx_in(usb_rx)
            );

            if (VALIDATION_MODE == 1) begin : gen_uart_echo
                assign uart_tx_data = uart_rx_data;
                assign uart_tx_valid = uart_rx_valid;
                assign uart_rx_ready = uart_tx_ready;

                assign led[0] = 1'b0;
                assign led[1] = 1'b1;
                assign led[2] = uart_rx_valid;
                assign led[3] = uart_tx_ready;
                assign led[4] = uart_rx_error;
                assign led[7:5] = 3'b000;
            end else begin : gen_uart_fifo_echo
                assign fifo_wr_valid = uart_rx_valid;
                assign uart_rx_ready = fifo_wr_ready;

                assign uart_tx_data = fifo_rdata;
                assign uart_tx_valid = fifo_rd_valid;
                assign fifo_rd_ready = uart_tx_ready;

                sync_fifo #(
                    .WIDTH(8),
                    .DEPTH(FIFO_DEPTH)
                ) fifo_inst (
                    .clk(sys_clk),
                    .rst_n(rst_n),
                    .wr_valid(fifo_wr_valid),
                    .wr_ready(fifo_wr_ready),
                    .wdata(uart_rx_data),
                    .rd_valid(fifo_rd_valid),
                    .rd_ready(fifo_rd_ready),
                    .rdata(fifo_rdata),
                    .count(fifo_count)
                );

                assign led[0] = 1'b1;
                assign led[1] = 1'b0;
                assign led[2] = uart_rx_valid;
                assign led[3] = uart_tx_ready;
                assign led[4] = uart_rx_error;
                assign led[5] = fifo_wr_valid && fifo_wr_ready;
                assign led[6] = (fifo_count == '0);
                assign led[7] = (fifo_count == FIFO_DEPTH);
            end
        end
    endgenerate

endmodule
`default_nettype wire

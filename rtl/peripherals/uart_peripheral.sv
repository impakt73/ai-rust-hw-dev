// UART Peripheral
// 8N1 UART (8 data bits, no parity, 1 stop bit) with FIFOs
// Memory-mapped at 0x52000000 in RTL peripheral address space
// Uses the generic uart.sv module for TX/RX logic
//
// Register Map:
//   Offset | Name   | Access | Description
//   -------|--------|--------|------------------------------------------
//   0x00   | TXDATA | WO     | Transmit data (write byte to TX FIFO)
//   0x04   | RXDATA | RO     | Receive data (read byte from RX FIFO)
//   0x08   | STATUS | RO     | Status register (see bit definitions below)
//   0x0C   | CTRL   | RW     | Control register (reserved for future use)
//
// STATUS Register Bits [7:0]:
//   [0] TX_FULL   - TX FIFO is full (cannot accept more data)
//   [1] TX_EMPTY  - TX FIFO is empty AND transmitter idle
//   [2] TX_BUSY   - TX shift register is actively transmitting
//   [3] Reserved  - (always 0)
//   [4] RX_FULL   - RX FIFO is full (incoming data will be lost)
//   [5] RX_EMPTY  - RX FIFO is empty (no data available)
//   [6] RX_BUSY   - RX shift register is actively receiving
//   [7] RX_ERROR  - Framing error (missing stop bit), cleared on STATUS read
//
// Features:
//   - Configurable baud rate via CLK_FREQ_HZ and BAUD_RATE parameters
//   - 8-entry TX and RX FIFOs
//   - Uses generic uart.sv module for TX/RX (16x RX oversampling, 2-FF sync)
//   - Single-cycle ready (always 1'b1)

/* verilator lint_off UNUSEDSIGNAL */
module uart_peripheral #(
    // System clock frequency in Hz (required for baud rate calculation)
    parameter int CLK_FREQ_HZ = 50_000_000,
    // Target baud rate in bits per second
    parameter int BAUD_RATE = 115200,
    // FIFO depth (number of entries in TX and RX FIFOs)
    parameter int FIFO_DEPTH = 8
) (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit, only [7:0] used)
    input  logic [31:0] wdata,     // Write data (only [7:0] used for data)
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable
    input  logic        req,       // Memory request
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word) - reserved
    output logic        ready,     // Operation complete (always ready)
    
    // External UART pins
    output logic        tx_out,    // Serial transmit output (idle high)
    input  logic        rx_in      // Serial receive input
);
/* verilator lint_on UNUSEDSIGNAL */

    // ============================================================
    // Parameter Validation
    // ============================================================
    
    // Parameter validation (simulation only)
    initial begin
        // Note: Baud rate validation is handled by the uart module
        // Note: FIFO_DEPTH validation is handled by the sync_fifo module
    end
    
    // ============================================================
    // TX FIFO Instance
    // ============================================================
    
    // TX FIFO control signals
    logic tx_fifo_wr_en;
    logic tx_fifo_rd_en;
    logic [7:0] tx_fifo_rdata;
    logic tx_fifo_full;
    logic tx_fifo_empty;
    
    // ============================================================
    // RX FIFO Instance
    // ============================================================
    
    // RX FIFO control signals
    logic rx_fifo_wr_en;
    logic rx_fifo_rd_en;
    logic [7:0] rx_fifo_rdata;
    logic rx_fifo_full;
    logic rx_fifo_empty;
    
    // ============================================================
    // UART Core Instance (TX/RX Logic)
    // ============================================================
    
    // UART TX interface
    logic [7:0] uart_tx_data;
    logic       uart_tx_valid;
    logic       uart_tx_ready;
    
    // UART RX interface
    logic [7:0] uart_rx_data;
    logic       uart_rx_valid;
    logic       uart_rx_ready;
    logic       uart_rx_error;
    
    // ============================================================
    // Register Address Decoding
    // ============================================================
    
    // UART is single-cycle - always ready
    assign ready = 1'b1;
    
    // Register offset decode (byte offset within 256B UART window)
    logic [7:0] reg_offset;
    assign reg_offset = addr[7:0];
    
    // ============================================================
    // UART Core Instantiation
    // ============================================================
    
    // RX Error clear signal - clear uart's rx_error when STATUS register is read
    logic clear_rx_error;
    assign clear_rx_error = req && !we && (reg_offset == 8'h08);
    
    uart #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .BAUD_RATE(BAUD_RATE)
    ) uart_inst (
        .clk(clk),
        .rst_n(rst_n),
        
        // TX interface
        .tx_data(uart_tx_data),
        .tx_valid(uart_tx_valid),
        .tx_ready(uart_tx_ready),
        
        // RX interface
        .rx_data(uart_rx_data),
        .rx_valid(uart_rx_valid),
        .rx_ready(uart_rx_ready),
        .rx_error(uart_rx_error),
        .rx_error_clr(clear_rx_error),
        
        // Serial pins
        .tx_out(tx_out),
        .rx_in(rx_in)
    );
    
    // ============================================================
    // TX Path: FIFO → UART
    // ============================================================
    
    // Connect TX FIFO to UART module
    assign uart_tx_data = tx_fifo_rdata;
    assign uart_tx_valid = !tx_fifo_empty;
    assign tx_fifo_rd_en = uart_tx_ready && !tx_fifo_empty;
    
    // ============================================================
    // RX Path: UART → FIFO
    // ============================================================
    
    // Connect UART to RX FIFO
    assign uart_rx_ready = !rx_fifo_full;
    assign rx_fifo_wr_en = uart_rx_valid && !rx_fifo_full;
    
    // ============================================================
    // TX FIFO Instance and Control
    // ============================================================
    
    // TX FIFO write enable (CPU writes to TXDATA register)
    assign tx_fifo_wr_en = we && (reg_offset == 8'h00);
    
    sync_fifo #(
        .WIDTH(8),
        .DEPTH(FIFO_DEPTH)
    ) tx_fifo_inst (
        .clk(clk),
        .rst_n(rst_n),
        .wr_en(tx_fifo_wr_en),
        .wdata(wdata[7:0]),
        .rd_en(tx_fifo_rd_en),
        .rdata(tx_fifo_rdata),
        .full(tx_fifo_full),
        .empty(tx_fifo_empty),
        .count()  // Not used
    );
    
    // ============================================================
    // RX FIFO Instance and Control
    // ============================================================
    
    // RX FIFO read enable (CPU reads from RXDATA register)
    // Read occurs when req is asserted and we is not (read intent implied)
    assign rx_fifo_rd_en = req && !we && (reg_offset == 8'h04);
    
    sync_fifo #(
        .WIDTH(8),
        .DEPTH(FIFO_DEPTH)
    ) rx_fifo_inst (
        .clk(clk),
        .rst_n(rst_n),
        .wr_en(rx_fifo_wr_en),
        .wdata(uart_rx_data),
        .rd_en(rx_fifo_rd_en),
        .rdata(rx_fifo_rdata),
        .full(rx_fifo_full),
        .empty(rx_fifo_empty),
        .count()  // Not used
    );
    
    // ============================================================
    // Register Read Logic
    // ============================================================
    
    // TX_EMPTY: FIFO empty AND transmitter idle
    logic tx_empty_status;
    assign tx_empty_status = tx_fifo_empty && uart_tx_ready;
    
    // TX_BUSY: UART is actively transmitting (not ready)
    logic tx_busy_status;
    assign tx_busy_status = !uart_tx_ready;
    
    // RX_BUSY: UART has received data but hasn't been acknowledged yet
    logic rx_busy_status;
    assign rx_busy_status = uart_rx_valid && !uart_rx_ready;
    
    // Read data mux
    // Read occurs when req is asserted and we is not (read intent implied)
    always_comb begin
        rdata = 32'h0;
        
        if (req && !we) begin
            case (reg_offset)
                8'h00: rdata = 32'h0;  // TXDATA is write-only
                8'h04: rdata = rx_fifo_empty ? 32'h0 : {24'h0, rx_fifo_rdata};  // RXDATA
                8'h08: rdata = {24'h0,                      // STATUS
                              uart_rx_error,                // [7] RX_ERROR (sticky, cleared on STATUS read)
                              rx_busy_status,               // [6] RX_BUSY
                              rx_fifo_empty,                // [5] RX_EMPTY
                              rx_fifo_full,                 // [4] RX_FULL
                              1'b0,                         // [3] Reserved
                              tx_busy_status,               // [2] TX_BUSY
                              tx_empty_status,              // [1] TX_EMPTY
                              tx_fifo_full};                // [0] TX_FULL
                8'h0C: rdata = 32'h0;  // CTRL (reserved)
                default: rdata = 32'h0;
            endcase
        end
    end

endmodule

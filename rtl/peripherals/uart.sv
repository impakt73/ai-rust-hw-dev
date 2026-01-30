// UART Peripheral
// 8N1 UART (8 data bits, no parity, 1 stop bit) with FIFOs
// Memory-mapped at 0x52000000 in RTL peripheral address space
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
//   - 16x oversampling on RX for robust bit detection
//   - 2-FF input synchronizer prevents metastability
//   - Single-cycle ready (always 1'b1)

/* verilator lint_off UNUSEDSIGNAL */
module uart #(
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
    // Parameter Validation and Baud Rate Calculation
    // ============================================================
    
    // Calculate clock divisor at compile time
    localparam int CLKS_PER_BIT = CLK_FREQ_HZ / BAUD_RATE;
    
    // RX oversampling (16x for robust start bit detection)
    localparam int CLKS_PER_SAMPLE = CLKS_PER_BIT / 16;
    
    // Parameter validation (simulation only)
    initial begin
        // Validate FIFO depth is power of 2
        if ((FIFO_DEPTH & (FIFO_DEPTH - 1)) != 0 || FIFO_DEPTH < 2) begin
            $fatal(1, "UART: FIFO_DEPTH must be power of 2 and >= 2, got %0d", FIFO_DEPTH);
        end
        
        // Validate baud rate is achievable with given clock
        if (CLK_FREQ_HZ / BAUD_RATE < 16) begin
            $fatal(1, "UART: Baud rate %0d too high for clock %0d Hz (need 16x oversampling)",
                   BAUD_RATE, CLK_FREQ_HZ);
        end
    end
    
    // ============================================================
    // TX FIFO and State Machine
    // ============================================================
    
    // TX FIFO storage
    logic [7:0] tx_fifo [0:FIFO_DEPTH-1];
    logic [$clog2(FIFO_DEPTH)-1:0] tx_wr_ptr;
    logic [$clog2(FIFO_DEPTH)-1:0] tx_rd_ptr;
    logic [$clog2(FIFO_DEPTH):0]   tx_fifo_count;  // Extra bit for full detection
    
    // TX FIFO status flags
    logic tx_fifo_full;
    logic tx_fifo_empty;
    assign tx_fifo_full  = (tx_fifo_count == FIFO_DEPTH[$clog2(FIFO_DEPTH):0]);
    assign tx_fifo_empty = (tx_fifo_count == '0);
    
    // TX State Machine
    typedef enum logic [1:0] {
        TX_IDLE,
        TX_START_BIT,
        TX_DATA_BITS,
        TX_STOP_BIT
    } tx_state_t;
    
    tx_state_t tx_state;
    logic [7:0] tx_shift_reg;
    logic [2:0] tx_bit_index;  // 0-7 for 8 data bits
    logic [$clog2(CLKS_PER_BIT)-1:0] tx_baud_counter;
    logic tx_busy;
    logic tx_baud_tick;
    
    assign tx_baud_tick = (tx_baud_counter == '0);
    
    // ============================================================
    // RX FIFO and State Machine
    // ============================================================
    
    // RX input synchronizer (2-FF for metastability)
    logic rx_sync_0, rx_sync_1;
    
    // RX FIFO storage
    logic [7:0] rx_fifo [0:FIFO_DEPTH-1];
    logic [$clog2(FIFO_DEPTH)-1:0] rx_wr_ptr;
    logic [$clog2(FIFO_DEPTH)-1:0] rx_rd_ptr;
    logic [$clog2(FIFO_DEPTH):0]   rx_fifo_count;
    
    // RX FIFO status flags
    logic rx_fifo_full;
    logic rx_fifo_empty;
    assign rx_fifo_full  = (rx_fifo_count == FIFO_DEPTH[$clog2(FIFO_DEPTH):0]);
    assign rx_fifo_empty = (rx_fifo_count == '0);
    
    // RX State Machine
    typedef enum logic [1:0] {
        RX_IDLE,
        RX_START_BIT,
        RX_DATA_BITS,
        RX_STOP_BIT
    } rx_state_t;
    
    rx_state_t rx_state;
    logic [7:0] rx_shift_reg;
    logic [2:0] rx_bit_index;
    logic [3:0] rx_sample_count;  // 0-15 for 16x oversampling
    logic [(CLKS_PER_SAMPLE > 1) ? $clog2(CLKS_PER_SAMPLE)-1 : 0 : 0] rx_baud_counter;
    logic rx_busy;
    logic rx_error;
    logic rx_fifo_write_int;  // Pulses high for one cycle when writing to RX FIFO
    
    // ============================================================
    // Register Address Decoding
    // ============================================================
    
    // UART is single-cycle - always ready
    assign ready = 1'b1;
    
    // Register offset decode (byte offset within 256B UART window)
    logic [7:0] reg_offset;
    assign reg_offset = addr[7:0];
    
    // ============================================================
    // TX State Machine Logic
    // ============================================================
    
    // TX FIFO read signal (TX state machine reads from FIFO)
    logic tx_fifo_read_int;
    assign tx_fifo_read_int = (tx_state == TX_IDLE) && !tx_fifo_empty;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            tx_state <= TX_IDLE;
            tx_out <= 1'b1;  // Idle high
            tx_shift_reg <= 8'h00;
            tx_bit_index <= 3'b0;
            tx_baud_counter <= '0;
            tx_busy <= 1'b0;
        end else begin
            case (tx_state)
                TX_IDLE: begin
                    tx_out <= 1'b1;
                    tx_busy <= 1'b0;
                    if (!tx_fifo_empty) begin
                        // Load byte from FIFO
                        tx_shift_reg <= tx_fifo[tx_rd_ptr];
                        tx_state <= TX_START_BIT;
                        tx_baud_counter <= CLKS_PER_BIT[$clog2(CLKS_PER_BIT)-1:0] - 1'b1;
                        tx_busy <= 1'b1;
                    end
                end
                
                TX_START_BIT: begin
                    tx_out <= 1'b0;  // Start bit is low
                    if (tx_baud_tick) begin
                        tx_baud_counter <= CLKS_PER_BIT[$clog2(CLKS_PER_BIT)-1:0] - 1'b1;
                        tx_state <= TX_DATA_BITS;
                        tx_bit_index <= 3'b0;
                    end else begin
                        tx_baud_counter <= tx_baud_counter - 1'b1;
                    end
                end
                
                TX_DATA_BITS: begin
                    tx_out <= tx_shift_reg[0];  // LSB first
                    if (tx_baud_tick) begin
                        tx_shift_reg <= {1'b0, tx_shift_reg[7:1]};  // Shift right
                        if (tx_bit_index == 3'd7) begin
                            tx_state <= TX_STOP_BIT;
                        end else begin
                            tx_bit_index <= tx_bit_index + 1'b1;
                        end
                        tx_baud_counter <= CLKS_PER_BIT[$clog2(CLKS_PER_BIT)-1:0] - 1'b1;
                    end else begin
                        tx_baud_counter <= tx_baud_counter - 1'b1;
                    end
                end
                
                TX_STOP_BIT: begin
                    tx_out <= 1'b1;  // Stop bit is high
                    if (tx_baud_tick) begin
                        tx_state <= TX_IDLE;
                    end else begin
                        tx_baud_counter <= tx_baud_counter - 1'b1;
                    end
                end
                
                default: tx_state <= TX_IDLE;
            endcase
        end
    end
    
    // ============================================================
    // RX Input Synchronizer and State Machine
    // ============================================================
    
    // 2-FF synchronizer for metastability
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_sync_0 <= 1'b1;
            rx_sync_1 <= 1'b1;
        end else begin
            rx_sync_0 <= rx_in;
            rx_sync_1 <= rx_sync_0;
        end
    end
    
    // RX error management
    // Clear error when reading STATUS register (read occurs when req && !we)
    logic clear_rx_error;
    assign clear_rx_error = req && !we && (reg_offset == 8'h08);
    
    // RX State Machine
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_state <= RX_IDLE;
            rx_shift_reg <= 8'h00;
            rx_bit_index <= 3'b0;
            rx_sample_count <= 4'b0;
            rx_baud_counter <= '0;
            rx_busy <= 1'b0;
            rx_error <= 1'b0;
            rx_fifo_write_int <= 1'b0;
        end else begin
            // Handle rx_error clearing FIRST (from STATUS register read)
            // This ensures error is visible for at least one cycle before clearing
            if (clear_rx_error) begin
                rx_error <= 1'b0;
            end
            
            // Default: clear write pulse
            rx_fifo_write_int <= 1'b0;
            
            case (rx_state)
                RX_IDLE: begin
                    rx_busy <= 1'b0;
                    rx_sample_count <= 4'd0;  // Reset sample count in idle
                    if (rx_sync_1 == 1'b0) begin  // Falling edge detected (start bit)
                        rx_state <= RX_START_BIT;
                        rx_sample_count <= 4'd0;
                        rx_baud_counter <= CLKS_PER_SAMPLE[$clog2(CLKS_PER_SAMPLE)-1:0] - 1'b1;
                        rx_busy <= 1'b1;
                    end
                end
                
                RX_START_BIT: begin
                    if (rx_baud_counter == '0) begin
                        rx_baud_counter <= CLKS_PER_SAMPLE[$clog2(CLKS_PER_SAMPLE)-1:0] - 1'b1;
                        if (rx_sample_count == 4'd7) begin
                            // Sample at middle of start bit
                            if (rx_sync_1 == 1'b0) begin
                                // Valid start bit - continue to end of start bit period
                                // DON'T transition yet - wait until sample 15
                            end else begin
                                // False start - return to idle
                                rx_state <= RX_IDLE;
                            end
                            rx_sample_count <= rx_sample_count + 1'b1;
                        end else if (rx_sample_count == 4'd15) begin
                            // End of start bit period - now transition to data bits
                            rx_state <= RX_DATA_BITS;
                            rx_sample_count <= 4'd0;
                            rx_bit_index <= 3'd0;
                        end else begin
                            rx_sample_count <= rx_sample_count + 1'b1;
                        end
                    end else begin
                        rx_baud_counter <= rx_baud_counter - 1'b1;
                    end
                end
                
                RX_DATA_BITS: begin
                    if (rx_baud_counter == '0) begin
                        rx_baud_counter <= CLKS_PER_SAMPLE[$clog2(CLKS_PER_SAMPLE)-1:0] - 1'b1;
                        if (rx_sample_count == 4'd7) begin
                            // Sample at middle of data bit
                            rx_shift_reg <= {rx_sync_1, rx_shift_reg[7:1]};  // LSB first
                        end
                        if (rx_sample_count == 4'd15) begin
                            // End of bit period
                            rx_sample_count <= 4'd0;
                            if (rx_bit_index == 3'd7) begin
                                rx_state <= RX_STOP_BIT;
                            end else begin
                                rx_bit_index <= rx_bit_index + 1'b1;
                            end
                        end else begin
                            rx_sample_count <= rx_sample_count + 1'b1;
                        end
                    end else begin
                        rx_baud_counter <= rx_baud_counter - 1'b1;
                    end
                end
                
                RX_STOP_BIT: begin
                    if (rx_baud_counter == '0) begin
                        rx_baud_counter <= CLKS_PER_SAMPLE[$clog2(CLKS_PER_SAMPLE)-1:0] - 1'b1;
                        if (rx_sample_count == 4'd7) begin
                            // Sample stop bit at middle
                            if (rx_sync_1 == 1'b1) begin
                                // Valid stop bit - signal FIFO write
                                rx_fifo_write_int <= 1'b1;
                            end else begin
                                // Framing error
                                rx_error <= 1'b1;
                            end
                            rx_state <= RX_IDLE;
                        end else begin
                            rx_sample_count <= rx_sample_count + 1'b1;
                        end
                    end else begin
                        rx_baud_counter <= rx_baud_counter - 1'b1;
                    end
                end
                
                default: begin
                    rx_state <= RX_IDLE;
                end
            endcase
        end
    end
    
    // ============================================================
    // TX FIFO Management
    // ============================================================
    
    // TX FIFO write signal (CPU writes to TXDATA register)
    logic tx_fifo_write;
    assign tx_fifo_write = we && (reg_offset == 8'h00) && !tx_fifo_full;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            tx_wr_ptr <= '0;
            tx_rd_ptr <= '0;
            tx_fifo_count <= '0;
        end else begin
            // Handle FIFO pointer and count updates
            case ({tx_fifo_write, tx_fifo_read_int})
                2'b10: begin  // Write only
                    tx_wr_ptr <= tx_wr_ptr + 1'b1;
                    tx_fifo_count <= tx_fifo_count + 1'b1;
                end
                2'b01: begin  // Read only (TX state machine read)
                    tx_rd_ptr <= tx_rd_ptr + 1'b1;
                    tx_fifo_count <= tx_fifo_count - 1'b1;
                end
                2'b11: begin  // Simultaneous read/write: count unchanged
                    tx_wr_ptr <= tx_wr_ptr + 1'b1;
                    tx_rd_ptr <= tx_rd_ptr + 1'b1;
                end
                default: ;  // 2'b00: no change
            endcase
            
            // Write to FIFO memory
            if (tx_fifo_write) begin
                tx_fifo[tx_wr_ptr] <= wdata[7:0];
            end
        end
    end
    
    // ============================================================
    // RX FIFO Management
    // ============================================================
    
    // RX FIFO read signal (CPU reads from RXDATA register)
    // Read occurs when req is asserted and we is not (read intent implied)
    logic rx_fifo_read;
    assign rx_fifo_read = req && !we && (reg_offset == 8'h04) && !rx_fifo_empty;
    
    // RX FIFO write signal with overflow check
    // Only write if FIFO is not full to keep count synchronized
    logic rx_fifo_write;
    assign rx_fifo_write = rx_fifo_write_int && !rx_fifo_full;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_rd_ptr <= '0;
            rx_wr_ptr <= '0;
            rx_fifo_count <= '0;
        end else begin
            // Handle FIFO pointer and count updates
            case ({rx_fifo_write, rx_fifo_read})
                2'b10: begin  // Write only (RX state machine)
                    rx_wr_ptr <= rx_wr_ptr + 1'b1;
                    rx_fifo_count <= rx_fifo_count + 1'b1;
                end
                2'b01: begin  // Read only (CPU read)
                    rx_rd_ptr <= rx_rd_ptr + 1'b1;
                    rx_fifo_count <= rx_fifo_count - 1'b1;
                end
                2'b11: begin  // Simultaneous read/write: count unchanged
                    rx_wr_ptr <= rx_wr_ptr + 1'b1;
                    rx_rd_ptr <= rx_rd_ptr + 1'b1;
                end
                default: ;  // 2'b00: no change
            endcase
            
            // Write to FIFO memory (already checked for full via rx_fifo_write)
            if (rx_fifo_write) begin
                rx_fifo[rx_wr_ptr] <= rx_shift_reg;
            end
        end
    end
    
    // ============================================================
    // Register Read Logic
    // ============================================================
    
    // TX_EMPTY: FIFO empty AND transmitter idle
    logic tx_empty_status;
    assign tx_empty_status = tx_fifo_empty && (tx_state == TX_IDLE);
    
    // Read data mux
    // Read occurs when req is asserted and we is not (read intent implied)
    always_comb begin
        rdata = 32'h0;
        
        if (req && !we) begin
            case (reg_offset)
                8'h00: rdata = 32'h0;  // TXDATA is write-only
                8'h04: rdata = rx_fifo_empty ? 32'h0 : {24'h0, rx_fifo[rx_rd_ptr]};  // RXDATA
                8'h08: rdata = {24'h0,                      // STATUS
                              rx_error,                     // [7] RX_ERROR
                              rx_busy,                      // [6] RX_BUSY
                              rx_fifo_empty,                // [5] RX_EMPTY
                              rx_fifo_full,                 // [4] RX_FULL
                              1'b0,                         // [3] Reserved
                              tx_busy,                      // [2] TX_BUSY
                              tx_empty_status,              // [1] TX_EMPTY
                              tx_fifo_full};                // [0] TX_FULL
                8'h0C: rdata = 32'h0;  // CTRL (reserved)
                default: rdata = 32'h0;
            endcase
        end
    end

endmodule

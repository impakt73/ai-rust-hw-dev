// UART Core Module
// Simple 8N1 UART (8 data bits, no parity, 1 stop bit) without FIFOs
// Uses ready/valid handshake interface for TX and RX
//
// Features:
//   - Configurable baud rate via CLK_FREQ_HZ and BAUD_RATE parameters
//   - No internal FIFOs - data held in shift registers
//   - Ready/valid handshake for flow control
//   - Per-bit counter on RX for accurate timing at all supported baud rates
//   - 3-sample majority voting on RX for glitch filtering
//   - Falling-edge detection for start bit (rejects held-low line)
//   - Full stop bit timing before returning to idle
//   - 3-FF input synchronizer prevents metastability
//   - Framing error detection on RX (sticky, cleared via rx_error_clr)
//   - RX overrun detection: drops incoming data if output not yet consumed

module uart #(
    // System clock frequency in Hz (required for baud rate calculation)
    parameter int CLK_FREQ_HZ = 50_000_000,
    // Target baud rate in bits per second
    parameter int BAUD_RATE = 115200
) (
    // Clock and reset
    input  logic       clk,
    input  logic       rst_n,
    
    // TX interface (ready/valid handshake)
    input  logic [7:0] tx_data,    // Data to transmit
    input  logic       tx_valid,   // Producer has data ready
    output logic       tx_ready,   // Module can accept data
    
    // RX interface (ready/valid handshake)
    output logic [7:0] rx_data,    // Received data
    output logic       rx_valid,   // Module has data ready
    input  logic       rx_ready,   // Consumer can accept data
    output logic       rx_error,   // Error flag (framing error or overrun), sticky until cleared
    input  logic       rx_error_clr, // Clear rx_error flag when asserted
    
    // External UART pins
    output logic       tx_out,     // Serial transmit output (idle high)
    input  logic       rx_in       // Serial receive input
);

    // ============================================================
    // Parameter Validation and Baud Rate Calculation
    // ============================================================
    
    // Calculate clock divisor at compile time
    localparam int CLKS_PER_BIT = CLK_FREQ_HZ / BAUD_RATE;
    localparam logic [$clog2(CLKS_PER_BIT)-1:0] CLKS_PER_BIT_MINUS_1 =
        $clog2(CLKS_PER_BIT)'(CLKS_PER_BIT - 1);
    
    // RX sampling positions within each bit period
    // Note: CLKS_PER_BIT uses integer division (CLK_FREQ_HZ / BAUD_RATE),
    // so baud-rate quantization error is expected for non-integer divisors
    localparam int RX_VOTE_0_POS = (CLKS_PER_BIT * 6) / 16;   // First majority vote sample
    localparam int RX_VOTE_1_POS = (CLKS_PER_BIT * 7) / 16;   // Second majority vote sample
    localparam int RX_MID_POS    = (CLKS_PER_BIT * 8) / 16;   // Mid-bit: third vote sample + action
    localparam int RX_BIT_END_POS = CLKS_PER_BIT - 1;         // End of bit period
    localparam int RX_BIT_CNT_WIDTH = $clog2(CLKS_PER_BIT);
    
    // Parameter validation (simulation only)
    initial begin
        // Validate baud rate is achievable with given clock
        if (CLK_FREQ_HZ / BAUD_RATE < 16) begin
            $fatal(1, "UART: Baud rate %0d too high for clock %0d Hz (need 16x oversampling)",
                   BAUD_RATE, CLK_FREQ_HZ);
        end
    end
    
    // ============================================================
    // TX Logic
    // ============================================================
    
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
    logic tx_baud_tick;
    
    assign tx_baud_tick = (tx_baud_counter == '0);
    
    // TX ready when idle, or on the final stop-bit tick for back-to-back frames
    assign tx_ready = (tx_state == TX_IDLE) || (tx_state == TX_STOP_BIT && tx_baud_tick);
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            tx_state <= TX_IDLE;
            tx_out <= 1'b1;  // Idle high
            tx_shift_reg <= 8'h00;
            tx_bit_index <= 3'b0;
            tx_baud_counter <= '0;
        end else begin
            case (tx_state)
                TX_IDLE: begin
                    tx_out <= 1'b1;
                    if (tx_valid) begin
                        // Latch data from input
                        tx_shift_reg <= tx_data;
                        tx_state <= TX_START_BIT;
                        tx_baud_counter <= CLKS_PER_BIT_MINUS_1;
                    end
                end
                
                TX_START_BIT: begin
                    tx_out <= 1'b0;  // Start bit is low
                    if (tx_baud_tick) begin
                        tx_baud_counter <= CLKS_PER_BIT_MINUS_1;
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
                        tx_baud_counter <= CLKS_PER_BIT_MINUS_1;
                    end else begin
                        tx_baud_counter <= tx_baud_counter - 1'b1;
                    end
                end
                
                TX_STOP_BIT: begin
                    tx_out <= 1'b1;  // Stop bit is high
                    if (tx_baud_tick) begin
                        if (tx_valid) begin
                            tx_shift_reg <= tx_data;
                            tx_state <= TX_START_BIT;
                            tx_baud_counter <= CLKS_PER_BIT_MINUS_1;
                        end else begin
                            tx_state <= TX_IDLE;
                        end
                    end else begin
                        tx_baud_counter <= tx_baud_counter - 1'b1;
                    end
                end
                
                default: tx_state <= TX_IDLE;
            endcase
        end
    end
    
    // ============================================================
    // RX Logic
    // ============================================================
    
    // RX input synchronizer (3-FF for metastability)
    logic rx_sync_1;
    ff_sync #(
        .STAGES(3),
        .WIDTH(1),
        .RESET_VALUE(1'b1)
    ) rx_sync_inst (
        .clk(clk),
        .rst_n(rst_n),
        .din(rx_in),
        .dout(rx_sync_1)
    );
    
    // Previous value of synchronized RX for falling-edge detection
    // Prevents false start bit detection when line is held low
    logic rx_sync_prev;
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            rx_sync_prev <= 1'b1;  // Idle high
        else
            rx_sync_prev <= rx_sync_1;
    end
    
    // Majority voting registers for glitch filtering
    // Samples captured at vote positions within each bit period;
    // the mid-bit position uses the live rx_sync_1 value for 3-sample vote
    logic [1:0] rx_vote_reg;
    logic rx_vote_result;
    
    // Majority vote: 2-of-3 using two stored samples and current input
    assign rx_vote_result = (rx_vote_reg[0] & rx_vote_reg[1]) |
                            (rx_vote_reg[1] & rx_sync_1) |
                            (rx_vote_reg[0] & rx_sync_1);
    
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
    logic [RX_BIT_CNT_WIDTH-1:0] rx_bit_counter;  // Counts 0 to CLKS_PER_BIT-1 per bit
    
    // Combinational signal to detect when a new error is being set this cycle
    // Used to ensure new errors take precedence over rx_error_clr clearing
    logic rx_error_set;
    always_comb begin
        rx_error_set = 1'b0;
        if (rx_state == RX_STOP_BIT && rx_bit_counter == RX_MID_POS[RX_BIT_CNT_WIDTH-1:0]) begin
            if (rx_vote_result == 1'b1) begin
                // Valid stop bit - check for overrun
                if (rx_valid && !rx_ready) begin
                    rx_error_set = 1'b1;  // Overrun error
                end
            end else begin
                rx_error_set = 1'b1;  // Framing error
            end
        end
    end
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rx_state <= RX_IDLE;
            rx_data <= 8'h00;
            rx_valid <= 1'b0;
            rx_shift_reg <= 8'h00;
            rx_bit_index <= 3'b0;
            rx_bit_counter <= '0;
            rx_error <= 1'b0;
            rx_vote_reg <= 2'b0;
        end else begin
            // Handle handshake: clear rx_valid when consumer asserts rx_ready
            if (rx_valid && rx_ready) begin
                rx_valid <= 1'b0;
            end
            
            case (rx_state)
                RX_IDLE: begin
                    rx_bit_counter <= '0;
                    // True falling edge: was high, now low
                    if (rx_sync_prev && !rx_sync_1) begin
                        rx_state <= RX_START_BIT;
                        rx_bit_counter <= '0;
                    end
                end
                
                RX_START_BIT: begin
                    // Capture samples for majority voting
                    if (rx_bit_counter == RX_VOTE_0_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[0] <= rx_sync_1;
                    if (rx_bit_counter == RX_VOTE_1_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[1] <= rx_sync_1;
                    
                    if (rx_bit_counter == RX_MID_POS[RX_BIT_CNT_WIDTH-1:0]) begin
                        // Validate start bit with majority vote (expect low)
                        if (rx_vote_result) begin
                            // False start (glitch) - return to idle
                            rx_state <= RX_IDLE;
                        end
                    end
                    
                    if (rx_bit_counter == RX_BIT_END_POS[RX_BIT_CNT_WIDTH-1:0]) begin
                        // End of start bit period - transition to data bits
                        rx_state <= RX_DATA_BITS;
                        rx_bit_counter <= '0;
                        rx_bit_index <= 3'd0;
                    end else begin
                        rx_bit_counter <= rx_bit_counter + 1'b1;
                    end
                end
                
                RX_DATA_BITS: begin
                    // Capture samples for majority voting
                    if (rx_bit_counter == RX_VOTE_0_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[0] <= rx_sync_1;
                    if (rx_bit_counter == RX_VOTE_1_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[1] <= rx_sync_1;
                    
                    if (rx_bit_counter == RX_MID_POS[RX_BIT_CNT_WIDTH-1:0]) begin
                        // Sample data bit using majority vote (glitch filtered)
                        rx_shift_reg <= {rx_vote_result, rx_shift_reg[7:1]};  // LSB first
                    end
                    
                    if (rx_bit_counter == RX_BIT_END_POS[RX_BIT_CNT_WIDTH-1:0]) begin
                        // End of bit period
                        rx_bit_counter <= '0;
                        if (rx_bit_index == 3'd7) begin
                            rx_state <= RX_STOP_BIT;
                        end else begin
                            rx_bit_index <= rx_bit_index + 1'b1;
                        end
                    end else begin
                        rx_bit_counter <= rx_bit_counter + 1'b1;
                    end
                end
                
                RX_STOP_BIT: begin
                    // Capture samples for majority voting
                    if (rx_bit_counter == RX_VOTE_0_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[0] <= rx_sync_1;
                    if (rx_bit_counter == RX_VOTE_1_POS[RX_BIT_CNT_WIDTH-1:0]) rx_vote_reg[1] <= rx_sync_1;
                    
                    if (rx_bit_counter == RX_MID_POS[RX_BIT_CNT_WIDTH-1:0]) begin
                        // Validate stop bit with majority vote (expect high)
                        if (rx_vote_result == 1'b1) begin
                            // Valid stop bit - check for overrun
                            if (rx_valid && !rx_ready) begin
                                // Output register still has valid data, drop incoming data
                                // and set error flag (overrun)
                                rx_error <= 1'b1;
                            end else begin
                                // Normal case: latch data and set valid
                                rx_data <= rx_shift_reg;
                                rx_valid <= 1'b1;
                            end
                        end else begin
                            // Framing error - set sticky error flag
                            rx_error <= 1'b1;
                        end
                        rx_state <= RX_IDLE;
                        rx_bit_counter <= '0;
                    end else begin
                        rx_bit_counter <= rx_bit_counter + 1'b1;
                    end
                end
                
                default: begin
                    rx_state <= RX_IDLE;
                end
            endcase
            
            // Handle rx_error_clr: clear sticky error flag when asserted
            // Only clear if no new error is being set in this cycle
            // This ensures new errors take precedence over clearing
            if (rx_error_clr && !rx_error_set) begin
                rx_error <= 1'b0;
            end
        end
    end

endmodule

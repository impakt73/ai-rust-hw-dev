// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Pull-only model: single transaction at a time
//   - Little-endian data format for x86/ARM compatibility
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length, Little-Endian):
//   Read Request:   [header][addr0][addr1][addr2][addr3]              (5 bytes)
//   Write Request:  [header][addr0][addr1][addr2][addr3][data...]     (6-9 bytes)
//   Write Response: [ack]                                             (1 byte, 0x00)
//   Read Response:  [data...]                                         (1-4 bytes, no header)

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus Slave Interface (from System Bus)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,
    
    // Host TX Interface (to External Host)
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,
    
    // Host RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [4:0] {
        STATE_IDLE        = 5'd0,
        STATE_CAPTURE     = 5'd1,
        
        // TX States (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 5'd2,   // Header byte
        STATE_TX_ADDR_0   = 5'd3,   // Address[7:0] (little-endian: LSB first)
        STATE_TX_ADDR_1   = 5'd4,   // Address[15:8]
        STATE_TX_ADDR_2   = 5'd5,   // Address[23:16]
        STATE_TX_ADDR_3   = 5'd6,   // Address[31:24]
        STATE_TX_WDATA_0  = 5'd7,   // WData[7:0] (little-endian: LSB first)
        STATE_TX_WDATA_1  = 5'd8,   // WData[15:8] (half/word writes)
        STATE_TX_WDATA_2  = 5'd9,   // WData[23:16] (word writes only)
        STATE_TX_WDATA_3  = 5'd10,  // WData[31:24] (word writes only)
        
        // RX States (variable length: 1-4 bytes)
        STATE_RX_ACK      = 5'd11,  // Write response: single ack byte
        STATE_RX_RDATA_0  = 5'd12,  // RData[7:0] (little-endian: LSB first)
        STATE_RX_RDATA_1  = 5'd13,  // RData[15:8] (halfword/word reads)
        STATE_RX_RDATA_2  = 5'd14,  // RData[23:16] (word reads only)
        STATE_RX_RDATA_3  = 5'd15,  // RData[31:24] (word reads only)
        
        // Complete state - asserts ready for one cycle
        STATE_COMPLETE    = 5'd16
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Request Registers
    // ============================================================
    logic [31:0] cap_addr;      // Captured address
    logic [31:0] cap_wdata;     // Captured write data
    logic        cap_we;        // Captured write enable
    logic [1:0]  cap_size;      // Captured access size
    
    // ============================================================
    // Response Data Registers
    // ============================================================
    logic [31:0] resp_rdata;    // Received read data
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
    logic        in_tx_phase;   // Indicates TX phase active
    logic        in_rx_phase;   // Indicates RX phase active

    // ============================================================
    // State Register
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;
        end else begin
            state <= next_state;
        end
    end
    
    // ============================================================
    // Next State Logic (Variable Length Packets, Little-Endian)
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                if (req) begin
                    next_state = STATE_CAPTURE;
                end
            end
            
            STATE_CAPTURE: begin
                next_state = STATE_TX_HEADER;
            end
            
            // --------------------------------------------------------
            // TX Phase: Header + Address (always) + Data (writes only)
            // Address and data sent in little-endian order (LSB first)
            // --------------------------------------------------------
            STATE_TX_HEADER: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_0;
            end
            
            STATE_TX_ADDR_0: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_1;
            end
            
            STATE_TX_ADDR_1: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_2;
            end
            
            STATE_TX_ADDR_2: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_3;
            end
            
            STATE_TX_ADDR_3: begin
                if (tx_valid && tx_ready) begin
                    if (cap_we) begin
                        // Write: send data bytes in little-endian order
                        next_state = STATE_TX_WDATA_0;  // Always start with LSB
                    end else begin
                        // Read: no data, go to RX phase
                        next_state = STATE_RX_RDATA_0;  // Read response starts with LSB
                    end
                end
            end
            
            // TX Write Data States (little-endian: LSB first)
            STATE_TX_WDATA_0: begin  // All writes start here (LSB)
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b00:   next_state = STATE_RX_ACK;       // Byte: done after 1 byte
                        default: next_state = STATE_TX_WDATA_1;  // Half/Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_1: begin  // Half and Word
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b01:   next_state = STATE_RX_ACK;       // Half: done after 2 bytes
                        default: next_state = STATE_TX_WDATA_2;  // Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_2: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_3;
            end
            
            STATE_TX_WDATA_3: begin  // Word only (MSB, last byte)
                if (tx_valid && tx_ready) next_state = STATE_RX_ACK;
            end
            
            // --------------------------------------------------------
            // RX Phase: Ack (writes) or Data (reads)
            // Data received in little-endian order (LSB first)
            // --------------------------------------------------------
            STATE_RX_ACK: begin  // Write response: single ack byte
                if (rx_valid && rx_ready) begin
                    next_state = STATE_COMPLETE;
                end
            end
            
            // RX Read Data States (little-endian: LSB first)
            STATE_RX_RDATA_0: begin  // All reads start here (LSB)
                if (rx_valid && rx_ready) begin
                    case (cap_size)
                        2'b00:   next_state = STATE_COMPLETE;     // Byte: done after 1 byte
                        default: next_state = STATE_RX_RDATA_1;  // Half/Word: continue
                    endcase
                end
            end
            
            STATE_RX_RDATA_1: begin  // Half and Word
                if (rx_valid && rx_ready) begin
                    case (cap_size)
                        2'b01:   next_state = STATE_COMPLETE;     // Half: done after 2 bytes
                        default: next_state = STATE_RX_RDATA_2;  // Word: continue
                    endcase
                end
            end
            
            STATE_RX_RDATA_2: begin  // Word only
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_3;
            end
            
            STATE_RX_RDATA_3: begin  // Word only (MSB, last byte)
                if (rx_valid && rx_ready) next_state = STATE_COMPLETE;
            end
            
            // --------------------------------------------------------
            // Complete state - asserts ready for one cycle then returns to IDLE
            // --------------------------------------------------------
            STATE_COMPLETE: begin
                next_state = STATE_IDLE;
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end

    // ============================================================
    // Capture Request on CAPTURE state
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cap_addr  <= 32'h0;
            cap_wdata <= 32'h0;
            cap_we    <= 1'b0;
            cap_size  <= 2'b00;
        end else if (state == STATE_IDLE && req) begin
            // Capture on rising edge of req while idle
            cap_addr  <= addr;
            cap_wdata <= wdata;
            cap_we    <= we;
            cap_size  <= size;
        end
    end

    // ============================================================
    // TX Phase Detection
    // ============================================================
    assign in_tx_phase = (state >= STATE_TX_HEADER) && (state <= STATE_TX_WDATA_3);
    
    // ============================================================
    // TX Data Multiplexer (Little-Endian: LSB first)
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            default:          tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    assign tx_valid = in_tx_phase;

    // ============================================================
    // RX Phase Detection
    // ============================================================
    assign in_rx_phase = (state >= STATE_RX_ACK) && (state <= STATE_RX_RDATA_3);
    
    // ============================================================
    // RX Ready Signal
    // ============================================================
    assign rx_ready = in_rx_phase;
    
    // ============================================================
    // RX Data Capture (Little-Endian: LSB first)
    // Clear resp_rdata when entering RX phase to avoid stale data
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_rdata <= 32'h0;
        end else if (state == STATE_TX_ADDR_3 && tx_valid && tx_ready && !cap_we) begin
            // Clear rdata when transitioning to read response phase
            resp_rdata <= 32'h0;
        end else if (rx_valid && rx_ready) begin
            case (state)
                STATE_RX_RDATA_0: resp_rdata[7:0]   <= rx_data;
                STATE_RX_RDATA_1: resp_rdata[15:8]  <= rx_data;
                STATE_RX_RDATA_2: resp_rdata[23:16] <= rx_data;
                STATE_RX_RDATA_3: resp_rdata[31:24] <= rx_data;
                default: ;
            endcase
        end
    end

    // ============================================================
    // Bus Ready Signal - asserted in COMPLETE state only
    // ============================================================
    assign ready = (state == STATE_COMPLETE);
    
    // ============================================================
    // Bus Read Data
    // ============================================================
    assign rdata = resp_rdata;

endmodule

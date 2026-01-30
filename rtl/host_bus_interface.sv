// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Pull-only model: single transaction at a time
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length):
//   Read Request:   [header][addr3][addr2][addr1][addr0]              (5 bytes)
//   Write Request:  [header][addr3][addr2][addr1][addr0][data...]     (6-9 bytes)
//   Write Response: [header]                                          (1 byte)
//   Read Response:  [header][data...]                                 (2-5 bytes)

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
    typedef enum logic [3:0] {
        STATE_IDLE        = 4'd0,
        STATE_CAPTURE     = 4'd1,
        
        // TX States (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 4'd2,   // Header byte
        STATE_TX_ADDR_3   = 4'd3,   // Address[31:24]
        STATE_TX_ADDR_2   = 4'd4,   // Address[23:16]
        STATE_TX_ADDR_1   = 4'd5,   // Address[15:8]
        STATE_TX_ADDR_0   = 4'd6,   // Address[7:0]
        STATE_TX_WDATA_3  = 4'd7,   // WData[31:24] (word writes only)
        STATE_TX_WDATA_2  = 4'd8,   // WData[23:16] (half/word writes)
        STATE_TX_WDATA_1  = 4'd9,   // WData[15:8] (word writes only)
        STATE_TX_WDATA_0  = 4'd10,  // WData[7:0] (all writes - byte aligned)
        
        // RX States (variable length: 1-5 bytes)
        STATE_RX_HEADER   = 4'd11,  // Response header
        STATE_RX_RDATA_3  = 4'd12,  // RData[31:24] (word reads only)
        STATE_RX_RDATA_2  = 4'd13,  // RData[23:16] (word reads only)
        STATE_RX_RDATA_1  = 4'd14,  // RData[15:8] (halfword/word reads)
        STATE_RX_RDATA_0  = 4'd15   // RData[7:0] (all reads)
    } state_t;
    
    state_t state, next_state;
    logic   transaction_complete;  // Indicates COMPLETE state
    
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
            transaction_complete <= 1'b0;
        end else begin
            state <= next_state;
            // Set complete flag when transitioning to complete state
            transaction_complete <= (next_state == STATE_IDLE) && 
                                   (state != STATE_IDLE) && 
                                   (state != STATE_CAPTURE);
        end
    end
    
    // ============================================================
    // Next State Logic (Variable Length Packets)
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
            // --------------------------------------------------------
            STATE_TX_HEADER: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_3;
            end
            
            STATE_TX_ADDR_3: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_2;
            end
            
            STATE_TX_ADDR_2: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_1;
            end
            
            STATE_TX_ADDR_1: begin
                if (tx_valid && tx_ready) next_state = STATE_TX_ADDR_0;
            end
            
            STATE_TX_ADDR_0: begin
                if (tx_valid && tx_ready) begin
                    if (cap_we) begin
                        // Write: send data bytes based on size
                        case (cap_size)
                            2'b10:   next_state = STATE_TX_WDATA_3;  // Word: 4 bytes
                            2'b01:   next_state = STATE_TX_WDATA_1;  // Half: 2 bytes ([15:8], [7:0])
                            default: next_state = STATE_TX_WDATA_0;  // Byte: 1 byte
                        endcase
                    end else begin
                        // Read: no data, go to RX phase
                        next_state = STATE_RX_HEADER;
                    end
                end
            end
            
            // TX Write Data States (conditional based on size)
            STATE_TX_WDATA_3: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_2;
            end
            
            STATE_TX_WDATA_2: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_1;
            end
            
            STATE_TX_WDATA_1: begin  // Half and Word
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_0;
            end
            
            STATE_TX_WDATA_0: begin  // All writes end here
                if (tx_valid && tx_ready) next_state = STATE_RX_HEADER;
            end
            
            // --------------------------------------------------------
            // RX Phase: Header (always) + Data (reads only)
            // --------------------------------------------------------
            STATE_RX_HEADER: begin
                if (rx_valid && rx_ready) begin
                    if (cap_we) begin
                        // Write response: header only, transaction complete
                        next_state = STATE_IDLE;
                    end else begin
                        // Read response: receive data based on size
                        case (cap_size)
                            2'b10:   next_state = STATE_RX_RDATA_3;  // Word: 4 bytes
                            2'b01:   next_state = STATE_RX_RDATA_1;  // Half: 2 bytes ([15:8], [7:0])
                            default: next_state = STATE_RX_RDATA_0;  // Byte: 1 byte
                        endcase
                    end
                end
            end
            
            // RX Read Data States (conditional based on size)
            STATE_RX_RDATA_3: begin  // Word only
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_2;
            end
            
            STATE_RX_RDATA_2: begin  // Word only
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_1;
            end
            
            STATE_RX_RDATA_1: begin  // Half and Word
                if (rx_valid && rx_ready) next_state = STATE_RX_RDATA_0;
            end
            
            STATE_RX_RDATA_0: begin  // All reads end here
                if (rx_valid && rx_ready) next_state = STATE_IDLE;
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
    assign in_tx_phase = (state >= STATE_TX_HEADER) && (state <= STATE_TX_WDATA_0);
    
    // ============================================================
    // TX Data Multiplexer
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
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
    assign in_rx_phase = (state >= STATE_RX_HEADER);
    
    // ============================================================
    // RX Ready Signal
    // ============================================================
    assign rx_ready = in_rx_phase;
    
    // ============================================================
    // RX Data Capture
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_rdata <= 32'h0;
        end else if (rx_valid && rx_ready) begin
            case (state)
                STATE_RX_RDATA_3: resp_rdata[31:24] <= rx_data;
                STATE_RX_RDATA_2: resp_rdata[23:16] <= rx_data;
                STATE_RX_RDATA_1: resp_rdata[15:8]  <= rx_data;
                STATE_RX_RDATA_0: resp_rdata[7:0]   <= rx_data;
                default: ;
            endcase
        end
    end

    // ============================================================
    // Bus Ready Signal
    // ============================================================
    assign ready = transaction_complete;
    
    // ============================================================
    // Bus Read Data
    // ============================================================
    assign rdata = resp_rdata;

endmodule

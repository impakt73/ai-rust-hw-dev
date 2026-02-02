// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Bi-directional communication: CPU→Host and Host→RTL
//   - Little-endian data format for x86/ARM compatibility
//   - No checksums (relies on transport layer if needed)
//
// Extended Protocol (Variable Length, Little-Endian):
//   Extended header format: {packet_type[3:0], size[1:0], 1'b0, we}
//   Packet types:
//     0000 = CPU-initiated request (FPGA → Host TX)
//     0001 = Host response to CPU request (Host → FPGA RX)
//     0010 = Host-initiated request (Host → FPGA RX)
//     0011 = FPGA response to Host request (FPGA → Host TX)
//     1111 = Error response (FPGA → Host TX)
//
//   CPU→Host Request:   [ext_header][addr0][addr1][addr2][addr3][data...]  (5-9 bytes)
//   Host→CPU Response:  [ext_header][data...]                               (1-5 bytes)
//   Host→FPGA Request:  [ext_header][addr0][addr1][addr2][addr3][data...]  (5-9 bytes)
//   FPGA→Host Response: [ext_header][data...]                               (1-5 bytes)
//   Error Response:     [ext_header][error_code]                            (2 bytes)

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus Slave Interface (from System Bus - CPU requests)
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
    output logic        rx_ready,
    
    // Host Bus Master Interface (to Bus Arbiter - for Host-initiated requests)
    output logic [31:0] host_bus_addr,
    output logic [31:0] host_bus_wdata,
    input  logic [31:0] host_bus_rdata,
    output logic        host_bus_we,
    output logic [1:0]  host_bus_size,
    output logic        host_bus_req,
    input  logic        host_bus_ready
);

    // ============================================================
    // Address Range Definitions for Host Request Validation
    // ============================================================
    localparam RTL_PERIPH_BASE  = 32'h5000_0000;
    localparam RTL_PERIPH_LIMIT = 32'h6000_0000;
    
    // ============================================================
    // State Machine (Extended for bi-directional communication)
    // ============================================================
    typedef enum logic [5:0] {
        STATE_IDLE        = 6'd0,
        STATE_CAPTURE     = 6'd1,
        
        // TX States for CPU→Host requests (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 6'd2,   // Header byte (packet type 0000)
        STATE_TX_ADDR_0   = 6'd3,   // Address[7:0] (little-endian: LSB first)
        STATE_TX_ADDR_1   = 6'd4,   // Address[15:8]
        STATE_TX_ADDR_2   = 6'd5,   // Address[23:16]
        STATE_TX_ADDR_3   = 6'd6,   // Address[31:24]
        STATE_TX_WDATA_0  = 6'd7,   // WData[7:0] (little-endian: LSB first)
        STATE_TX_WDATA_1  = 6'd8,   // WData[15:8] (half/word writes)
        STATE_TX_WDATA_2  = 6'd9,   // WData[23:16] (word writes only)
        STATE_TX_WDATA_3  = 6'd10,  // WData[31:24] (word writes only)
        
        // RX States for Host→CPU responses (variable length: 1-5 bytes)
        STATE_RX_RESP_HDR = 6'd11,  // Response header byte (packet type 0001)
        STATE_RX_ACK      = 6'd12,  // Write response: ack received in header
        STATE_RX_RDATA_0  = 6'd13,  // RData[7:0] (little-endian: LSB first)
        STATE_RX_RDATA_1  = 6'd14,  // RData[15:8] (halfword/word reads)
        STATE_RX_RDATA_2  = 6'd15,  // RData[23:16] (word reads only)
        STATE_RX_RDATA_3  = 6'd16,  // RData[31:24] (word reads only)
        
        // Complete state - asserts ready for one cycle
        STATE_COMPLETE    = 6'd17,
        
        // NEW: States for Host→FPGA requests
        STATE_HOST_RX_HEADER    = 6'd20,  // Receive host request header (packet type 0010)
        STATE_HOST_RX_ADDR_0    = 6'd21,  // Receive address bytes
        STATE_HOST_RX_ADDR_1    = 6'd22,
        STATE_HOST_RX_ADDR_2    = 6'd23,
        STATE_HOST_RX_ADDR_3    = 6'd24,
        STATE_HOST_RX_WDATA_0   = 6'd25,  // Receive write data bytes
        STATE_HOST_RX_WDATA_1   = 6'd26,
        STATE_HOST_RX_WDATA_2   = 6'd27,
        STATE_HOST_RX_WDATA_3   = 6'd28,
        STATE_HOST_BUS_REQ      = 6'd29,  // Issue request to bus arbiter
        STATE_HOST_BUS_WAIT     = 6'd30,  // Wait for bus response
        STATE_HOST_TX_HEADER    = 6'd31,  // Send response header (packet type 0011)
        STATE_HOST_TX_RDATA_0   = 6'd32,  // Send read data to host
        STATE_HOST_TX_RDATA_1   = 6'd33,
        STATE_HOST_TX_RDATA_2   = 6'd34,
        STATE_HOST_TX_RDATA_3   = 6'd35,
        STATE_HOST_COMPLETE     = 6'd36,
        STATE_HOST_ERROR        = 6'd37,  // Error handling
        STATE_HOST_ERROR_CODE   = 6'd38   // Send error code byte
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Request Registers (CPU→Host)
    // ============================================================
    logic [31:0] cap_addr;      // Captured address
    logic [31:0] cap_wdata;     // Captured write data
    logic        cap_we;        // Captured write enable
    logic [1:0]  cap_size;      // Captured access size
    
    // ============================================================
    // Response Data Registers (Host→CPU)
    // ============================================================
    logic [31:0] resp_rdata;    // Received read data
    
    // ============================================================
    // Host-Initiated Transaction Registers (Host→FPGA)
    // ============================================================
    logic [31:0] host_cap_addr;     // Captured address from host request
    logic [31:0] host_cap_wdata;    // Captured write data from host request
    logic        host_cap_we;       // Captured write enable from host request
    logic [1:0]  host_cap_size;     // Captured access size from host request
    logic [31:0] host_resp_rdata;   // Read data to send back to host
    logic        host_addr_valid;   // Address validation result
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
    logic        in_tx_phase;   // Indicates TX phase active (CPU→Host)
    logic        in_rx_phase;   // Indicates RX phase active (Host→CPU response)
    logic        in_host_tx_phase; // Indicates TX phase for Host response (FPGA→Host)
    logic        in_host_rx_phase; // Indicates RX phase for Host request (Host→FPGA)
    
    // ============================================================
    // Packet Type Detection Functions
    // ============================================================
    // Check for host-initiated request header (packet type 0010)
    function logic is_host_request_header(input logic [7:0] data);
        is_host_request_header = (data[7:4] == 4'b0010);
    endfunction
    
    // Check for host response header (packet type 0001)
    function logic is_host_response_header(input logic [7:0] data);
        is_host_response_header = (data[7:4] == 4'b0001);
    endfunction
    
    // ============================================================
    // Address Validation for Host Requests
    // ============================================================
    // Host can ONLY access RTL peripheral space (0x5000_0000 - 0x5FFF_FFFF)
    assign host_addr_valid = (host_cap_addr >= RTL_PERIPH_BASE) && 
                             (host_cap_addr < RTL_PERIPH_LIMIT);

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
                // Priority 1: Check for incoming Host-initiated request (packet type 0010)
                if (rx_valid && is_host_request_header(rx_data)) begin
                    next_state = STATE_HOST_RX_HEADER;
                end
                // Priority 2: Check for CPU request
                else if (req) begin
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
                        // Read: no data, go to RX phase - wait for response header
                        next_state = STATE_RX_RESP_HDR;
                    end
                end
            end
            
            // TX Write Data States (little-endian: LSB first)
            STATE_TX_WDATA_0: begin  // All writes start here (LSB)
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b00:   next_state = STATE_RX_RESP_HDR;  // Byte: done after 1 byte
                        default: next_state = STATE_TX_WDATA_1;  // Half/Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_1: begin  // Half and Word
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b01:   next_state = STATE_RX_RESP_HDR;  // Half: done after 2 bytes
                        default: next_state = STATE_TX_WDATA_2;  // Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_2: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_3;
            end
            
            STATE_TX_WDATA_3: begin  // Word only (MSB, last byte)
                if (tx_valid && tx_ready) next_state = STATE_RX_RESP_HDR;
            end
            
            // --------------------------------------------------------
            // RX Phase: Response header then Ack (writes) or Data (reads)
            // Data received in little-endian order (LSB first)
            // --------------------------------------------------------
            STATE_RX_RESP_HDR: begin  // Wait for response header (packet type 0001)
                if (rx_valid && rx_ready) begin
                    // Header received - check if it's a write ack or read data
                    if (cap_we) begin
                        next_state = STATE_COMPLETE;  // Write: header is the ack
                    end else begin
                        // Read: continue to receive data bytes
                        next_state = STATE_RX_RDATA_0;
                    end
                end
            end
            
            STATE_RX_ACK: begin  // Write response: single ack byte (legacy compatibility)
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
            
            // ============================================================
            // Host-Initiated Request States (Host→FPGA)
            // ============================================================
            STATE_HOST_RX_HEADER: begin
                // Header byte consumed - parse and move to address reception
                if (rx_valid && rx_ready) begin
                    next_state = STATE_HOST_RX_ADDR_0;
                end
            end
            
            STATE_HOST_RX_ADDR_0: begin
                if (rx_valid && rx_ready) next_state = STATE_HOST_RX_ADDR_1;
            end
            
            STATE_HOST_RX_ADDR_1: begin
                if (rx_valid && rx_ready) next_state = STATE_HOST_RX_ADDR_2;
            end
            
            STATE_HOST_RX_ADDR_2: begin
                if (rx_valid && rx_ready) next_state = STATE_HOST_RX_ADDR_3;
            end
            
            STATE_HOST_RX_ADDR_3: begin
                if (rx_valid && rx_ready) begin
                    if (host_cap_we) begin
                        // Write: receive write data bytes
                        next_state = STATE_HOST_RX_WDATA_0;
                    end else begin
                        // Read: issue bus request
                        next_state = STATE_HOST_BUS_REQ;
                    end
                end
            end
            
            // Host RX Write Data States
            STATE_HOST_RX_WDATA_0: begin
                if (rx_valid && rx_ready) begin
                    case (host_cap_size)
                        2'b00:   next_state = STATE_HOST_BUS_REQ;     // Byte: done
                        default: next_state = STATE_HOST_RX_WDATA_1; // Half/Word: continue
                    endcase
                end
            end
            
            STATE_HOST_RX_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    case (host_cap_size)
                        2'b01:   next_state = STATE_HOST_BUS_REQ;     // Half: done
                        default: next_state = STATE_HOST_RX_WDATA_2; // Word: continue
                    endcase
                end
            end
            
            STATE_HOST_RX_WDATA_2: begin
                if (rx_valid && rx_ready) next_state = STATE_HOST_RX_WDATA_3;
            end
            
            STATE_HOST_RX_WDATA_3: begin
                if (rx_valid && rx_ready) next_state = STATE_HOST_BUS_REQ;
            end
            
            // Bus Request State - validate address and issue request
            STATE_HOST_BUS_REQ: begin
                if (host_addr_valid) begin
                    next_state = STATE_HOST_BUS_WAIT;
                end else begin
                    // Invalid address - send error response
                    next_state = STATE_HOST_ERROR;
                end
            end
            
            STATE_HOST_BUS_WAIT: begin
                if (host_bus_ready) begin
                    // Bus transaction complete - send response
                    next_state = STATE_HOST_TX_HEADER;
                end
            end
            
            // Host TX Response States (FPGA→Host)
            STATE_HOST_TX_HEADER: begin
                if (tx_valid && tx_ready) begin
                    if (host_cap_we) begin
                        // Write: header is the ack, we're done
                        next_state = STATE_HOST_COMPLETE;
                    end else begin
                        // Read: send data bytes
                        next_state = STATE_HOST_TX_RDATA_0;
                    end
                end
            end
            
            STATE_HOST_TX_RDATA_0: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b00:   next_state = STATE_HOST_COMPLETE;    // Byte: done
                        default: next_state = STATE_HOST_TX_RDATA_1; // Half/Word: continue
                    endcase
                end
            end
            
            STATE_HOST_TX_RDATA_1: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b01:   next_state = STATE_HOST_COMPLETE;    // Half: done
                        default: next_state = STATE_HOST_TX_RDATA_2; // Word: continue
                    endcase
                end
            end
            
            STATE_HOST_TX_RDATA_2: begin
                if (tx_valid && tx_ready) next_state = STATE_HOST_TX_RDATA_3;
            end
            
            STATE_HOST_TX_RDATA_3: begin
                if (tx_valid && tx_ready) next_state = STATE_HOST_COMPLETE;
            end
            
            STATE_HOST_COMPLETE: begin
                next_state = STATE_IDLE;
            end
            
            // Error Handling States
            STATE_HOST_ERROR: begin
                // Send error response header (packet type 1111)
                if (tx_valid && tx_ready) begin
                    next_state = STATE_HOST_ERROR_CODE;
                end
            end
            
            STATE_HOST_ERROR_CODE: begin
                // Send error code byte
                if (tx_valid && tx_ready) begin
                    next_state = STATE_IDLE;
                end
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end

    // ============================================================
    // Capture Request on CAPTURE state (CPU→Host)
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
    // Host Request Capture (Host→FPGA)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            host_cap_addr  <= 32'h0;
            host_cap_wdata <= 32'h0;
            host_cap_we    <= 1'b0;
            host_cap_size  <= 2'b00;
        end else if (state == STATE_IDLE && rx_valid && is_host_request_header(rx_data)) begin
            // Capture header info when receiving host request header
            host_cap_we   <= (rx_data & 8'h01) != 8'h0;
            host_cap_size <= rx_data[3:2];
            host_cap_addr <= 32'h0;
            host_cap_wdata <= 32'h0;
        end else if (rx_valid && rx_ready) begin
            // Capture address bytes (little-endian)
            case (state)
                STATE_HOST_RX_ADDR_0: host_cap_addr[7:0]   <= rx_data;
                STATE_HOST_RX_ADDR_1: host_cap_addr[15:8]  <= rx_data;
                STATE_HOST_RX_ADDR_2: host_cap_addr[23:16] <= rx_data;
                STATE_HOST_RX_ADDR_3: host_cap_addr[31:24] <= rx_data;
                STATE_HOST_RX_WDATA_0: host_cap_wdata[7:0]   <= rx_data;
                STATE_HOST_RX_WDATA_1: host_cap_wdata[15:8]  <= rx_data;
                STATE_HOST_RX_WDATA_2: host_cap_wdata[23:16] <= rx_data;
                STATE_HOST_RX_WDATA_3: host_cap_wdata[31:24] <= rx_data;
                default: ;
            endcase
        end
    end
    
    // ============================================================
    // Host Response Data Capture
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            host_resp_rdata <= 32'h0;
        end else if (state == STATE_HOST_BUS_WAIT && host_bus_ready) begin
            // Capture read data from bus when transaction completes
            host_resp_rdata <= host_bus_rdata;
        end
    end

    // ============================================================
    // TX Phase Detection (includes CPU→Host and FPGA→Host responses)
    // ============================================================
    assign in_tx_phase = (state >= STATE_TX_HEADER && state <= STATE_TX_WDATA_3);
    assign in_host_tx_phase = (state >= STATE_HOST_TX_HEADER && state <= STATE_HOST_TX_RDATA_3) ||
                              (state == STATE_HOST_ERROR) || (state == STATE_HOST_ERROR_CODE);
    
    // ============================================================
    // TX Data Multiplexer (Little-Endian: LSB first)
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            // CPU→Host request TX
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};  // Packet type 0000
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            // FPGA→Host response TX
            STATE_HOST_TX_HEADER:  tx_byte = {4'b0011, host_cap_size, 1'b0, host_cap_we};  // Packet type 0011
            STATE_HOST_TX_RDATA_0: tx_byte = host_resp_rdata[7:0];
            STATE_HOST_TX_RDATA_1: tx_byte = host_resp_rdata[15:8];
            STATE_HOST_TX_RDATA_2: tx_byte = host_resp_rdata[23:16];
            STATE_HOST_TX_RDATA_3: tx_byte = host_resp_rdata[31:24];
            // Error response TX
            STATE_HOST_ERROR:      tx_byte = {4'b1111, host_cap_size, 1'b0, host_cap_we};  // Packet type 1111
            STATE_HOST_ERROR_CODE: tx_byte = 8'hFF;  // Error code: invalid address
            default:               tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal (active for both CPU→Host TX and FPGA→Host response TX)
    // ============================================================
    assign tx_valid = in_tx_phase || in_host_tx_phase;

    // ============================================================
    // RX Phase Detection (includes Host→CPU responses and Host→FPGA requests)
    // ============================================================
    assign in_rx_phase = (state >= STATE_RX_RESP_HDR && state <= STATE_RX_RDATA_3);
    assign in_host_rx_phase = (state >= STATE_HOST_RX_HEADER && state <= STATE_HOST_RX_WDATA_3);
    
    // In IDLE state, we need to be ready to accept a host request header
    // This creates a combinational path: rx_ready = 1 in IDLE allows rx_valid && rx_ready
    // to detect host request headers. The state machine then transitions to STATE_HOST_RX_HEADER.
    logic idle_rx_ready;
    assign idle_rx_ready = (state == STATE_IDLE);
    
    // ============================================================
    // RX Ready Signal (active for both Host→CPU responses and Host→FPGA requests)
    // Also active in IDLE to detect incoming host request headers
    // ============================================================
    assign rx_ready = in_rx_phase || in_host_rx_phase || idle_rx_ready;
    
    // ============================================================
    // RX Data Capture for CPU←Host responses (Little-Endian: LSB first)
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
    // Bus Ready Signal - asserted in COMPLETE state only (for CPU requests)
    // ============================================================
    assign ready = (state == STATE_COMPLETE);
    
    // ============================================================
    // Bus Read Data (for CPU requests)
    // ============================================================
    assign rdata = resp_rdata;
    
    // ============================================================
    // Host Bus Master Interface Outputs (for Host-initiated requests)
    // ============================================================
    assign host_bus_addr  = host_cap_addr;
    assign host_bus_wdata = host_cap_wdata;
    assign host_bus_we    = host_cap_we;
    assign host_bus_size  = host_cap_size;
    assign host_bus_req   = (state == STATE_HOST_BUS_REQ || state == STATE_HOST_BUS_WAIT) && host_addr_valid;

endmodule

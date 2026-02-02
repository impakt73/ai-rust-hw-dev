// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Bi-directional: supports CPU→Host and Host→FPGA requests
//   - Little-endian data format for x86/ARM compatibility
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length, Little-Endian):
//   Extended header format: {packet_type[3:0], size[1:0], 1'b0, we}
//   Packet types:
//     0000 = CPU-initiated request (FPGA → Host TX)
//     0001 = Host response to CPU request (Host → FPGA RX)
//     0010 = Host-initiated request (Host → FPGA RX)
//     0011 = FPGA response to Host request (FPGA → Host TX)
//     1111 = Error response (FPGA → Host TX)
//
//   CPU-initiated request (type 0000):
//     Read Request:   [header][addr0][addr1][addr2][addr3]              (5 bytes)
//     Write Request:  [header][addr0][addr1][addr2][addr3][data...]     (6-9 bytes)
//   Host response to CPU (type 0001):
//     Write Response: [header]                                          (1 byte)
//     Read Response:  [header][data...]                                 (2-5 bytes)
//
//   Host-initiated request (type 0010):
//     Read Request:   [header][addr0][addr1][addr2][addr3]              (5 bytes)
//     Write Request:  [header][addr0][addr1][addr2][addr3][data...]     (6-9 bytes)
//   FPGA response to Host (type 0011):
//     Write Response: [header]                                          (1 byte)
//     Read Response:  [header][data...]                                 (2-5 bytes)
//   Error response (type 1111):
//     Error:          [header][error_code]                              (2 bytes)

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus Slave Interface (from System Bus - CPU-initiated requests)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,
    
    // Bus Master Interface (to Arbiter - Host-initiated requests)
    output logic [31:0] host_bus_addr,
    output logic [31:0] host_bus_wdata,
    input  logic [31:0] host_bus_rdata,
    output logic        host_bus_we,
    output logic [1:0]  host_bus_size,
    output logic        host_bus_req,
    input  logic        host_bus_ready,
    
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
    typedef enum logic [5:0] {
        STATE_IDLE        = 6'd0,
        STATE_CAPTURE     = 6'd1,
        
        // TX States for CPU-initiated requests (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 6'd2,   // Header byte
        STATE_TX_ADDR_0   = 6'd3,   // Address[7:0] (little-endian: LSB first)
        STATE_TX_ADDR_1   = 6'd4,   // Address[15:8]
        STATE_TX_ADDR_2   = 6'd5,   // Address[23:16]
        STATE_TX_ADDR_3   = 6'd6,   // Address[31:24]
        STATE_TX_WDATA_0  = 6'd7,   // WData[7:0] (little-endian: LSB first)
        STATE_TX_WDATA_1  = 6'd8,   // WData[15:8] (half/word writes)
        STATE_TX_WDATA_2  = 6'd9,   // WData[23:16] (word writes only)
        STATE_TX_WDATA_3  = 6'd10,  // WData[31:24] (word writes only)
        
        // RX States for CPU-initiated requests (variable length: 1-5 bytes)
        STATE_RX_HEADER   = 6'd11,  // Response header byte (new)
        STATE_RX_RDATA_0  = 6'd12,  // RData[7:0] (little-endian: LSB first)
        STATE_RX_RDATA_1  = 6'd13,  // RData[15:8] (halfword/word reads)
        STATE_RX_RDATA_2  = 6'd14,  // RData[23:16] (word reads only)
        STATE_RX_RDATA_3  = 6'd15,  // RData[31:24] (word reads only)
        
        // Complete state for CPU-initiated requests - asserts ready for one cycle
        STATE_COMPLETE    = 6'd16,
        
        // Host-initiated request states (Host → FPGA)
        STATE_HOST_RX_HEADER    = 6'd20,  // Receive host request header
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
        STATE_HOST_TX_HEADER    = 6'd31,  // Send response header to host
        STATE_HOST_TX_RDATA_0   = 6'd32,  // Send read data to host
        STATE_HOST_TX_RDATA_1   = 6'd33,
        STATE_HOST_TX_RDATA_2   = 6'd34,
        STATE_HOST_TX_RDATA_3   = 6'd35,
        STATE_HOST_COMPLETE     = 6'd36,
        STATE_HOST_ERROR        = 6'd37,  // Error handling - send error header
        STATE_HOST_ERROR_CODE   = 6'd38   // Error handling - send error code
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Request Registers (CPU-initiated)
    // ============================================================
    logic [31:0] cap_addr;      // Captured address
    logic [31:0] cap_wdata;     // Captured write data
    logic        cap_we;        // Captured write enable
    logic [1:0]  cap_size;      // Captured access size
    
    // ============================================================
    // Host-initiated Request Registers
    // ============================================================
    logic [31:0] host_cap_addr;   // Host request address
    logic [31:0] host_cap_wdata;  // Host request write data
    logic        host_cap_we;     // Host request write enable
    logic [1:0]  host_cap_size;   // Host request access size
    logic [31:0] host_resp_rdata; // Host response read data (from bus)
    
    // ============================================================
    // Response Data Registers (CPU-initiated)
    // ============================================================
    logic [31:0] resp_rdata;    // Received read data
    
    // ============================================================
    // Address Validation Constants
    // ============================================================
    // Host can ONLY access RTL peripheral space (0x5000_0000 - 0x5FFF_FFFF)
    localparam RTL_PERIPH_BASE  = 32'h5000_0000;
    localparam RTL_PERIPH_LIMIT = 32'h6000_0000;
    
    // ============================================================
    // Address Validation
    // ============================================================
    logic host_addr_valid;
    assign host_addr_valid = (host_cap_addr >= RTL_PERIPH_BASE) && 
                             (host_cap_addr < RTL_PERIPH_LIMIT);
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
    logic        in_tx_phase;   // Indicates TX phase active (CPU-initiated)
    logic        in_rx_phase;   // Indicates RX phase active (CPU-initiated)
    logic        in_host_tx_phase; // Indicates TX phase active (Host-initiated response)
    logic        in_host_rx_phase; // Indicates RX phase active (Host-initiated request)

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
    
    // Helper function to identify host-initiated request header
    // Host-initiated request headers use packet type 0010 in bits [7:4]
    function logic is_host_initiated_request_header(input logic [7:0] data);
        return (data[7:4] == 4'b0010);
    endfunction
    
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                // Priority: Check for Host-initiated request first (on RX), then CPU request
                if (rx_valid && is_host_initiated_request_header(rx_data)) begin
                    // Host-initiated request incoming (packet type 0010)
                    next_state = STATE_HOST_RX_HEADER;
                end else if (req) begin
                    // CPU-initiated request
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
                        next_state = STATE_RX_HEADER;
                    end
                end
            end
            
            // TX Write Data States (little-endian: LSB first)
            STATE_TX_WDATA_0: begin  // All writes start here (LSB)
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b00:   next_state = STATE_RX_HEADER;    // Byte: done after 1 byte
                        default: next_state = STATE_TX_WDATA_1;  // Half/Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_1: begin  // Half and Word
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b01:   next_state = STATE_RX_HEADER;    // Half: done after 2 bytes
                        default: next_state = STATE_TX_WDATA_2;  // Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_2: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_3;
            end
            
            STATE_TX_WDATA_3: begin  // Word only (MSB, last byte)
                if (tx_valid && tx_ready) next_state = STATE_RX_HEADER;
            end
            
            // --------------------------------------------------------
            // RX Phase: Header + Data (for all responses with extended header)
            // Data received in little-endian order (LSB first)
            // --------------------------------------------------------
            STATE_RX_HEADER: begin  // Response header byte (type 0001)
                if (rx_valid && rx_ready) begin
                    if (cap_we) begin
                        // Write response: header only
                        next_state = STATE_COMPLETE;
                    end else begin
                        // Read response: header + data
                        next_state = STATE_RX_RDATA_0;
                    end
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
            
            // --------------------------------------------------------
            // Host-initiated request states (Host → FPGA)
            // --------------------------------------------------------
            STATE_HOST_RX_HEADER: begin
                // Header already consumed in IDLE state check, capture parameters
                // and transition to address reception
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
                        // Write: continue receiving data
                        next_state = STATE_HOST_RX_WDATA_0;
                    end else begin
                        // Read: go to bus request (address validation in BUS_REQ state)
                        next_state = STATE_HOST_BUS_REQ;
                    end
                end
            end
            
            STATE_HOST_RX_WDATA_0: begin
                if (rx_valid && rx_ready) begin
                    case (host_cap_size)
                        2'b00:   next_state = STATE_HOST_BUS_REQ;      // Byte: done
                        default: next_state = STATE_HOST_RX_WDATA_1; // Half/Word: continue
                    endcase
                end
            end
            
            STATE_HOST_RX_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    case (host_cap_size)
                        2'b01:   next_state = STATE_HOST_BUS_REQ;      // Half: done
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
            
            STATE_HOST_BUS_REQ: begin
                // Validate address before issuing bus request
                if (host_addr_valid) begin
                    next_state = STATE_HOST_BUS_WAIT;
                end else begin
                    // Invalid address - send error response
                    next_state = STATE_HOST_ERROR;
                end
            end
            
            STATE_HOST_BUS_WAIT: begin
                if (host_bus_ready) begin
                    // Bus transaction complete, send response
                    next_state = STATE_HOST_TX_HEADER;
                end
            end
            
            STATE_HOST_TX_HEADER: begin
                if (tx_valid && tx_ready) begin
                    if (host_cap_we) begin
                        // Write: header only
                        next_state = STATE_HOST_COMPLETE;
                    end else begin
                        // Read: header + data
                        next_state = STATE_HOST_TX_RDATA_0;
                    end
                end
            end
            
            STATE_HOST_TX_RDATA_0: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b00:   next_state = STATE_HOST_COMPLETE;     // Byte: done
                        default: next_state = STATE_HOST_TX_RDATA_1;  // Half/Word: continue
                    endcase
                end
            end
            
            STATE_HOST_TX_RDATA_1: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b01:   next_state = STATE_HOST_COMPLETE;     // Half: done
                        default: next_state = STATE_HOST_TX_RDATA_2;  // Word: continue
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
            
            STATE_HOST_ERROR: begin
                // Send error response header
                if (tx_valid && tx_ready) next_state = STATE_HOST_ERROR_CODE;
            end
            
            STATE_HOST_ERROR_CODE: begin
                // Send error code byte
                if (tx_valid && tx_ready) next_state = STATE_IDLE;
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end

    // ============================================================
    // Capture Request on CAPTURE state (CPU-initiated)
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
    // Capture Host-initiated Request Data
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            host_cap_addr   <= 32'h0;
            host_cap_wdata  <= 32'h0;
            host_cap_we     <= 1'b0;
            host_cap_size   <= 2'b00;
            host_resp_rdata <= 32'h0;
        end else begin
            // Capture header info when entering HOST_RX_HEADER
            if (state == STATE_IDLE && rx_valid && is_host_initiated_request_header(rx_data)) begin
                host_cap_we   <= rx_data[0];
                host_cap_size <= rx_data[3:2];
                host_cap_addr <= 32'h0;
                host_cap_wdata <= 32'h0;
            end
            
            // Capture address bytes (little-endian: LSB first)
            if (rx_valid && rx_ready) begin
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
            
            // Capture bus response data when bus transaction completes
            if (state == STATE_HOST_BUS_WAIT && host_bus_ready) begin
                host_resp_rdata <= host_bus_rdata;
            end
        end
    end

    // ============================================================
    // TX Phase Detection
    // ============================================================
    // CPU-initiated request TX (sending request to host)
    assign in_tx_phase = (state >= STATE_TX_HEADER) && (state <= STATE_TX_WDATA_3);
    
    // Host-initiated response TX (sending response back to host)
    assign in_host_tx_phase = (state == STATE_HOST_TX_HEADER) || 
                              ((state >= STATE_HOST_TX_RDATA_0) && (state <= STATE_HOST_TX_RDATA_3)) ||
                              (state == STATE_HOST_ERROR) || (state == STATE_HOST_ERROR_CODE);
    
    // ============================================================
    // TX Data Multiplexer (Little-Endian: LSB first)
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            // CPU-initiated request TX
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            
            // Host-initiated response TX (packet type 0011)
            STATE_HOST_TX_HEADER:  tx_byte = {4'b0011, host_cap_size, 1'b0, host_cap_we};
            STATE_HOST_TX_RDATA_0: tx_byte = host_resp_rdata[7:0];
            STATE_HOST_TX_RDATA_1: tx_byte = host_resp_rdata[15:8];
            STATE_HOST_TX_RDATA_2: tx_byte = host_resp_rdata[23:16];
            STATE_HOST_TX_RDATA_3: tx_byte = host_resp_rdata[31:24];
            
            // Error response TX (packet type 1111)
            STATE_HOST_ERROR:      tx_byte = {4'b1111, host_cap_size, 1'b0, host_cap_we};
            STATE_HOST_ERROR_CODE: tx_byte = 8'hFF;  // Error code: invalid address
            
            default:          tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    assign tx_valid = in_tx_phase || in_host_tx_phase;

    // ============================================================
    // RX Phase Detection
    // ============================================================
    // CPU-initiated request RX (receiving response from host)
    assign in_rx_phase = (state == STATE_RX_HEADER) ||
                         ((state >= STATE_RX_RDATA_0) && (state <= STATE_RX_RDATA_3));
    
    // Host-initiated request RX (receiving request from host)
    assign in_host_rx_phase = (state == STATE_HOST_RX_HEADER) ||
                              ((state >= STATE_HOST_RX_ADDR_0) && (state <= STATE_HOST_RX_WDATA_3)) ||
                              (state == STATE_IDLE); // Also in IDLE to detect incoming host requests
    
    // ============================================================
    // RX Ready Signal
    // ============================================================
    assign rx_ready = in_rx_phase || in_host_rx_phase;
    
    // ============================================================
    // RX Data Capture (Little-Endian: LSB first)
    // Clear resp_rdata when entering RX phase to avoid stale data
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_rdata <= 32'h0;
        end else if (state == STATE_RX_HEADER && rx_valid && rx_ready && !cap_we) begin
            // Clear rdata when receiving header and about to receive read data
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
    // Bus Ready Signal (CPU-initiated) - asserted in COMPLETE state only
    // ============================================================
    assign ready = (state == STATE_COMPLETE);
    
    // ============================================================
    // Bus Read Data (CPU-initiated)
    // ============================================================
    assign rdata = resp_rdata;
    
    // ============================================================
    // Host Bus Master Interface (Host-initiated requests)
    // ============================================================
    assign host_bus_addr  = host_cap_addr;
    assign host_bus_wdata = host_cap_wdata;
    assign host_bus_we    = host_cap_we;
    assign host_bus_size  = host_cap_size;
    assign host_bus_req   = (state == STATE_HOST_BUS_REQ) || (state == STATE_HOST_BUS_WAIT);

endmodule

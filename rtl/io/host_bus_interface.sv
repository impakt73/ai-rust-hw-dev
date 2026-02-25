// Host Bus Interface Module
// Serializes bus transactions for external host communication
//
// Features:
//   - 32-bit bus slave interface compatible with system bus (CPU->Host requests)
//   - 32-bit bus master interface for Host->CPU requests (via arbiter, currently unused)
//   - 8-bit TX/RX byte stream with valid/ready flow control
//   - Variable-length packets optimized for minimal bandwidth
//   - Extended header format with packet type bits
//   - Little-endian data format for x86/ARM compatibility
//   - No checksums (relies on transport layer if needed)
//
// Protocol (Variable Length, Little-Endian, Extended Header):
//   CPU-initiated request (type 0000):  [ext_header][addr0-3][data...] (FPGA → Host TX)
//   Host response to CPU (type 0001):   [ext_header][data...]          (Host → FPGA RX)
//   Host-initiated request (type 0010): [ext_header][addr0-3][data...] (Host → FPGA RX)
//   FPGA response to Host (type 0011):  [ext_header][data...]          (FPGA → Host TX)
//
// Extended Header Format (1 byte):
//   Bits [7:4]: Packet type
//     0000 = CPU-initiated request (FPGA → Host TX)
//     0001 = Host response to CPU request (Host → FPGA RX)
//     0010 = Host-initiated request (Host → FPGA RX)
//     0011 = FPGA response to Host request (FPGA → Host TX)
//   Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
//   Bit  [1]:   Reserved (0)
//   Bit  [0]:   we (1=write, 0=read)

module host_bus_interface (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // Bus Slave Interface (from System Bus - CPU→Host path)
    input  logic [31:0] addr,
    input  logic [31:0] wdata,
    output logic [31:0] rdata,
    input  logic        we,
    input  logic [1:0]  size,
    input  logic        req,
    output logic        ready,
    
    // Bus Master Interface (to Arbiter - Host→CPU path)
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
    // RX Buffer Instance
    // ============================================================
    logic        buf_pkt_valid;
    logic [3:0]  buf_pkt_type;
    logic        buf_pkt_we;
    logic [1:0]  buf_pkt_size;
    logic [31:0] buf_pkt_addr;
    logic [31:0] buf_pkt_data;
    logic        buf_pkt_ready;
    
    logic        buf_resp_valid;
    logic        buf_req_valid;
    logic        buf_resp_ready;
    logic        buf_req_ready;
    
    host_rx_buffer rx_buf (
        .clk(clk),
        .rst_n(rst_n),
        
        // RX interface from host
        .rx_data(rx_data),
        .rx_valid(rx_valid),
        .rx_ready(rx_ready),
        
        // Unified buffered packet interface
        .packet_valid(buf_pkt_valid),
        .packet_type(buf_pkt_type),
        .packet_we(buf_pkt_we),
        .packet_size(buf_pkt_size),
        .packet_addr(buf_pkt_addr),
        .packet_data(buf_pkt_data),
        .packet_ready(buf_pkt_ready)
    );
    
    assign buf_resp_valid = buf_pkt_valid && (buf_pkt_type == 4'b0001);
    assign buf_req_valid  = buf_pkt_valid && (buf_pkt_type == 4'b0010);
    
    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [4:0] {
        STATE_IDLE        = 5'd0,
        STATE_CAPTURE     = 5'd1,
        
        // TX States for CPU-initiated requests (variable length: 5-9 bytes)
        STATE_TX_HEADER   = 5'd2,   // Header byte
        STATE_TX_ADDR_0   = 5'd3,   // Address[7:0] (little-endian: LSB first)
        STATE_TX_ADDR_1   = 5'd4,   // Address[15:8]
        STATE_TX_ADDR_2   = 5'd5,   // Address[23:16]
        STATE_TX_ADDR_3   = 5'd6,   // Address[31:24]
        STATE_TX_WDATA_0  = 5'd7,   // WData[7:0] (little-endian: LSB first)
        STATE_TX_WDATA_1  = 5'd8,   // WData[15:8] (half/word writes)
        STATE_TX_WDATA_2  = 5'd9,   // WData[23:16] (word writes only)
        STATE_TX_WDATA_3  = 5'd10,  // WData[31:24] (word writes only)
        
        // RX States for CPU-initiated response (buffered via host_rx_buffer)
        STATE_CPU_RESP_WAIT = 5'd11,  // Waiting for buffered response
        
        // Complete state for CPU-initiated transactions
        STATE_COMPLETE    = 5'd12,
        
        // States for Host-initiated response TX (removed STATE_HOST_REQ_PENDING, STATE_HOST_RESP_WAIT)
        STATE_HOST_TX_HEADER   = 5'd15,   // TX response header (packet type 0011)
        STATE_HOST_TX_DATA_0   = 5'd16,   // TX response data byte 0
        STATE_HOST_TX_DATA_1   = 5'd17,   // TX response data byte 1
        STATE_HOST_TX_DATA_2   = 5'd18,   // TX response data byte 2
        STATE_HOST_TX_DATA_3   = 5'd19    // TX response data byte 3
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Request Registers (CPU-initiated path)
    // ============================================================
    logic [31:0] cap_addr;      // Captured address
    logic [31:0] cap_wdata;     // Captured write data
    logic        cap_we;        // Captured write enable
    logic [1:0]  cap_size;      // Captured access size
    
    // ============================================================
    // Host-initiated Request Registers
    // (Removed host_cap_addr, host_cap_wdata - now driven directly from rx buffer)
    // ============================================================
    logic        host_cap_we;     // Captured host request write enable
    logic [1:0]  host_cap_size;   // Captured host request access size
    logic [31:0] host_resp_rdata; // Response data from bus master
    logic        host_resp_valid; // Indicates a complete response is ready to send
    
    // ============================================================
    // Response Data Registers (CPU-initiated path, now deprecated)
    // ============================================================
    // Note: CPU response data now comes from buffer, but keep for backward compat
    logic [31:0] resp_rdata;
    
    // ============================================================
    // TX Data Mux
    // ============================================================
    logic [7:0]  tx_byte;       // Current byte to transmit
    logic        in_tx_phase;   // Indicates TX phase active
    logic        in_rx_phase;   // Indicates RX phase active
    
    // ============================================================
    // TX Phase Detection (declared early for use in capture logic)
    // ============================================================
    logic in_cpu_tx_phase;
    logic in_host_tx_phase;
    
    assign in_cpu_tx_phase = (state >= STATE_TX_HEADER) && (state <= STATE_TX_WDATA_3);
    assign in_host_tx_phase = (state >= STATE_HOST_TX_HEADER) && (state <= STATE_HOST_TX_DATA_3);
    assign in_tx_phase = in_cpu_tx_phase || in_host_tx_phase;
    
    // ============================================================
    // Bus Master Handshake Complete Signal
    // High when host_bus_req and host_bus_ready are both high
    // ============================================================
    logic bus_master_handshake_complete;
    assign bus_master_handshake_complete = host_bus_req && host_bus_ready;

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
    // Priority: CPU-initiated transactions > Host-initiated transactions
    // Host-initiated bus master requests now happen in parallel, not via FSM states
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                // Priority 1: CPU-initiated request (slave interface)
                if (req) begin
                    next_state = STATE_CAPTURE;
                // Priority 2: Host response ready to transmit
                end else if (host_resp_valid) begin
                    next_state = STATE_HOST_TX_HEADER;
                end
            end
            
            STATE_CAPTURE: begin
                next_state = STATE_TX_HEADER;
            end
            
            // --------------------------------------------------------
            // CPU-initiated TX Phase: Header + Address (always) + Data (writes only)
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
                        // Read: no data, wait for buffered response
                        next_state = STATE_CPU_RESP_WAIT;
                    end
                end
            end
            
            // TX Write Data States (little-endian: LSB first)
            STATE_TX_WDATA_0: begin  // All writes start here (LSB)
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b00:   next_state = STATE_CPU_RESP_WAIT;  // Byte: done, wait for response
                        default: next_state = STATE_TX_WDATA_1;     // Half/Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_1: begin  // Half and Word
                if (tx_valid && tx_ready) begin
                    case (cap_size)
                        2'b01:   next_state = STATE_CPU_RESP_WAIT;  // Half: done, wait for response
                        default: next_state = STATE_TX_WDATA_2;     // Word: continue
                    endcase
                end
            end
            
            STATE_TX_WDATA_2: begin  // Word only
                if (tx_valid && tx_ready) next_state = STATE_TX_WDATA_3;
            end
            
            STATE_TX_WDATA_3: begin  // Word only (MSB, last byte)
                if (tx_valid && tx_ready) next_state = STATE_CPU_RESP_WAIT;
            end
            
            // --------------------------------------------------------
            // CPU-initiated RX Phase: Wait for buffered response
            // --------------------------------------------------------
            STATE_CPU_RESP_WAIT: begin
                // Wait for buffer to have valid response (packet type 0001)
                if (buf_resp_valid) begin
                    next_state = STATE_COMPLETE;
                end
            end
            
            // --------------------------------------------------------
            // Complete state - asserts ready for one cycle then returns to IDLE
            // --------------------------------------------------------
            STATE_COMPLETE: begin
                next_state = STATE_IDLE;
            end
            
            // --------------------------------------------------------
            // Host-initiated Response TX (bus master handshake happens in parallel)
            // --------------------------------------------------------
            STATE_HOST_TX_HEADER: begin
                // Send response header (packet type 0011)
                if (tx_valid && tx_ready) begin
                    if (host_cap_we) begin
                        // Write response: header only, no data
                        next_state = STATE_IDLE;
                    end else begin
                        // Read response: send data bytes
                        next_state = STATE_HOST_TX_DATA_0;
                    end
                end
            end
            
            // Host Response Data TX States (little-endian: LSB first)
            STATE_HOST_TX_DATA_0: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b00:   next_state = STATE_IDLE;           // Byte: done
                        default: next_state = STATE_HOST_TX_DATA_1; // Half/Word: continue
                    endcase
                end
            end
            
            STATE_HOST_TX_DATA_1: begin
                if (tx_valid && tx_ready) begin
                    case (host_cap_size)
                        2'b01:   next_state = STATE_IDLE;           // Half: done
                        default: next_state = STATE_HOST_TX_DATA_2; // Word: continue
                    endcase
                end
            end
            
            STATE_HOST_TX_DATA_2: begin
                if (tx_valid && tx_ready) next_state = STATE_HOST_TX_DATA_3;
            end
            
            STATE_HOST_TX_DATA_3: begin
                if (tx_valid && tx_ready) next_state = STATE_IDLE;
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end

    // ============================================================
    // Capture Request Registers
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cap_addr  <= 32'h0;
            cap_wdata <= 32'h0;
            cap_we    <= 1'b0;
            cap_size  <= 2'b00;
            
            host_cap_we    <= 1'b0;
            host_cap_size  <= 2'b00;
            host_resp_rdata <= 32'h0;
            host_resp_valid <= 1'b0;
        end else begin
            // Capture CPU-initiated request
            if (state == STATE_IDLE && req) begin
                cap_addr  <= addr;
                cap_wdata <= wdata;
                cap_we    <= we;
                cap_size  <= size;
            end
            
            // Default: don't change host_resp_valid
            // Priority 1: Clear host_resp_valid when host TX response is complete
            // (transitioning from a host TX state back to idle)
            // This takes priority over setting it, to avoid race condition
            if (state == STATE_HOST_TX_HEADER && tx_valid && tx_ready && host_cap_we) begin
                // Write response complete (header only)
                host_resp_valid <= 1'b0;
            end else if (state == STATE_HOST_TX_DATA_0 && tx_valid && tx_ready && host_cap_size == 2'b00) begin
                // Byte read response complete
                host_resp_valid <= 1'b0;
            end else if (state == STATE_HOST_TX_DATA_1 && tx_valid && tx_ready && host_cap_size == 2'b01) begin
                // Half read response complete
                host_resp_valid <= 1'b0;
            end else if (state == STATE_HOST_TX_DATA_3 && tx_valid && tx_ready) begin
                // Word read response complete
                host_resp_valid <= 1'b0;
            end else if (bus_master_handshake_complete && !host_resp_valid && !in_host_tx_phase) begin
                // Priority 2: Capture data on bus master handshake completion (host-initiated path)
                // Only capture if:
                // 1. We don't already have a pending response (host_resp_valid = 0)
                // 2. We're not currently transmitting a response (in_host_tx_phase = 0)
                // This happens in parallel with the FSM, outside of state machine control
                host_cap_we     <= buf_pkt_we;
                host_cap_size   <= buf_pkt_size;
                host_resp_rdata <= host_bus_rdata;
                host_resp_valid <= 1'b1;
            end
        end
    end

    // ============================================================
    // Buffer Ready Signals
    // ============================================================
    // Accept buffered response when CPU transaction completes
    assign buf_resp_ready = (state == STATE_COMPLETE);
    
    // Accept buffered request when bus master handshake completes
    // and we don't already have a pending response and not in TX phase
    assign buf_req_ready = bus_master_handshake_complete && !host_resp_valid && !in_host_tx_phase;
    
    assign buf_pkt_ready = (buf_resp_ready && buf_resp_valid) || (buf_req_ready && buf_req_valid);
    
    // ============================================================
    // TX Data Multiplexer (Little-Endian: LSB first)
    // Extended Header Format: {packet_type[3:0], size[1:0], 1'b0, we}
    // Packet type 0000 = CPU-initiated request
    // Packet type 0011 = FPGA response to host
    // ============================================================
    always_comb begin
        tx_byte = 8'h00;
        
        case (state)
            // CPU-initiated request packet (type 0000)
            STATE_TX_HEADER:  tx_byte = {4'b0000, cap_size, 1'b0, cap_we};
            STATE_TX_ADDR_0:  tx_byte = cap_addr[7:0];
            STATE_TX_ADDR_1:  tx_byte = cap_addr[15:8];
            STATE_TX_ADDR_2:  tx_byte = cap_addr[23:16];
            STATE_TX_ADDR_3:  tx_byte = cap_addr[31:24];
            STATE_TX_WDATA_0: tx_byte = cap_wdata[7:0];
            STATE_TX_WDATA_1: tx_byte = cap_wdata[15:8];
            STATE_TX_WDATA_2: tx_byte = cap_wdata[23:16];
            STATE_TX_WDATA_3: tx_byte = cap_wdata[31:24];
            
            // Host-initiated response packet (type 0011)
            STATE_HOST_TX_HEADER:  tx_byte = {4'b0011, host_cap_size, 1'b0, host_cap_we};
            STATE_HOST_TX_DATA_0:  tx_byte = host_resp_rdata[7:0];
            STATE_HOST_TX_DATA_1:  tx_byte = host_resp_rdata[15:8];
            STATE_HOST_TX_DATA_2:  tx_byte = host_resp_rdata[23:16];
            STATE_HOST_TX_DATA_3:  tx_byte = host_resp_rdata[31:24];
            
            default:          tx_byte = 8'h00;
        endcase
    end
    
    assign tx_data = tx_byte;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    assign tx_valid = in_tx_phase;

    // ============================================================
    // Bus Ready Signal - asserted in COMPLETE state only
    // ============================================================
    assign ready = (state == STATE_COMPLETE);
    
    // ============================================================
    // Bus Read Data (from buffered response)
    // ============================================================
    assign rdata = buf_pkt_data;
    
    // ============================================================
    // Bus Master Interface (Host→CPU path)
    // Drives bus requests directly from rx buffer outputs
    // Request is asserted whenever a buffered request is available
    // and we don't already have a pending response to transmit
    // ============================================================
    assign host_bus_addr  = buf_pkt_addr;
    assign host_bus_wdata = buf_pkt_data;
    assign host_bus_we    = buf_pkt_we;
    assign host_bus_size  = buf_pkt_size;
    assign host_bus_req   = buf_req_valid && !host_resp_valid;

endmodule

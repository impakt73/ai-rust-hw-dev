// Host RX Buffer Module
// Buffers incoming host data with separate response and request buffers
//
// Features:
//   - Dual buffer design: one for responses (type 0001), one for requests (type 0010)
//   - Parses extended header format to route packets to correct buffer
//   - Implements backpressure via rx_ready when both buffers are full
//   - Little-endian data format for x86/ARM compatibility
//   - Supports variable-length packets (1-9 bytes)
//
// Packet Types:
//   0001 = Host response to CPU request (response buffer)
//   0010 = Host-initiated request (request buffer)
//
// Buffer Capacity:
//   Response buffer: header + up to 4 data bytes (read responses)
//   Request buffer: header + 4 address bytes + up to 4 data bytes (write requests)

module host_rx_buffer (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // Buffered Response Packet (for CPU-initiated requests)
    output logic        resp_valid,       // Complete response packet available
    output logic        resp_we,          // Response write enable (echoed from request)
    output logic [1:0]  resp_size,        // Response access size
    output logic [31:0] resp_rdata,       // Response read data (for reads)
    input  logic        resp_consumed,    // Response has been processed
    
    // Buffered Request Packet (for Host-initiated requests)
    output logic        req_valid,        // Complete request packet available
    output logic        req_we,           // Request write enable
    output logic [1:0]  req_size,         // Request access size
    output logic [31:0] req_addr,         // Request address
    output logic [31:0] req_wdata,        // Request write data (for writes)
    input  logic        req_consumed      // Request has been processed
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [3:0] {
        STATE_IDLE          = 4'd0,
        STATE_RESP_RDATA_0  = 4'd1,   // Receiving response data byte 0
        STATE_RESP_RDATA_1  = 4'd2,   // Receiving response data byte 1
        STATE_RESP_RDATA_2  = 4'd3,   // Receiving response data byte 2
        STATE_RESP_RDATA_3  = 4'd4,   // Receiving response data byte 3
        STATE_REQ_ADDR_0    = 4'd5,   // Receiving request address byte 0
        STATE_REQ_ADDR_1    = 4'd6,   // Receiving request address byte 1
        STATE_REQ_ADDR_2    = 4'd7,   // Receiving request address byte 2
        STATE_REQ_ADDR_3    = 4'd8,   // Receiving request address byte 3
        STATE_REQ_WDATA_0   = 4'd9,   // Receiving request write data byte 0
        STATE_REQ_WDATA_1   = 4'd10,  // Receiving request write data byte 1
        STATE_REQ_WDATA_2   = 4'd11,  // Receiving request write data byte 2
        STATE_REQ_WDATA_3   = 4'd12   // Receiving request write data byte 3
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Storage Registers
    // ============================================================
    // Response packet storage
    logic        resp_valid_reg;
    logic        resp_we_reg;
    logic [1:0]  resp_size_reg;
    logic [31:0] resp_rdata_reg;
    
    // Request packet storage
    logic        req_valid_reg;
    logic        req_we_reg;
    logic [1:0]  req_size_reg;
    logic [31:0] req_addr_reg;
    logic [31:0] req_wdata_reg;
    
    // Temporary header fields (used during parsing)
    logic        temp_we;
    logic [1:0]  temp_size;
    
    // Combinational signals for header parsing
    logic [3:0]  header_packet_type;
    logic        header_we;
    logic [1:0]  header_size;
    
    // ============================================================
    // Header Parsing (combinational)
    // ============================================================
    assign header_packet_type = rx_data[7:4];
    assign header_we          = rx_data[0];
    assign header_size        = rx_data[3:2];
    
    // ============================================================
    // Output Assignments
    // ============================================================
    assign resp_valid = resp_valid_reg;
    assign resp_we    = resp_we_reg;
    assign resp_size  = resp_size_reg;
    assign resp_rdata = resp_rdata_reg;
    
    assign req_valid  = req_valid_reg;
    assign req_we     = req_we_reg;
    assign req_size   = req_size_reg;
    assign req_addr   = req_addr_reg;
    assign req_wdata  = req_wdata_reg;
    
    // ============================================================
    // rx_ready Logic
    // ============================================================
    // Ready to receive when:
    // 1. In IDLE state AND at least one buffer is available to store a new packet
    //    (we don't know the packet type until we see the header, so we must accept
    //    if either buffer could potentially store the incoming packet)
    // 2. OR actively receiving a packet (must continue accepting data)
    //
    // We lower rx_ready ONLY when both buffers are full AND we're in IDLE.
    // This satisfies Rule 5: Target must accept data even with outstanding request.
    logic can_accept_new_packet;
    logic is_receiving_packet;
    
    // At least one buffer must be free to start accepting a new packet
    assign can_accept_new_packet = !resp_valid_reg || !req_valid_reg;
    
    // Currently in the middle of receiving a packet (not in IDLE)
    assign is_receiving_packet = (state != STATE_IDLE);
    
    // Ready when: actively receiving OR (in IDLE with at least one free buffer)
    assign rx_ready = is_receiving_packet || can_accept_new_packet;
    
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
    // Next State Logic
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                if (rx_valid && rx_ready) begin
                    // Parse header to determine packet type
                    // Only accept if the destination buffer is available
                    case (header_packet_type)
                        4'b0001: begin  // Host response to CPU request
                            // Only accept response if response buffer is free
                            if (!resp_valid_reg) begin
                                if (header_we) begin
                                    // Write response - header only, mark complete
                                    next_state = STATE_IDLE;  // Stays in idle, resp_valid set in ff block
                                end else begin
                                    // Read response - need to receive data bytes
                                    next_state = STATE_RESP_RDATA_0;
                                end
                            end
                            // If resp_valid_reg is set, stay in IDLE (reject this packet)
                            // The sender must retry when rx_ready goes high again
                        end
                        4'b0010: begin  // Host-initiated request
                            // Only accept request if request buffer is free
                            if (!req_valid_reg) begin
                                next_state = STATE_REQ_ADDR_0;
                            end
                            // If req_valid_reg is set, stay in IDLE (reject this packet)
                        end
                        default: begin
                            // Unknown packet type, stay in idle and ignore
                            next_state = STATE_IDLE;
                        end
                    endcase
                end
            end
            
            // Response data receive states - use temp_size which was captured from header
            STATE_RESP_RDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_RESP_RDATA_1;
                    end
                end
            end
            
            STATE_RESP_RDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        next_state = STATE_IDLE;  // Halfword: done
                    end else begin
                        next_state = STATE_RESP_RDATA_2;
                    end
                end
            end
            
            STATE_RESP_RDATA_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_RESP_RDATA_3;
                end
            end
            
            STATE_RESP_RDATA_3: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_IDLE;  // Word: done
                end
            end
            
            // Request address receive states
            STATE_REQ_ADDR_0: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_1;
            end
            
            STATE_REQ_ADDR_1: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_2;
            end
            
            STATE_REQ_ADDR_2: begin
                if (rx_valid && rx_ready) next_state = STATE_REQ_ADDR_3;
            end
            
            STATE_REQ_ADDR_3: begin
                if (rx_valid && rx_ready) begin
                    if (temp_we) begin
                        // Write request - receive write data
                        next_state = STATE_REQ_WDATA_0;
                    end else begin
                        // Read request - done receiving
                        next_state = STATE_IDLE;
                    end
                end
            end
            
            // Request write data receive states
            STATE_REQ_WDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_REQ_WDATA_1;
                    end
                end
            end
            
            STATE_REQ_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        next_state = STATE_IDLE;  // Halfword: done
                    end else begin
                        next_state = STATE_REQ_WDATA_2;
                    end
                end
            end
            
            STATE_REQ_WDATA_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_REQ_WDATA_3;
                end
            end
            
            STATE_REQ_WDATA_3: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_IDLE;  // Word: done
                end
            end
            
            default: next_state = STATE_IDLE;
        endcase
    end
    
    // ============================================================
    // Data Capture Logic
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            resp_valid_reg <= 1'b0;
            resp_we_reg    <= 1'b0;
            resp_size_reg  <= 2'b00;
            resp_rdata_reg <= 32'h0;
            
            req_valid_reg  <= 1'b0;
            req_we_reg     <= 1'b0;
            req_size_reg   <= 2'b00;
            req_addr_reg   <= 32'h0;
            req_wdata_reg  <= 32'h0;
            
            temp_we        <= 1'b0;
            temp_size      <= 2'b00;
        end else begin
            // Handle consumed signals
            if (resp_consumed) begin
                resp_valid_reg <= 1'b0;
            end
            if (req_consumed) begin
                req_valid_reg <= 1'b0;
            end
            
            // Handle state machine data capture
            if (rx_valid && rx_ready) begin
                case (state)
                    STATE_IDLE: begin
                        // Parse header - only capture if destination buffer is free
                        case (header_packet_type)
                            4'b0001: begin  // Host response to CPU request
                                // Only capture if response buffer is free
                                if (!resp_valid_reg) begin
                                    resp_we_reg    <= header_we;
                                    resp_size_reg  <= header_size;
                                    temp_size      <= header_size;  // Store for later state decisions
                                    resp_rdata_reg <= 32'h0;  // Clear for accumulation
                                    
                                    if (header_we) begin
                                        // Write response - complete immediately
                                        resp_valid_reg <= 1'b1;
                                    end
                                end
                            end
                            4'b0010: begin  // Host-initiated request
                                // Only capture if request buffer is free
                                if (!req_valid_reg) begin
                                    temp_we        <= header_we;
                                    temp_size      <= header_size;
                                    req_we_reg     <= header_we;
                                    req_size_reg   <= header_size;
                                    req_addr_reg   <= 32'h0;  // Clear for accumulation
                                    req_wdata_reg  <= 32'h0;  // Clear for accumulation
                                end
                            end
                            default: ; // Ignore unknown packet types
                        endcase
                    end
                    
                    // Response data capture (little-endian) - use temp_size for decisions
                    STATE_RESP_RDATA_0: begin
                        resp_rdata_reg[7:0] <= rx_data;
                        if (temp_size == 2'b00) begin
                            resp_valid_reg <= 1'b1;  // Byte read complete
                        end
                    end
                    
                    STATE_RESP_RDATA_1: begin
                        resp_rdata_reg[15:8] <= rx_data;
                        if (temp_size == 2'b01) begin
                            resp_valid_reg <= 1'b1;  // Halfword read complete
                        end
                    end
                    
                    STATE_RESP_RDATA_2: begin
                        resp_rdata_reg[23:16] <= rx_data;
                    end
                    
                    STATE_RESP_RDATA_3: begin
                        resp_rdata_reg[31:24] <= rx_data;
                        resp_valid_reg <= 1'b1;  // Word read complete
                    end
                    
                    // Request address capture (little-endian)
                    STATE_REQ_ADDR_0: req_addr_reg[7:0]   <= rx_data;
                    STATE_REQ_ADDR_1: req_addr_reg[15:8]  <= rx_data;
                    STATE_REQ_ADDR_2: req_addr_reg[23:16] <= rx_data;
                    STATE_REQ_ADDR_3: begin
                        req_addr_reg[31:24] <= rx_data;
                        if (!temp_we) begin
                            req_valid_reg <= 1'b1;  // Read request complete
                        end
                    end
                    
                    // Request write data capture (little-endian)
                    STATE_REQ_WDATA_0: begin
                        req_wdata_reg[7:0] <= rx_data;
                        if (temp_size == 2'b00) begin
                            req_valid_reg <= 1'b1;  // Byte write complete
                        end
                    end
                    
                    STATE_REQ_WDATA_1: begin
                        req_wdata_reg[15:8] <= rx_data;
                        if (temp_size == 2'b01) begin
                            req_valid_reg <= 1'b1;  // Halfword write complete
                        end
                    end
                    
                    STATE_REQ_WDATA_2: begin
                        req_wdata_reg[23:16] <= rx_data;
                    end
                    
                    STATE_REQ_WDATA_3: begin
                        req_wdata_reg[31:24] <= rx_data;
                        req_valid_reg <= 1'b1;  // Word write complete
                    end
                    
                    default: ;
                endcase
            end
        end
    end

endmodule

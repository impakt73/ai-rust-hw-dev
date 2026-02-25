// Host RX Buffer Module
// Buffers one incoming host packet at a time using a unified packet interface
//
// Features:
//   - Single unified packet buffer (`packet_req` indicates request vs response)
//   - Parses extended header format for packet decode
//   - Implements backpressure via rx_ready when unified buffer is full
//   - Little-endian data format for x86/ARM compatibility
//   - Supports variable-length packets (1-9 bytes)
//
// Packet Types:
//   0001 = Host response to CPU request
//   0010 = Host-initiated request
//
// Buffer Capacity:
//   Exactly one complete packet (response OR request)

module host_rx_buffer (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // RX Interface (from External Host)
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // Unified Buffered Packet Interface
    output logic        packet_valid,      // Complete packet available
    output logic        packet_req,        // 1=request packet, 0=response packet
    output logic        packet_we,         // Access write-enable
    output logic [1:0]  packet_size,       // Access size
    output logic [31:0] packet_addr,       // Valid for request packets
    output logic [31:0] packet_data,       // Request wdata or response rdata
    input  logic        packet_ready       // Consumer has accepted packet
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
    logic        packet_valid_reg;
    logic        packet_req_reg;
    logic        packet_we_reg;
    logic [1:0]  packet_size_reg;
    logic [31:0] packet_addr_reg;
    logic [31:0] packet_data_reg;
    
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
    assign packet_valid = packet_valid_reg;
    assign packet_req   = packet_req_reg;
    assign packet_we    = packet_we_reg;
    assign packet_size  = packet_size_reg;
    assign packet_addr  = packet_addr_reg;
    assign packet_data  = packet_data_reg;
    
    // ============================================================
    // rx_ready Logic
    // ============================================================
    // Ready to receive when:
    // 1. In IDLE state and unified packet buffer is empty
    // 2. OR actively receiving a packet
    logic can_accept_new_packet;
    logic is_receiving_packet;
    
    // Unified packet buffer must be free to start accepting a new packet
    assign can_accept_new_packet = !packet_valid_reg;
    
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
                    case (header_packet_type)
                        4'b0001: begin  // Host response to CPU request
                            if (header_we) begin
                                // Write response has no payload bytes
                                next_state = STATE_IDLE;
                            end else begin
                                next_state = STATE_RESP_RDATA_0;
                            end
                        end
                        4'b0010: begin  // Host-initiated request
                            next_state = STATE_REQ_ADDR_0;
                        end
                        default: begin
                            // Unknown packet type, stay in idle and ignore
                            next_state = STATE_IDLE;
                        end
                    endcase
                end
            end
            
            // Response data receive states
            STATE_RESP_RDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (packet_size_reg == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_RESP_RDATA_1;
                    end
                end
            end
            
            STATE_RESP_RDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (packet_size_reg == 2'b01) begin
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
                    if (packet_we_reg) begin
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
                    if (packet_size_reg == 2'b00) begin
                        next_state = STATE_IDLE;  // Byte: done
                    end else begin
                        next_state = STATE_REQ_WDATA_1;
                    end
                end
            end
            
            STATE_REQ_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (packet_size_reg == 2'b01) begin
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
            packet_valid_reg <= 1'b0;
            packet_req_reg   <= 1'b0;
            packet_we_reg    <= 1'b0;
            packet_size_reg  <= 2'b00;
            packet_addr_reg  <= 32'h0;
            packet_data_reg  <= 32'h0;
        end else begin
            if (packet_ready) begin
                packet_valid_reg <= 1'b0;
            end
            
            // Handle state machine data capture
            if (rx_valid && rx_ready) begin
                case (state)
                    STATE_IDLE: begin
                        case (header_packet_type)
                            4'b0001: begin  // Host response to CPU request
                                packet_req_reg   <= 1'b0;
                                packet_we_reg    <= header_we;
                                packet_size_reg  <= header_size;
                                packet_addr_reg  <= 32'h0;
                                packet_data_reg  <= 32'h0;
                                if (header_we) begin
                                    packet_valid_reg <= 1'b1;
                                end
                            end
                            4'b0010: begin  // Host-initiated request
                                packet_req_reg   <= 1'b1;
                                packet_we_reg    <= header_we;
                                packet_size_reg  <= header_size;
                                packet_addr_reg  <= 32'h0;
                                packet_data_reg  <= 32'h0;
                            end
                            default: ; // Ignore unknown packet types
                        endcase
                    end
                    
                    // Response data capture (little-endian)
                    STATE_RESP_RDATA_0: begin
                        packet_data_reg[7:0] <= rx_data;
                        if (packet_size_reg == 2'b00) begin
                            packet_valid_reg <= 1'b1;
                        end
                    end
                    
                    STATE_RESP_RDATA_1: begin
                        packet_data_reg[15:8] <= rx_data;
                        if (packet_size_reg == 2'b01) begin
                            packet_valid_reg <= 1'b1;
                        end
                    end
                    
                    STATE_RESP_RDATA_2: begin
                        packet_data_reg[23:16] <= rx_data;
                    end
                    
                    STATE_RESP_RDATA_3: begin
                        packet_data_reg[31:24] <= rx_data;
                        packet_valid_reg <= 1'b1;
                    end
                    
                    // Request address capture (little-endian)
                    STATE_REQ_ADDR_0: packet_addr_reg[7:0]   <= rx_data;
                    STATE_REQ_ADDR_1: packet_addr_reg[15:8]  <= rx_data;
                    STATE_REQ_ADDR_2: packet_addr_reg[23:16] <= rx_data;
                    STATE_REQ_ADDR_3: begin
                        packet_addr_reg[31:24] <= rx_data;
                        if (!packet_we_reg) begin
                            packet_valid_reg <= 1'b1;
                        end
                    end
                    
                    // Request write data capture (little-endian)
                    STATE_REQ_WDATA_0: begin
                        packet_data_reg[7:0] <= rx_data;
                        if (packet_size_reg == 2'b00) begin
                            packet_valid_reg <= 1'b1;
                        end
                    end
                    
                    STATE_REQ_WDATA_1: begin
                        packet_data_reg[15:8] <= rx_data;
                        if (packet_size_reg == 2'b01) begin
                            packet_valid_reg <= 1'b1;
                        end
                    end
                    
                    STATE_REQ_WDATA_2: begin
                        packet_data_reg[23:16] <= rx_data;
                    end
                    
                    STATE_REQ_WDATA_3: begin
                        packet_data_reg[31:24] <= rx_data;
                        packet_valid_reg <= 1'b1;
                    end
                    
                    default: ;
                endcase
            end
        end
    end

endmodule

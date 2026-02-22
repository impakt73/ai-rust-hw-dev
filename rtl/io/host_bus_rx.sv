// Host Bus RX Module
// Receives and parses incoming byte stream, buffers complete packets
//
// Features:
//   - Parses packet header to extract type/size/we
//   - Accumulates address bytes (little-endian) for request packets
//   - Accumulates data bytes (little-endian) for responses and write requests
//   - Buffers complete packets until consumed
//   - Separate outputs for response packets (type 0001) and request packets (type 0010)
//   - Backpressure when buffer is full
//
// Key Assumption: Only one packet type buffered at a time (response OR request, not both)

module host_bus_rx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // ============================================================
    // RX Byte Stream Interface (from UART/transport)
    // ============================================================
    input  logic [7:0]  rx_data,
    input  logic        rx_valid,
    output logic        rx_ready,
    
    // ============================================================
    // Response Packet Output (Type 0001: Host → CPU)
    // Valid when complete response packet has been received
    // ============================================================
    output logic        resp_valid,       // Complete packet ready
    output logic        resp_we,          // Write enable (echoed from req)
    output logic [1:0]  resp_size,        // Access size (00=byte, 01=half, 10=word)
    output logic [31:0] resp_rdata,       // Read data (0 for writes)
    input  logic        resp_consumed,    // Pulse: consumer has latched data
    
    // ============================================================
    // Request Packet Output (Type 0010: Host → Target)
    // Valid when complete request packet has been received
    // ============================================================
    output logic        req_valid,        // Complete packet ready
    output logic        req_we,           // Write enable
    output logic [1:0]  req_size,         // Access size
    output logic [31:0] req_addr,         // Address
    output logic [31:0] req_wdata,        // Write data (0 for reads)
    input  logic        req_consumed      // Pulse: consumer has latched data
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [4:0] {
        STATE_IDLE          = 5'd0,
        STATE_RESP_DATA_0   = 5'd1,
        STATE_RESP_DATA_1   = 5'd2,
        STATE_RESP_DATA_2   = 5'd3,
        STATE_RESP_DATA_3   = 5'd4,
        STATE_RESP_COMPLETE = 5'd5,
        STATE_REQ_ADDR_0    = 5'd6,
        STATE_REQ_ADDR_1    = 5'd7,
        STATE_REQ_ADDR_2    = 5'd8,
        STATE_REQ_ADDR_3    = 5'd9,
        STATE_REQ_WDATA_0   = 5'd10,
        STATE_REQ_WDATA_1   = 5'd11,
        STATE_REQ_WDATA_2   = 5'd12,
        STATE_REQ_WDATA_3   = 5'd13,
        STATE_REQ_COMPLETE  = 5'd14
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Storage Registers
    // ============================================================
    // Response packet storage
    logic        resp_buf_we;
    logic [1:0]  resp_buf_size;
    logic [31:0] resp_buf_rdata;
    
    // Request packet storage
    logic        req_buf_we;
    logic [1:0]  req_buf_size;
    logic [31:0] req_buf_addr;
    logic [31:0] req_buf_wdata;
    
    // Temporary header fields (captured during header reception)
    logic [3:0]  temp_packet_type;
    logic        temp_we;
    logic [1:0]  temp_size;
    
    // Byte counter for accumulation
    logic [2:0]  byte_count;
    
    // ============================================================
    // Header Parsing (combinational)
    // ============================================================
    logic [3:0]  header_packet_type;
    logic        header_we;
    logic [1:0]  header_size;
    
    assign header_packet_type = rx_data[7:4];
    assign header_we          = rx_data[0];
    assign header_size        = rx_data[3:2];
    
    // ============================================================
    // Output Assignments
    // ============================================================
    assign resp_valid = (state == STATE_RESP_COMPLETE);
    assign resp_we    = resp_buf_we;
    assign resp_size  = resp_buf_size;
    assign resp_rdata = resp_buf_rdata;
    
    assign req_valid = (state == STATE_REQ_COMPLETE);
    assign req_we    = req_buf_we;
    assign req_size  = req_buf_size;
    assign req_addr  = req_buf_addr;
    assign req_wdata = req_buf_wdata;
    
    // ============================================================
    // Ready Logic
    // ============================================================
    // Assert backpressure when packet is complete and not yet consumed
    assign rx_ready = (state != STATE_RESP_COMPLETE) && (state != STATE_REQ_COMPLETE);
    
    // ============================================================
    // State Machine - Sequential Logic
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= STATE_IDLE;
        end else begin
            state <= next_state;
        end
    end
    
    // ============================================================
    // State Machine - Next State Logic
    // ============================================================
    always_comb begin
        next_state = state;
        
        case (state)
            STATE_IDLE: begin
                if (rx_valid && rx_ready) begin
                    // Parse header and route based on packet type
                    if (header_packet_type == 4'b0001) begin
                        // Response packet (type 0001)
                        if (header_we) begin
                            // Write response - header only, no data
                            next_state = STATE_RESP_COMPLETE;
                        end else begin
                            // Read response - receive data bytes
                            next_state = STATE_RESP_DATA_0;
                        end
                    end else if (header_packet_type == 4'b0010) begin
                        // Request packet (type 0010) - proceed to address reception
                        next_state = STATE_REQ_ADDR_0;
                    end
                    // else: Unknown packet type - stay in IDLE (drop packet)
                end
            end
            
            // Response packet data reception
            STATE_RESP_DATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        // Byte size - packet complete
                        next_state = STATE_RESP_COMPLETE;
                    end else begin
                        // Half or word - continue
                        next_state = STATE_RESP_DATA_1;
                    end
                end
            end
            
            STATE_RESP_DATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        // Half size - packet complete
                        next_state = STATE_RESP_COMPLETE;
                    end else begin
                        // Word - continue
                        next_state = STATE_RESP_DATA_2;
                    end
                end
            end
            
            STATE_RESP_DATA_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_RESP_DATA_3;
                end
            end
            
            STATE_RESP_DATA_3: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_RESP_COMPLETE;
                end
            end
            
            STATE_RESP_COMPLETE: begin
                if (resp_consumed) begin
                    next_state = STATE_IDLE;
                end
            end
            
            // Request packet address reception (always 4 bytes)
            STATE_REQ_ADDR_0: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_REQ_ADDR_1;
                end
            end
            
            STATE_REQ_ADDR_1: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_REQ_ADDR_2;
                end
            end
            
            STATE_REQ_ADDR_2: begin
                if (rx_valid && rx_ready) begin
                    next_state = STATE_REQ_ADDR_3;
                end
            end
            
            STATE_REQ_ADDR_3: begin
                if (rx_valid && rx_ready) begin
                    if (temp_we) begin
                        // Write request - receive data
                        next_state = STATE_REQ_WDATA_0;
                    end else begin
                        // Read request - packet complete
                        next_state = STATE_REQ_COMPLETE;
                    end
                end
            end
            
            // Request packet write data reception (0-4 bytes depending on size)
            STATE_REQ_WDATA_0: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b00) begin
                        // Byte size - packet complete
                        next_state = STATE_REQ_COMPLETE;
                    end else begin
                        // Half or word - continue
                        next_state = STATE_REQ_WDATA_1;
                    end
                end
            end
            
            STATE_REQ_WDATA_1: begin
                if (rx_valid && rx_ready) begin
                    if (temp_size == 2'b01) begin
                        // Half size - packet complete
                        next_state = STATE_REQ_COMPLETE;
                    end else begin
                        // Word - continue
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
                    next_state = STATE_REQ_COMPLETE;
                end
            end
            
            STATE_REQ_COMPLETE: begin
                if (req_consumed) begin
                    next_state = STATE_IDLE;
                end
            end
            
            default: begin
                next_state = STATE_IDLE;
            end
        endcase
    end
    
    // ============================================================
    // Data Path - Byte Accumulation and Buffer Updates
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            temp_packet_type <= 4'h0;
            temp_we          <= 1'b0;
            temp_size        <= 2'b00;
            byte_count       <= 3'd0;
            
            resp_buf_we    <= 1'b0;
            resp_buf_size  <= 2'b00;
            resp_buf_rdata <= 32'h0;
            
            req_buf_we    <= 1'b0;
            req_buf_size  <= 2'b00;
            req_buf_addr  <= 32'h0;
            req_buf_wdata <= 32'h0;
        end else begin
            // Capture header when in IDLE state and classify
            if (state == STATE_IDLE && rx_valid && rx_ready) begin
                temp_packet_type <= header_packet_type;
                temp_we          <= header_we;
                temp_size        <= header_size;
                byte_count       <= 3'd0;
`ifndef SYNTHESIS
                if (header_packet_type == 4'b0001 || header_packet_type == 4'b0010) begin
                    assert (header_size != 2'b11)
                    else $error("host_bus_rx: invalid header size encoding 2'b11");
                end
`endif
                
                // For write responses, set metadata immediately (transition to RESP_COMPLETE)
                if (header_packet_type == 4'b0001 && header_we) begin
                    resp_buf_we    <= header_we;
                    resp_buf_size  <= header_size;
                    resp_buf_rdata <= 32'h0;  // No data for write responses
                end
            end
            
            // Response data accumulation (little-endian)
            if ((state >= STATE_RESP_DATA_0 && state <= STATE_RESP_DATA_3) && rx_valid && rx_ready) begin
                case (byte_count)
                    3'd0: resp_buf_rdata[7:0]   <= rx_data;
                    3'd1: resp_buf_rdata[15:8]  <= rx_data;
                    3'd2: resp_buf_rdata[23:16] <= rx_data;
                    3'd3: resp_buf_rdata[31:24] <= rx_data;
                    default: ;
                endcase
                byte_count <= byte_count + 3'd1;
                
                // Update metadata when transitioning to complete
                if (next_state == STATE_RESP_COMPLETE) begin
                    resp_buf_we   <= temp_we;
                    resp_buf_size <= temp_size;
                end
            end
            
            // Request address accumulation (always 4 bytes, little-endian)
            if ((state >= STATE_REQ_ADDR_0 && state <= STATE_REQ_ADDR_3) && rx_valid && rx_ready) begin
                case (byte_count)
                    3'd0: req_buf_addr[7:0]   <= rx_data;
                    3'd1: req_buf_addr[15:8]  <= rx_data;
                    3'd2: req_buf_addr[23:16] <= rx_data;
                    3'd3: req_buf_addr[31:24] <= rx_data;
                    default: ;
                endcase
                byte_count <= byte_count + 3'd1;
                
                // Update metadata when address is complete
                if (state == STATE_REQ_ADDR_3) begin
                    req_buf_we   <= temp_we;
                    req_buf_size <= temp_size;
                    byte_count   <= 3'd0;  // Reset for wdata accumulation
                    if (!temp_we) begin
                        req_buf_wdata <= 32'h0;  // Read request - no data
                    end
                end
            end
            
            // Request write data accumulation (0-4 bytes, little-endian)
            if ((state >= STATE_REQ_WDATA_0 && state <= STATE_REQ_WDATA_3) && rx_valid && rx_ready) begin
                case (byte_count)
                    3'd0: req_buf_wdata[7:0]   <= rx_data;
                    3'd1: req_buf_wdata[15:8]  <= rx_data;
                    3'd2: req_buf_wdata[23:16] <= rx_data;
                    3'd3: req_buf_wdata[31:24] <= rx_data;
                    default: ;
                endcase
                byte_count <= byte_count + 3'd1;
            end
        end
    end

endmodule

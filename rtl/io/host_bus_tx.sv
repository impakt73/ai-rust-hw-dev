// Host Bus TX Module
// Accepts structured bus transactions, serializes to byte stream
//
// Features:
//   - Accepts CPU-initiated requests via bus slave interface
//   - Accepts host-initiated responses via dedicated interface
//   - Serializes packets to byte stream (header + addr + data)
//   - Variable-length packets based on size field
//   - Priority arbitration (CPU requests > host responses)
//   - Ready/valid handshake on byte stream output

module host_bus_tx (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // ============================================================
    // TX Byte Stream Interface (to UART/transport)
    // ============================================================
    output logic [7:0]  tx_data,
    output logic        tx_valid,
    input  logic        tx_ready,
    
    // ============================================================
    // CPU Request Input (Type 0000: CPU → Host)
    // Bus slave interface from CPU-side transactions
    // ============================================================
    input  logic [31:0] cpu_req_addr,
    input  logic [31:0] cpu_req_wdata,
    input  logic        cpu_req_we,
    input  logic [1:0]  cpu_req_size,
    input  logic        cpu_req_valid,    // Request present
    output logic        cpu_req_ready,    // TX module ready to accept
    
    // ============================================================
    // Host Response Input (Type 0011: Target → Host)
    // Response data for host-initiated requests
    // ============================================================
    input  logic [31:0] host_resp_rdata,
    input  logic        host_resp_we,     // Echoed from original request
    input  logic [1:0]  host_resp_size,   // Echoed from original request
    input  logic        host_resp_valid,  // Response present
    output logic        host_resp_ready   // TX module ready to accept
);

    // ============================================================
    // State Machine
    // ============================================================
    typedef enum logic [4:0] {
        STATE_IDLE          = 5'd0,
        STATE_CPU_HEADER    = 5'd1,
        STATE_CPU_ADDR_0    = 5'd2,
        STATE_CPU_ADDR_1    = 5'd3,
        STATE_CPU_ADDR_2    = 5'd4,
        STATE_CPU_ADDR_3    = 5'd5,
        STATE_CPU_WDATA_0   = 5'd6,
        STATE_CPU_WDATA_1   = 5'd7,
        STATE_CPU_WDATA_2   = 5'd8,
        STATE_CPU_WDATA_3   = 5'd9,
        STATE_HOST_HEADER   = 5'd10,
        STATE_HOST_DATA_0   = 5'd11,
        STATE_HOST_DATA_1   = 5'd12,
        STATE_HOST_DATA_2   = 5'd13,
        STATE_HOST_DATA_3   = 5'd14
    } state_t;
    
    state_t state, next_state;
    
    // ============================================================
    // Captured Input Registers
    // ============================================================
    // CPU request capture
    logic [31:0] cpu_req_addr_reg;
    logic [31:0] cpu_req_wdata_reg;
    logic        cpu_req_we_reg;
    logic [1:0]  cpu_req_size_reg;
    
    // Host response capture
    logic [31:0] host_resp_rdata_reg;
    logic        host_resp_we_reg;
    logic [1:0]  host_resp_size_reg;
    
    // ============================================================
    // Ready Signaling
    // ============================================================
    // CPU requests are accepted when FSM is IDLE
    assign cpu_req_ready = (state == STATE_IDLE) && cpu_req_valid;
    
    // Host responses are accepted when FSM is IDLE and no CPU request pending
    assign host_resp_ready = (state == STATE_IDLE) && !cpu_req_valid && host_resp_valid;
    
    // ============================================================
    // TX Valid Signal
    // ============================================================
    // Valid when actively transmitting (not IDLE)
    assign tx_valid = (state != STATE_IDLE);
    
    // ============================================================
    // Byte Serialization Mux
    // ============================================================
    always_comb begin
        tx_data = 8'h00;
        
        case (state)
            // CPU packet (type 0000)
            STATE_CPU_HEADER:  tx_data = {4'b0000, cpu_req_size_reg, 1'b0, cpu_req_we_reg};
            STATE_CPU_ADDR_0:  tx_data = cpu_req_addr_reg[7:0];
            STATE_CPU_ADDR_1:  tx_data = cpu_req_addr_reg[15:8];
            STATE_CPU_ADDR_2:  tx_data = cpu_req_addr_reg[23:16];
            STATE_CPU_ADDR_3:  tx_data = cpu_req_addr_reg[31:24];
            STATE_CPU_WDATA_0: tx_data = cpu_req_wdata_reg[7:0];
            STATE_CPU_WDATA_1: tx_data = cpu_req_wdata_reg[15:8];
            STATE_CPU_WDATA_2: tx_data = cpu_req_wdata_reg[23:16];
            STATE_CPU_WDATA_3: tx_data = cpu_req_wdata_reg[31:24];
            
            // Host response packet (type 0011)
            STATE_HOST_HEADER: tx_data = {4'b0011, host_resp_size_reg, 1'b0, host_resp_we_reg};
            STATE_HOST_DATA_0: tx_data = host_resp_rdata_reg[7:0];
            STATE_HOST_DATA_1: tx_data = host_resp_rdata_reg[15:8];
            STATE_HOST_DATA_2: tx_data = host_resp_rdata_reg[23:16];
            STATE_HOST_DATA_3: tx_data = host_resp_rdata_reg[31:24];
            
            default: tx_data = 8'h00;
        endcase
    end
    
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
                // Priority: CPU requests > host responses
                if (cpu_req_valid) begin
                    next_state = STATE_CPU_HEADER;
                end else if (host_resp_valid) begin
                    next_state = STATE_HOST_HEADER;
                end
            end
            
            // CPU packet transmission
            STATE_CPU_HEADER: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_CPU_ADDR_0;
                end
            end
            
            STATE_CPU_ADDR_0: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_CPU_ADDR_1;
                end
            end
            
            STATE_CPU_ADDR_1: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_CPU_ADDR_2;
                end
            end
            
            STATE_CPU_ADDR_2: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_CPU_ADDR_3;
                end
            end
            
            STATE_CPU_ADDR_3: begin
                if (tx_valid && tx_ready) begin
                    if (cpu_req_we_reg) begin
                        // Write request - transmit data bytes
                        next_state = STATE_CPU_WDATA_0;
                    end else begin
                        // Read request - packet complete
                        next_state = STATE_IDLE;
                    end
                end
            end
            
            STATE_CPU_WDATA_0: begin
                if (tx_valid && tx_ready) begin
                    if (cpu_req_size_reg == 2'b00) begin
                        // Byte write - packet complete
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_CPU_WDATA_1;
                    end
                end
            end
            
            STATE_CPU_WDATA_1: begin
                if (tx_valid && tx_ready) begin
                    if (cpu_req_size_reg == 2'b01) begin
                        // Half-word write - packet complete
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_CPU_WDATA_2;
                    end
                end
            end
            
            STATE_CPU_WDATA_2: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_CPU_WDATA_3;
                end
            end
            
            STATE_CPU_WDATA_3: begin
                if (tx_valid && tx_ready) begin
                    // Word write - packet complete
                    next_state = STATE_IDLE;
                end
            end
            
            // Host response packet transmission
            STATE_HOST_HEADER: begin
                if (tx_valid && tx_ready) begin
                    if (host_resp_we_reg) begin
                        // Write response - header only (no data)
                        next_state = STATE_IDLE;
                    end else begin
                        // Read response - transmit data bytes
                        next_state = STATE_HOST_DATA_0;
                    end
                end
            end
            
            STATE_HOST_DATA_0: begin
                if (tx_valid && tx_ready) begin
                    if (host_resp_size_reg == 2'b00) begin
                        // Byte read response - packet complete
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_HOST_DATA_1;
                    end
                end
            end
            
            STATE_HOST_DATA_1: begin
                if (tx_valid && tx_ready) begin
                    if (host_resp_size_reg == 2'b01) begin
                        // Half-word read response - packet complete
                        next_state = STATE_IDLE;
                    end else begin
                        next_state = STATE_HOST_DATA_2;
                    end
                end
            end
            
            STATE_HOST_DATA_2: begin
                if (tx_valid && tx_ready) begin
                    next_state = STATE_HOST_DATA_3;
                end
            end
            
            STATE_HOST_DATA_3: begin
                if (tx_valid && tx_ready) begin
                    // Word read response - packet complete
                    next_state = STATE_IDLE;
                end
            end
            
            default: begin
                next_state = STATE_IDLE;
            end
        endcase
    end
    
    // ============================================================
    // Input Capture Logic
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cpu_req_addr_reg     <= 32'h0;
            cpu_req_wdata_reg    <= 32'h0;
            cpu_req_we_reg       <= 1'b0;
            cpu_req_size_reg     <= 2'b00;
            
            host_resp_rdata_reg  <= 32'h0;
            host_resp_we_reg     <= 1'b0;
            host_resp_size_reg   <= 2'b00;
        end else begin
            // Capture CPU request when accepted
            if (state == STATE_IDLE && cpu_req_valid) begin
                cpu_req_addr_reg  <= cpu_req_addr;
                cpu_req_wdata_reg <= cpu_req_wdata;
                cpu_req_we_reg    <= cpu_req_we;
                cpu_req_size_reg  <= cpu_req_size;
            end
            
            // Capture host response when accepted
            if (state == STATE_IDLE && !cpu_req_valid && host_resp_valid) begin
                host_resp_rdata_reg <= host_resp_rdata;
                host_resp_we_reg    <= host_resp_we;
                host_resp_size_reg  <= host_resp_size;
            end
        end
    end

endmodule

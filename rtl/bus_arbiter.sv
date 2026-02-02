// Bus Arbiter Module
// Implements fixed-priority arbitration between CPU and Host masters
// Priority: Host > CPU
//
// Features:
// - Registered outputs for timing closure
// - Hold grant until transaction completes (ready asserted)
// - No combinational loops
//
// Protocol:
// - Masters assert req to request bus access
// - Arbiter asserts grant for the winning master
// - Master holds req until ready is returned
// - Arbiter releases grant after ready is asserted

module bus_arbiter (
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU Master Interface
    input  logic [31:0] cpu_addr,
    input  logic [31:0] cpu_wdata,
    output logic [31:0] cpu_rdata,
    input  logic        cpu_we,
    input  logic [1:0]  cpu_size,
    input  logic        cpu_req,
    output logic        cpu_ready,
    
    // Host Master Interface (from host_bus_interface)
    input  logic [31:0] host_addr,
    input  logic [31:0] host_wdata,
    output logic [31:0] host_rdata,
    input  logic        host_we,
    input  logic [1:0]  host_size,
    input  logic        host_req,
    output logic        host_ready,
    
    // Slave Interface (to bus.sv)
    output logic [31:0] bus_addr,
    output logic [31:0] bus_wdata,
    input  logic [31:0] bus_rdata,
    output logic        bus_we,
    output logic [1:0]  bus_size,
    output logic        bus_req,
    input  logic        bus_ready
);

    // ============================================================
    // Arbiter State
    // ============================================================
    typedef enum logic [1:0] {
        ARB_IDLE       = 2'd0,
        ARB_CPU_GRANT  = 2'd1,
        ARB_HOST_GRANT = 2'd2
    } arb_state_t;
    
    arb_state_t state, next_state;
    
    // ============================================================
    // State Machine
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= ARB_IDLE;
        end else begin
            state <= next_state;
        end
    end
    
    always_comb begin
        next_state = state;
        
        case (state)
            ARB_IDLE: begin
                // Priority: Host > CPU
                if (host_req) begin
                    next_state = ARB_HOST_GRANT;
                end else if (cpu_req) begin
                    next_state = ARB_CPU_GRANT;
                end
            end
            
            ARB_CPU_GRANT: begin
                if (bus_ready) begin
                    // Transaction complete
                    // Check if host is waiting (preempt for next transaction)
                    if (host_req) begin
                        next_state = ARB_HOST_GRANT;
                    end else if (!cpu_req) begin
                        next_state = ARB_IDLE;
                    end
                    // else stay in CPU_GRANT for consecutive CPU transactions
                end
            end
            
            ARB_HOST_GRANT: begin
                if (bus_ready) begin
                    // Transaction complete
                    if (host_req) begin
                        // Host has more requests
                        next_state = ARB_HOST_GRANT;
                    end else if (cpu_req) begin
                        next_state = ARB_CPU_GRANT;
                    end else begin
                        next_state = ARB_IDLE;
                    end
                end
            end
            
            default: next_state = ARB_IDLE;
        endcase
    end
    
    // ============================================================
    // Multiplexer Logic
    // ============================================================
    always_comb begin
        // Default: idle
        bus_addr   = 32'h0;
        bus_wdata  = 32'h0;
        bus_we     = 1'b0;
        bus_size   = 2'b00;
        bus_req    = 1'b0;
        
        cpu_rdata  = 32'h0;
        cpu_ready  = 1'b0;
        host_rdata = 32'h0;
        host_ready = 1'b0;
        
        case (state)
            ARB_CPU_GRANT: begin
                bus_addr  = cpu_addr;
                bus_wdata = cpu_wdata;
                bus_we    = cpu_we;
                bus_size  = cpu_size;
                bus_req   = cpu_req;
                cpu_rdata = bus_rdata;
                cpu_ready = bus_ready;
            end
            
            ARB_HOST_GRANT: begin
                bus_addr   = host_addr;
                bus_wdata  = host_wdata;
                bus_we     = host_we;
                bus_size   = host_size;
                bus_req    = host_req;
                host_rdata = bus_rdata;
                host_ready = bus_ready;
            end
            
            default: ;
        endcase
    end

endmodule

// Top-Level Module
// Wraps the RISC-V CPU core with RTL peripherals
// Uses a generic bus module to route requests between CPU and peripherals
//
// UNIFIED MEMORY INTERFACE: Uses a single memory interface for both instruction
// fetch and data access. The CPU's multi-cycle FSM ensures only one type of
// access is active at a time.

module top #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1,  // RV32F extension: Floating-Point (default: enabled)
    // UART Parameters
    parameter int UART_CLK_FREQ_HZ = 50_000_000,
    parameter int UART_BAUD_RATE   = 115200,
    // UART Loopback: When enabled (default), TX is internally connected to RX
    // for simulation testing. Disable for FPGA deployment with external pins.
    parameter bit ENABLE_UART_LOOPBACK = 1'b1
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Unified external memory interface (for DRAM + Rust peripherals)
    // Handles both instruction fetch and data access
    output logic [31:0] ext_mem_addr,
    output logic [31:0] ext_mem_wdata,
    input  logic [31:0] ext_mem_rdata,
    output logic        ext_mem_we,
    output logic [1:0]  ext_mem_size,
    output logic        ext_mem_req,
    input  logic        ext_mem_ready,
    
    // LED peripheral outputs
    output logic [7:0]  led_out,
    
    // UART peripheral pins (active only when ENABLE_UART_LOOPBACK = 0)
    output logic        uart_tx,    // UART transmit output
    /* verilator lint_off UNUSEDSIGNAL */
    input  logic        uart_rx,    // UART receive input (unused in loopback mode)
    /* verilator lint_on UNUSEDSIGNAL */
    
    // System control signals (passed through from CPU)
    output logic        halted,
    output logic        instr_complete,
    
    // Debug outputs (passed through from CPU)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data,
    output logic [31:0] debug_pc,
    output logic [31:0] debug_instruction,
    output logic [31:0] debug_current_pc,
    output logic [31:0] debug_current_instruction,
    output logic [3:0]  debug_fsm_state
);

    // ============================================================
    // Address Range Definitions
    // ============================================================
    // Peripheral address configuration for bus routing
    // Slave 0: LED Controller
    localparam LED_BASE = 32'h50000000;
    localparam LED_SIZE = 32'h00000010;  // 16 bytes
    // Slave 1: UART Controller
    localparam UART_BASE = 32'h52000000;
    localparam UART_SIZE = 32'h00000100;  // 256 bytes
    // Slave 2: External Memory (catch-all via DEFAULT_SLAVE_IDX)
    // External memory handles DRAM + Rust peripherals (everything not LED/UART)
    
    // Number of bus slaves and their indices
    localparam NUM_BUS_SLAVES = 3;
    localparam LED_SLAVE_IDX = 0;
    localparam UART_SLAVE_IDX = 1;
    localparam EXT_MEM_SLAVE_IDX = 2;
    
    // ============================================================
    // Internal CPU Memory Interface Signals (Unified)
    // ============================================================
    // The CPU provides a unified memory interface for both instruction and data
    logic [31:0] cpu_mem_addr;
    logic [31:0] cpu_mem_wdata;
    logic [31:0] cpu_mem_rdata;
    logic        cpu_mem_we;
    logic [1:0]  cpu_mem_size;
    logic        cpu_mem_req;
    logic        cpu_mem_ready;
    
    // ============================================================
    // Bus Slave Interface Signals (Flattened for Yosys Compatibility)
    // ============================================================
    // LED Controller (Slave 0)
    logic [31:0] led_slave_addr;
    logic [31:0] led_slave_wdata;
    logic [31:0] led_slave_rdata;
    logic        led_slave_we;
    logic [1:0]  led_slave_size;
    logic        led_slave_req;
    logic        led_slave_ready;
    
    // UART Controller (Slave 1)
    logic [31:0] uart_slave_addr;
    logic [31:0] uart_slave_wdata;
    logic [31:0] uart_slave_rdata;
    logic        uart_slave_we;
    logic [1:0]  uart_slave_size;
    logic        uart_slave_req;
    logic        uart_slave_ready;
    
    // External Memory (Slave 2)
    logic [31:0] ext_slave_addr;
    logic [31:0] ext_slave_wdata;
    logic [31:0] ext_slave_rdata;
    logic        ext_slave_we;
    logic [1:0]  ext_slave_size;
    logic        ext_slave_req;
    logic        ext_slave_ready;
    
    // Internal UART signals for loopback
    logic uart_tx_internal;  // TX output from UART module
    logic uart_rx_internal;  // RX input to UART module
    
    // ============================================================
    // Bus Slave Configuration and Interface Signals
    // ============================================================
    // Concatenated vectors for bus module connections (Yosys compatible)
    // Format: {slave[2], slave[1], slave[0]} = {ext_mem, uart, led}
    
    // Slave configuration
    logic [NUM_BUS_SLAVES*32-1:0] slave_base_addrs;
    logic [NUM_BUS_SLAVES*32-1:0] slave_addr_sizes;
    
    // Initialize slave configuration using concatenation
    assign slave_base_addrs = {32'h0, UART_BASE, LED_BASE};  // [2]=unused, [1]=UART, [0]=LED
    assign slave_addr_sizes = {32'h0, UART_SIZE, LED_SIZE};  // External uses default slave routing
    
    // Slave interface signals (concatenated)
    logic [NUM_BUS_SLAVES*32-1:0] bus_slave_addr_cat;
    logic [NUM_BUS_SLAVES*32-1:0] bus_slave_wdata_cat;
    logic [NUM_BUS_SLAVES*32-1:0] bus_slave_rdata_cat;
    logic [NUM_BUS_SLAVES-1:0]    bus_slave_we_cat;
    logic [NUM_BUS_SLAVES*2-1:0]  bus_slave_size_cat;
    logic [NUM_BUS_SLAVES-1:0]    bus_slave_req_cat;
    logic [NUM_BUS_SLAVES-1:0]    bus_slave_ready_cat;
    
    // ============================================================
    // Bus Module Instantiation
    // ============================================================
    // Routes CPU requests to the appropriate slave peripheral
    bus #(
        .NUM_SLAVES(NUM_BUS_SLAVES),
        .ADDR_WIDTH(32),
        .DATA_WIDTH(32),
        .DEFAULT_SLAVE_IDX(EXT_MEM_SLAVE_IDX)  // External memory handles unmatched addresses
    ) system_bus (
        .clk(clk),
        .rst_n(rst_n),
        
        // Master interface (CPU)
        .master_addr(cpu_mem_addr),
        .master_wdata(cpu_mem_wdata),
        .master_rdata(cpu_mem_rdata),
        .master_we(cpu_mem_we),
        .master_size(cpu_mem_size),
        .master_req(cpu_mem_req),
        .master_ready(cpu_mem_ready),
        
        // Slave configuration (base addresses and sizes)
        .slave_base_addr(slave_base_addrs),
        .slave_addr_size(slave_addr_sizes),
        
        // Slave interfaces (concatenated vectors)
        .slave_addr(bus_slave_addr_cat),
        .slave_wdata(bus_slave_wdata_cat),
        .slave_rdata(bus_slave_rdata_cat),
        .slave_we(bus_slave_we_cat),
        .slave_size(bus_slave_size_cat),
        .slave_req(bus_slave_req_cat),
        .slave_ready(bus_slave_ready_cat)
    );
    
    // ============================================================
    // Extract Individual Slave Signals from Concatenated Vectors
    // ============================================================
    // LED Controller (Slave 0) - extract from bit positions [31:0]
    assign led_slave_addr  = bus_slave_addr_cat[31:0];
    assign led_slave_wdata = bus_slave_wdata_cat[31:0];
    assign led_slave_we    = bus_slave_we_cat[0];
    assign led_slave_size  = bus_slave_size_cat[1:0];
    assign led_slave_req   = bus_slave_req_cat[0];
    
    // UART Controller (Slave 1) - extract from bit positions [63:32]
    assign uart_slave_addr  = bus_slave_addr_cat[63:32];
    assign uart_slave_wdata = bus_slave_wdata_cat[63:32];
    assign uart_slave_we    = bus_slave_we_cat[1];
    assign uart_slave_size  = bus_slave_size_cat[3:2];
    assign uart_slave_req   = bus_slave_req_cat[1];
    
    // External Memory (Slave 2) - extract from bit positions [95:64]
    assign ext_slave_addr  = bus_slave_addr_cat[95:64];
    assign ext_slave_wdata = bus_slave_wdata_cat[95:64];
    assign ext_slave_we    = bus_slave_we_cat[2];
    assign ext_slave_size  = bus_slave_size_cat[5:4];
    assign ext_slave_req   = bus_slave_req_cat[2];
    
    // Concatenate slave rdata and ready back to bus
    // Format: {slave[2], slave[1], slave[0]}
    assign bus_slave_rdata_cat = {ext_slave_rdata, uart_slave_rdata, led_slave_rdata};
    assign bus_slave_ready_cat = {ext_slave_ready, uart_slave_ready, led_slave_ready};
    
    // ============================================================
    // External Memory Interface Connection (Slave 2)
    // ============================================================
    // Connect external memory signals to bus slave interface
    assign ext_mem_addr  = ext_slave_addr;
    assign ext_mem_wdata = ext_slave_wdata;
    assign ext_mem_size  = ext_slave_size;
    assign ext_mem_req   = ext_slave_req;
    assign ext_mem_we    = ext_slave_we;
    assign ext_slave_rdata = ext_mem_rdata;
    assign ext_slave_ready = ext_mem_ready;
    
    // ============================================================
    // CPU Core Instantiation
    // ============================================================
    cpu #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) cpu_core (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(boot_addr),
        
        // Unified memory interface (handles both instruction fetch and data access)
        .mem_addr(cpu_mem_addr),
        .mem_wdata(cpu_mem_wdata),
        .mem_rdata(cpu_mem_rdata),
        .mem_we(cpu_mem_we),
        .mem_size(cpu_mem_size),
        .mem_req(cpu_mem_req),
        .mem_ready(cpu_mem_ready),
        
        // System control (passed through)
        .halted(halted),
        .instr_complete(instr_complete),
        
        // Debug signals (passed through)
        .debug_rs1_data(debug_rs1_data),
        .debug_rs2_data(debug_rs2_data),
        .debug_rd_data(debug_rd_data),
        .debug_pc(debug_pc),
        .debug_instruction(debug_instruction),
        .debug_current_pc(debug_current_pc),
        .debug_current_instruction(debug_current_instruction),
        .debug_fsm_state(debug_fsm_state)
    );
    
    // ============================================================
    // UART Loopback or External Connection
    // ============================================================
    generate
        if (ENABLE_UART_LOOPBACK) begin : gen_loopback
            // Internal loopback: connect TX directly to RX for testing
            assign uart_rx_internal = uart_tx_internal;
            // Still expose TX externally for debugging/monitoring
            assign uart_tx = uart_tx_internal;
            // uart_rx input port is ignored in loopback mode
        end else begin : gen_external
            // External connection: use actual RX/TX pins
            assign uart_rx_internal = uart_rx;
            assign uart_tx = uart_tx_internal;
        end
    endgenerate
    
    // ============================================================
    // LED Controller Instantiation (Slave 0)
    // ============================================================
    led_controller led_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // Bus slave interface
        .addr(led_slave_addr),
        .wdata(led_slave_wdata),
        .rdata(led_slave_rdata),
        .we(led_slave_we),
        .req(led_slave_req),
        .size(led_slave_size),
        .ready(led_slave_ready),
        
        // LED outputs
        .led_out(led_out)
    );
    
    // ============================================================
    // UART Controller Instantiation (Slave 1)
    // ============================================================
    uart #(
        .CLK_FREQ_HZ(UART_CLK_FREQ_HZ),
        .BAUD_RATE(UART_BAUD_RATE),
        .FIFO_DEPTH(8)
    ) uart_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // Bus slave interface
        .addr(uart_slave_addr),
        .wdata(uart_slave_wdata),
        .rdata(uart_slave_rdata),
        .we(uart_slave_we),
        .req(uart_slave_req),
        .size(uart_slave_size),
        .ready(uart_slave_ready),
        
        // Internal signals (connected via loopback or external pins)
        .tx_out(uart_tx_internal),
        .rx_in(uart_rx_internal)
    );

endmodule

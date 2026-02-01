// Top-Level Module
// Wraps the RISC-V CPU core with RTL peripherals
// Uses the bus module to route requests between CPU and peripherals
// External memory requests are serialized via host_bus_interface for host communication
//
// UNIFIED MEMORY INTERFACE: Uses a single memory interface for both instruction
// fetch and data access. The CPU's multi-cycle FSM ensures only one type of
// access is active at a time.
//
// HOST INTERFACE: External memory requests are serialized to an 8-bit byte stream
// via the host_bus_interface module for communication with a host (simulation or FPGA).

module top #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1,  // RV32F extension: Floating-Point (default: enabled)
    parameter bit USE_BRAM_REGFILE = 1'b0,  // Use BRAM-based register file (default: LUT-based)
    // System Clock Frequency (used by UART and Clock Peripheral)
    parameter int CLK_FREQ_HZ = 50_000_000,
    // UART Parameters
    parameter int UART_BAUD_RATE   = 115200,
    // UART Loopback: When enabled (default), TX is internally connected to RX
    // for simulation testing. Disable for FPGA deployment with external pins.
    parameter bit ENABLE_UART_LOOPBACK = 1'b1
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Host TX Interface (to External Host)
    // Serialized bus transactions sent to host
    output logic [7:0]  host_tx_data,
    output logic        host_tx_valid,
    input  logic        host_tx_ready,
    
    // Host RX Interface (from External Host)
    // Serialized bus transaction responses from host
    input  logic [7:0]  host_rx_data,
    input  logic        host_rx_valid,
    output logic        host_rx_ready,
    
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
    // Internal CPU Memory Interface Signals (Unified)
    // ============================================================
    logic [31:0] cpu_mem_addr;
    logic [31:0] cpu_mem_wdata;
    logic [31:0] cpu_mem_rdata;
    logic        cpu_mem_we;
    logic [1:0]  cpu_mem_size;
    logic        cpu_mem_req;
    logic        cpu_mem_ready;
    
    // ============================================================
    // LED Controller Interface Signals
    // ============================================================
    logic [31:0] led_addr;
    logic [31:0] led_wdata;
    logic [31:0] led_rdata;
    logic        led_we;
    logic [1:0]  led_size;
    logic        led_req;
    logic        led_ready;
    
    // ============================================================
    // Clock Peripheral Interface Signals
    // ============================================================
    logic [31:0] clock_addr;
    logic [31:0] clock_wdata;
    logic [31:0] clock_rdata;
    logic        clock_we;
    logic [1:0]  clock_size;
    logic        clock_req;
    logic        clock_ready;
    
    // ============================================================
    // UART Controller Interface Signals
    // ============================================================
    logic [31:0] uart_addr;
    logic [31:0] uart_wdata;
    logic [31:0] uart_rdata;
    logic        uart_we;
    logic [1:0]  uart_size;
    logic        uart_req;
    logic        uart_ready;
    
    // Internal UART signals for loopback
    logic uart_tx_internal;  // TX output from UART module
    logic uart_rx_internal;  // RX input to UART module
    
    // ============================================================
    // External Memory Interface Signals (Bus to Host Bus Interface)
    // ============================================================
    logic [31:0] ext_mem_addr;
    logic [31:0] ext_mem_wdata;
    logic [31:0] ext_mem_rdata;
    logic        ext_mem_we;
    logic [1:0]  ext_mem_size;
    logic        ext_mem_req;
    logic        ext_mem_ready;
    
    // ============================================================
    // Bus Module Instantiation
    // ============================================================
    // Routes CPU requests to the appropriate peripheral based on address
    bus system_bus (
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
        
        // LED Controller interface
        .led_addr(led_addr),
        .led_wdata(led_wdata),
        .led_rdata(led_rdata),
        .led_we(led_we),
        .led_size(led_size),
        .led_req(led_req),
        .led_ready(led_ready),
        
        // Clock Peripheral interface
        .clock_addr(clock_addr),
        .clock_wdata(clock_wdata),
        .clock_rdata(clock_rdata),
        .clock_we(clock_we),
        .clock_size(clock_size),
        .clock_req(clock_req),
        .clock_ready(clock_ready),
        
        // UART Controller interface
        .uart_addr(uart_addr),
        .uart_wdata(uart_wdata),
        .uart_rdata(uart_rdata),
        .uart_we(uart_we),
        .uart_size(uart_size),
        .uart_req(uart_req),
        .uart_ready(uart_ready),
        
        // External Memory interface (routed to host_bus_interface)
        .ext_mem_addr(ext_mem_addr),
        .ext_mem_wdata(ext_mem_wdata),
        .ext_mem_rdata(ext_mem_rdata),
        .ext_mem_we(ext_mem_we),
        .ext_mem_size(ext_mem_size),
        .ext_mem_req(ext_mem_req),
        .ext_mem_ready(ext_mem_ready)
    );
    
    // ============================================================
    // Host Bus Interface Instantiation
    // ============================================================
    // Serializes external memory transactions to byte stream for host communication
    host_bus_interface host_bus_if (
        .clk(clk),
        .rst_n(rst_n),
        
        // Bus Slave Interface (from System Bus)
        .addr(ext_mem_addr),
        .wdata(ext_mem_wdata),
        .rdata(ext_mem_rdata),
        .we(ext_mem_we),
        .size(ext_mem_size),
        .req(ext_mem_req),
        .ready(ext_mem_ready),
        
        // Host TX Interface (to External Host)
        .tx_data(host_tx_data),
        .tx_valid(host_tx_valid),
        .tx_ready(host_tx_ready),
        
        // Host RX Interface (from External Host)
        .rx_data(host_rx_data),
        .rx_valid(host_rx_valid),
        .rx_ready(host_rx_ready)
    );
    
    // ============================================================
    // CPU Core Instantiation
    // ============================================================
    cpu #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .USE_BRAM_REGFILE(USE_BRAM_REGFILE)
    ) cpu_core (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(boot_addr),
        
        // Unified memory interface
        .mem_addr(cpu_mem_addr),
        .mem_wdata(cpu_mem_wdata),
        .mem_rdata(cpu_mem_rdata),
        .mem_we(cpu_mem_we),
        .mem_size(cpu_mem_size),
        .mem_req(cpu_mem_req),
        .mem_ready(cpu_mem_ready),
        
        // System control
        .halted(halted),
        .instr_complete(instr_complete),
        
        // Debug signals
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
            assign uart_rx_internal = uart_tx_internal;
            assign uart_tx = uart_tx_internal;
        end else begin : gen_external
            assign uart_rx_internal = uart_rx;
            assign uart_tx = uart_tx_internal;
        end
    endgenerate
    
    // ============================================================
    // LED Controller Instantiation
    // ============================================================
    led_controller_peripheral led_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        .addr(led_addr),
        .wdata(led_wdata),
        .rdata(led_rdata),
        .we(led_we),
        .req(led_req),
        .size(led_size),
        .ready(led_ready),
        
        .led_out(led_out)
    );
    
    // ============================================================
    // Clock Peripheral Instantiation
    // ============================================================
    clock_peripheral #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ)
    ) clock_periph (
        .clk(clk),
        .rst_n(rst_n),
        
        .addr(clock_addr),
        .wdata(clock_wdata),
        .rdata(clock_rdata),
        .we(clock_we),
        .req(clock_req),
        .size(clock_size),
        .ready(clock_ready)
    );
    
    // ============================================================
    // UART Controller Instantiation
    // ============================================================
    uart_peripheral #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .BAUD_RATE(UART_BAUD_RATE),
        .FIFO_DEPTH(8)
    ) uart_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // Bus slave interface
        .addr(uart_addr),
        .wdata(uart_wdata),
        .rdata(uart_rdata),
        .we(uart_we),
        .req(uart_req),
        .size(uart_size),
        .ready(uart_ready),
        
        // Internal signals (connected via loopback or external pins)
        .tx_out(uart_tx_internal),
        .rx_in(uart_rx_internal)
    );

endmodule

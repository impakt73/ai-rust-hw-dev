// Top-Level Module
// Wraps the RISC-V CPU core with RTL peripherals
// Routes RTL peripheral addresses internally, forwards others to external bus
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
    output logic        ext_mem_re,
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
    localparam RTL_PERIPH_BASE  = 32'h50000000;
    localparam RTL_PERIPH_LIMIT = 32'h60000000;
    localparam LED_BASE         = 32'h50000000;
    localparam LED_LIMIT        = 32'h50000010;  // 16 bytes
    localparam UART_BASE        = 32'h52000000;
    localparam UART_LIMIT       = 32'h52000100;  // 256 bytes
    
    // ============================================================
    // Internal CPU Memory Interface Signals (Unified)
    // ============================================================
    // The CPU provides a unified memory interface for both instruction and data
    logic [31:0] cpu_mem_addr;
    logic [31:0] cpu_mem_wdata;
    logic [31:0] cpu_mem_rdata;
    logic        cpu_mem_we;
    logic        cpu_mem_re;
    logic [1:0]  cpu_mem_size;
    logic        cpu_mem_req;
    logic        cpu_mem_ready;
    
    // ============================================================
    // LED Controller Interface Signals
    // ============================================================
    logic [31:0] led_rdata;
    logic        led_ready;
    
    // ============================================================
    // UART Controller Interface Signals
    // ============================================================
    logic [31:0] uart_rdata;
    logic        uart_ready;
    
    // Internal UART signals for loopback
    logic uart_tx_internal;  // TX output from UART module
    logic uart_rx_internal;  // RX input to UART module
    
    // ============================================================
    // Address Decoder
    // ============================================================
    // Decodes CPU memory address to select appropriate destination:
    // - RTL peripherals (LED, UART) are handled internally
    // - All other addresses go to external bus (DRAM, Rust peripherals)
    // Note: Both instruction fetch and data access use this decoder.
    // Instruction fetches typically target DRAM, data accesses may target
    // either DRAM or peripherals.
    logic sel_led;
    logic sel_uart;
    logic sel_external;
    logic sel_unmapped_rtl;
    
    always_comb begin
        sel_led          = 1'b0;
        sel_uart         = 1'b0;
        sel_unmapped_rtl = 1'b0;
        sel_external     = 1'b0;
        
        // Check if address is in LED range
        if (cpu_mem_addr >= LED_BASE && cpu_mem_addr < LED_LIMIT) begin
            sel_led = 1'b1;
        end
        // Check if address is in UART range
        else if (cpu_mem_addr >= UART_BASE && cpu_mem_addr < UART_LIMIT) begin
            sel_uart = 1'b1;
        end
        // Check if address is in unmapped RTL peripheral space
        else if (cpu_mem_addr >= RTL_PERIPH_BASE && cpu_mem_addr < RTL_PERIPH_LIMIT) begin
            sel_unmapped_rtl = 1'b1;
        end
        // Otherwise route to external bus (Rust peripherals + DRAM)
        else begin
            sel_external = 1'b1;
        end
    end
    
    // ============================================================
    // Response Multiplexer
    // ============================================================
    always_comb begin
        // Default values
        cpu_mem_rdata = 32'h0;
        cpu_mem_ready = 1'b0;
        
        // Select response source
        if (sel_led) begin
            cpu_mem_rdata = led_rdata;
            cpu_mem_ready = led_ready;
        end else if (sel_uart) begin
            cpu_mem_rdata = uart_rdata;
            cpu_mem_ready = uart_ready;
        end else if (sel_unmapped_rtl) begin
            // Unmapped RTL peripheral address - return zero and ready immediately
            cpu_mem_rdata = 32'h0;
            cpu_mem_ready = 1'b1;
            // Note: In simulation, this triggers a warning via $display in tests
        end else if (sel_external) begin
            cpu_mem_rdata = ext_mem_rdata;
            cpu_mem_ready = ext_mem_ready;
        end else begin
            // Should never reach here if decoder logic is correct
            cpu_mem_rdata = 32'h0;
            cpu_mem_ready = 1'b1;
        end
    end
    
    // ============================================================
    // External Bus Forwarding
    // ============================================================
    // Forward CPU requests to external bus (for Rust peripherals + DRAM)
    assign ext_mem_addr  = cpu_mem_addr;
    assign ext_mem_wdata = cpu_mem_wdata;
    assign ext_mem_size  = cpu_mem_size;
    
    // Only assert request/enable if address is external
    assign ext_mem_req = cpu_mem_req && sel_external;
    assign ext_mem_we  = cpu_mem_we  && sel_external;
    assign ext_mem_re  = cpu_mem_re  && sel_external;
    
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
        .mem_re(cpu_mem_re),
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
    // LED Controller Instantiation
    // ============================================================
    led_controller led_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // CPU interface (unified memory)
        .addr(cpu_mem_addr),
        .wdata(cpu_mem_wdata),
        .rdata(led_rdata),
        .we(cpu_mem_we && sel_led),
        .re(cpu_mem_re && sel_led),
        .size(cpu_mem_size),
        .ready(led_ready),
        
        // LED outputs
        .led_out(led_out)
    );
    
    // ============================================================
    // UART Controller Instantiation
    // ============================================================
    uart #(
        .CLK_FREQ_HZ(UART_CLK_FREQ_HZ),
        .BAUD_RATE(UART_BAUD_RATE),
        .FIFO_DEPTH(8)
    ) uart_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // CPU interface (unified memory)
        .addr(cpu_mem_addr),
        .wdata(cpu_mem_wdata),
        .rdata(uart_rdata),
        .we(cpu_mem_we && sel_uart),
        .re(cpu_mem_re && sel_uart),
        .size(cpu_mem_size),
        .ready(uart_ready),
        
        // Internal signals (connected via loopback or external pins)
        .tx_out(uart_tx_internal),
        .rx_in(uart_rx_internal)
    );

endmodule

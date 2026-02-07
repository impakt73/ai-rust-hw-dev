// System Bus Module
// Routes memory requests from the CPU to peripheral slaves based on address.
// All address mapping logic is contained within this module.
//
// Address Map:
// - LED Controller:      0x50000000 - 0x5000000F (16 bytes)
// - Clock Peripheral:    0x51000000 - 0x5100000F (16 bytes)
// - UART Controller:     0x52000000 - 0x520000FF (256 bytes)
// - System Controller:   0x53000000 - 0x5300000F (16 bytes)
// - External Memory:     Everything else (DRAM + Rust peripherals)
//
// Unmapped addresses (within RTL peripheral range but not LED/CLOCK/UART/SYSCTRL):
// - Reads return 0
// - Writes are dropped
// - Ready is asserted immediately

module bus (
    // Clock and reset (unused in current combinational implementation)
    /* verilator lint_off UNUSED */
    input  logic        clk,
    input  logic        rst_n,
    /* verilator lint_on UNUSED */
    
    // Master interface (from CPU)
    input  logic [31:0] master_addr,
    input  logic [31:0] master_wdata,
    output logic [31:0] master_rdata,
    input  logic        master_we,
    input  logic [1:0]  master_size,
    input  logic        master_req,
    output logic        master_ready,
    
    // LED Controller interface
    output logic [31:0] led_addr,
    output logic [31:0] led_wdata,
    input  logic [31:0] led_rdata,
    output logic        led_we,
    output logic [1:0]  led_size,
    output logic        led_req,
    input  logic        led_ready,
    
    // Clock Peripheral interface
    output logic [31:0] clock_addr,
    output logic [31:0] clock_wdata,
    input  logic [31:0] clock_rdata,
    output logic        clock_we,
    output logic [1:0]  clock_size,
    output logic        clock_req,
    input  logic        clock_ready,
    
    // UART Controller interface
    output logic [31:0] uart_addr,
    output logic [31:0] uart_wdata,
    input  logic [31:0] uart_rdata,
    output logic        uart_we,
    output logic [1:0]  uart_size,
    output logic        uart_req,
    input  logic        uart_ready,
    
    // System Controller interface
    output logic [31:0] sysctrl_addr,
    output logic [31:0] sysctrl_wdata,
    input  logic [31:0] sysctrl_rdata,
    output logic        sysctrl_we,
    output logic [1:0]  sysctrl_size,
    output logic        sysctrl_req,
    input  logic        sysctrl_ready,
    
    // External Memory interface (DRAM + Rust peripherals)
    output logic [31:0] ext_mem_addr,
    output logic [31:0] ext_mem_wdata,
    input  logic [31:0] ext_mem_rdata,
    output logic        ext_mem_we,
    output logic [1:0]  ext_mem_size,
    output logic        ext_mem_req,
    input  logic        ext_mem_ready
);

    // ============================================================
    // Address Range Definitions
    // ============================================================
    localparam LED_BASE   = 32'h50000000;
    localparam LED_LIMIT  = 32'h50000010;  // LED_BASE + 16 bytes
    localparam CLOCK_BASE = 32'h51000000;
    localparam CLOCK_LIMIT = 32'h51000010; // CLOCK_BASE + 16 bytes
    localparam UART_BASE  = 32'h52000000;
    localparam UART_LIMIT = 32'h52000100;  // UART_BASE + 256 bytes
    localparam SYSCTRL_BASE  = 32'h53000000;
    localparam SYSCTRL_LIMIT = 32'h53000010; // SYSCTRL_BASE + 16 bytes
    // RTL peripheral range (for detecting unmapped RTL addresses)
    localparam RTL_PERIPH_BASE  = 32'h50000000;
    localparam RTL_PERIPH_LIMIT = 32'h60000000;

    // ============================================================
    // Address Decoder
    // ============================================================
    logic sel_led;
    logic sel_clock;
    logic sel_uart;
    logic sel_sysctrl;
    logic sel_ext_mem;
    
    always_comb begin
        sel_led      = 1'b0;
        sel_clock    = 1'b0;
        sel_uart     = 1'b0;
        sel_sysctrl  = 1'b0;
        sel_ext_mem  = 1'b0;
        
        // Check if address is in LED range
        if (master_addr >= LED_BASE && master_addr < LED_LIMIT) begin
            sel_led = 1'b1;
        end
        // Check if address is in Clock Peripheral range
        else if (master_addr >= CLOCK_BASE && master_addr < CLOCK_LIMIT) begin
            sel_clock = 1'b1;
        end
        // Check if address is in UART range
        else if (master_addr >= UART_BASE && master_addr < UART_LIMIT) begin
            sel_uart = 1'b1;
        end
        // Check if address is in System Controller range
        else if (master_addr >= SYSCTRL_BASE && master_addr < SYSCTRL_LIMIT) begin
            sel_sysctrl = 1'b1;
        end
        // Check if address is in unmapped RTL peripheral space
        // (unmapped: no select asserted, uses default response)
        else if (master_addr >= RTL_PERIPH_BASE && master_addr < RTL_PERIPH_LIMIT) begin
            // Unmapped RTL peripheral address - no slave selected
            // Response mux defaults to ready=1, rdata=0
        end
        // Otherwise route to external memory (DRAM + Rust peripherals)
        else begin
            sel_ext_mem = 1'b1;
        end
    end
    
    // ============================================================
    // Request Routing to Slaves
    // ============================================================
    // Address, data, and size are broadcast to all slaves
    assign led_addr      = master_addr;
    assign led_wdata     = master_wdata;
    assign led_size      = master_size;
    
    assign clock_addr    = master_addr;
    assign clock_wdata   = master_wdata;
    assign clock_size    = master_size;
    
    assign uart_addr     = master_addr;
    assign uart_wdata    = master_wdata;
    assign uart_size     = master_size;
    
    assign sysctrl_addr  = master_addr;
    assign sysctrl_wdata = master_wdata;
    assign sysctrl_size  = master_size;
    
    assign ext_mem_addr  = master_addr;
    assign ext_mem_wdata = master_wdata;
    assign ext_mem_size  = master_size;
    
    // Request and write enable are only asserted for the selected slave
    // Unmapped addresses: writes are dropped (no req/we asserted)
    assign led_req      = master_req && sel_led;
    assign led_we       = master_we  && sel_led;
    
    assign clock_req    = master_req && sel_clock;
    assign clock_we     = master_we  && sel_clock;
    
    assign uart_req     = master_req && sel_uart;
    assign uart_we      = master_we  && sel_uart;
    
    assign sysctrl_req  = master_req && sel_sysctrl;
    assign sysctrl_we   = master_we  && sel_sysctrl;
    
    assign ext_mem_req  = master_req && sel_ext_mem;
    assign ext_mem_we   = master_we  && sel_ext_mem;
    
    // ============================================================
    // Response Multiplexer
    // ============================================================
    always_comb begin
        // Default: unmapped address - return zero and assert ready
        master_rdata = 32'h0;
        master_ready = 1'b1;
        
        if (sel_led) begin
            master_rdata = led_rdata;
            master_ready = led_ready;
        end else if (sel_clock) begin
            master_rdata = clock_rdata;
            master_ready = clock_ready;
        end else if (sel_uart) begin
            master_rdata = uart_rdata;
            master_ready = uart_ready;
        end else if (sel_sysctrl) begin
            master_rdata = sysctrl_rdata;
            master_ready = sysctrl_ready;
        end else if (sel_ext_mem) begin
            master_rdata = ext_mem_rdata;
            master_ready = ext_mem_ready;
        end
        // Unmapped addresses: default values apply (return 0, ready = 1)
    end

endmodule

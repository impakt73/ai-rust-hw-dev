// System Bus Module
// Routes memory requests from the CPU to peripheral slaves based on address.
// Decodes only RTL peripheral addresses. Top-level host_bus_mux handles
// external-vs-RTL range split before requests reach this module.
//
// Address Map:
// - LED Controller:      0x50000000 - 0x5000000F (16 bytes)
// - Clock Peripheral:    0x60000000 - 0x6000000F (16 bytes)
// - SRAM Peripheral:     0x70000000 - 0x70002FFF (12KB)
// - System Controller:   0x20000000 - 0x2000000F (16 bytes)
// External memory (DRAM + Rust peripherals) is routed outside this module.
//
// Unmapped addresses (within selected RTL peripheral nibble windows but outside
// each peripheral's low-order decode window):
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
    
    // SRAM Peripheral interface
    output logic [31:0] sram_addr,
    output logic [31:0] sram_wdata,
    input  logic [31:0] sram_rdata,
    output logic        sram_we,
    output logic [1:0]  sram_size,
    output logic        sram_req,
    input  logic        sram_ready,
    
    // System Controller interface
    output logic [31:0] sysctrl_addr,
    output logic [31:0] sysctrl_wdata,
    input  logic [31:0] sysctrl_rdata,
    output logic        sysctrl_we,
    output logic [1:0]  sysctrl_size,
    output logic        sysctrl_req,
    input  logic        sysctrl_ready
);

    // ============================================================
    // Address Range Definitions
    // ============================================================
    localparam logic [3:0] SYSCTRL_TOP_NIBBLE = 4'h2;
    localparam logic [3:0] LED_TOP_NIBBLE     = 4'h5;
    localparam logic [3:0] CLOCK_TOP_NIBBLE   = 4'h6;
    localparam logic [3:0] SRAM_TOP_NIBBLE    = 4'h7;

    localparam logic [27:0] SMALL_PERIPH_WINDOW_SIZE = 28'h0000010; // 16B
    localparam logic [27:0] LED_WINDOW_SIZE     = SMALL_PERIPH_WINDOW_SIZE;
    localparam logic [27:0] CLOCK_WINDOW_SIZE   = SMALL_PERIPH_WINDOW_SIZE;
    localparam logic [27:0] SRAM_WINDOW_SIZE    = 28'h0003000; // 12KB
    localparam logic [27:0] SYSCTRL_WINDOW_SIZE = 28'h0000010; // 16B
    // ============================================================
    // Address Decoder
    // ============================================================
    logic sel_led;
    logic sel_clock;
    logic sel_sram;
    logic sel_sysctrl;
    
    always_comb begin
        sel_led      = 1'b0;
        sel_clock    = 1'b0;
        sel_sram     = 1'b0;
        sel_sysctrl  = 1'b0;
        
        // Select peripheral by top nibble, then gate with low-window range.
        if (master_addr[31:28] == LED_TOP_NIBBLE && master_addr[27:0] < LED_WINDOW_SIZE) begin
            sel_led = 1'b1;
        end
        else if (master_addr[31:28] == CLOCK_TOP_NIBBLE && master_addr[27:0] < CLOCK_WINDOW_SIZE) begin
            sel_clock = 1'b1;
        end
        else if (master_addr[31:28] == SRAM_TOP_NIBBLE && master_addr[27:0] < SRAM_WINDOW_SIZE) begin
            sel_sram = 1'b1;
        end
        else if (master_addr[31:28] == SYSCTRL_TOP_NIBBLE && master_addr[27:0] < SYSCTRL_WINDOW_SIZE) begin
            sel_sysctrl = 1'b1;
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
    
    assign sram_addr     = master_addr;
    assign sram_wdata    = master_wdata;
    assign sram_size     = master_size;
    
    assign sysctrl_addr  = master_addr;
    assign sysctrl_wdata = master_wdata;
    assign sysctrl_size  = master_size;
    
    // Request and write enable are only asserted for the selected slave
    // Unmapped addresses: writes are dropped (no req/we asserted)
    assign led_req      = master_req && sel_led;
    assign led_we       = master_we  && sel_led;
    
    assign clock_req    = master_req && sel_clock;
    assign clock_we     = master_we  && sel_clock;
    
    assign sram_req     = master_req && sel_sram;
    assign sram_we      = master_we  && sel_sram;
    
    assign sysctrl_req  = master_req && sel_sysctrl;
    assign sysctrl_we   = master_we  && sel_sysctrl;
    
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
        end else if (sel_sram) begin
            master_rdata = sram_rdata;
            master_ready = sram_ready;
        end else if (sel_sysctrl) begin
            master_rdata = sysctrl_rdata;
            master_ready = sysctrl_ready;
        end
        // Unmapped addresses: default values apply (return 0, ready = 1)
    end

endmodule

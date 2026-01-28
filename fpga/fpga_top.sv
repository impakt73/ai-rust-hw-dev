// FPGA Top-Level Module for Alchitry Cu v1
// Wraps RISC-V CPU with on-chip block RAM and simple peripherals
// Board: Alchitry Cu v1 (iCE40-HX8K-CB132)
// Configurable extension support for resource-constrained FPGA targets

module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b0,  // RV32M extension: Multiply/Divide (disabled for iCE40 resources)
    parameter bit ENABLE_F_EXT = 1'b0   // RV32F extension: Floating-Point (disabled for iCE40 resources)
) (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,
    
    // Reset button (active low)
    input  logic       rst_n_btn,
    
    // LED outputs (8 LEDs on Alchitry Cu main board)
    output logic [7:0] led
);

    // ============================================================
    // PLL Configuration - Generate 25 MHz from 100 MHz input
    // ============================================================
    // Using iCE40 PLL to divide 100 MHz input to 25 MHz for timing closure
    // PLL parameters calculated for: 100 MHz input -> 25 MHz output
    // DIVR = 0, DIVF = 7, DIVQ = 5 gives: 100 * (7+1) / (2^5) = 100 * 8 / 32 = 25 MHz
    
    logic pll_clk;       // PLL output clock (25 MHz)
    logic pll_locked;    // PLL lock indicator
    
    SB_PLL40_CORE #(
        .FEEDBACK_PATH("SIMPLE"),
        .DIVR(4'b0000),        // DIVR = 0
        .DIVF(7'b0000111),     // DIVF = 7
        .DIVQ(3'b101),         // DIVQ = 5 (divide by 32)
        .FILTER_RANGE(3'b001)  // Filter range for 100 MHz input
    ) pll_inst (
        .REFERENCECLK(clk),
        .PLLOUTCORE(pll_clk),
        .PLLOUTGLOBAL(),
        .LOCK(pll_locked),
        .BYPASS(1'b0),
        .RESETB(1'b1)
    );
    
    // Use PLL clock for all internal logic
    logic sys_clk;
    assign sys_clk = pll_clk;
    
    // ============================================================
    // Reset Synchronization
    // ============================================================
    logic rst_n;
    
    // Synchronize reset button to PLL clock domain (2-FF synchronizer)
    // Also incorporate PLL lock status into reset
    logic rst_n_sync1, rst_n_sync2;
    always_ff @(posedge sys_clk) begin
        rst_n_sync1 <= rst_n_btn & pll_locked;
        rst_n_sync2 <= rst_n_sync1;
    end
    assign rst_n = rst_n_sync2;
    
    // ============================================================
    // Memory Configuration
    // ============================================================
    // Boot address: Start of instruction memory (DRAM base)
    localparam logic [31:0] BOOT_ADDR = 32'h80000000;
    localparam logic [31:0] DRAM_BASE = 32'h80000000;
    
    // Instruction memory interface
    logic [31:0] imem_addr;
    logic [31:0] imem_data;
    logic        imem_req;
    logic        imem_ready;
    
    // Data memory interface (external - to Rust peripherals in simulation)
    logic [31:0] ext_mem_addr;
    logic [31:0] ext_mem_wdata;
    logic [31:0] ext_mem_rdata;
    logic        ext_mem_we;
    logic        ext_mem_re;
    logic [1:0]  ext_mem_size;
    logic        ext_mem_req;
    logic        ext_mem_ready;
    
    // BRAM word addresses (properly mapped from CPU addresses)
    logic [9:0] imem_bram_addr;  // Word address for 4KB = 1024 words
    logic [9:0] dmem_bram_addr;  // Word address for 4KB = 1024 words
    
    // Calculate word offset within BRAM (subtract DRAM base, then word-align)
    // This maps CPU address 0x80000000 -> BRAM offset 0, 0x80000004 -> offset 1, etc.
    assign imem_bram_addr = (imem_addr - DRAM_BASE) >> 2;
    assign dmem_bram_addr = (ext_mem_addr - DRAM_BASE) >> 2;
    
    // LED controller output
    logic [7:0]  led_out;
    
    // System control
    logic halted;
    logic instr_complete;
    
    // Debug signals (unused in FPGA, but required by CPU)
    logic [31:0] debug_rs1_data;
    logic [31:0] debug_rs2_data;
    logic [31:0] debug_rd_data;
    logic [31:0] debug_pc;
    logic [31:0] debug_instruction;
    logic [31:0] debug_current_pc;
    logic [31:0] debug_current_instruction;
    logic [3:0]  debug_fsm_state;
    
    // ============================================================
    // CPU Core with Peripherals
    // ============================================================
    top_with_peripherals #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) cpu (
        .clk(sys_clk),
        .rst_n(rst_n),
        .boot_addr(BOOT_ADDR),
        
        // Instruction memory
        .imem_addr(imem_addr),
        .imem_data(imem_data),
        .imem_req(imem_req),
        .imem_ready(imem_ready),
        
        // External data memory
        .ext_mem_addr(ext_mem_addr),
        .ext_mem_wdata(ext_mem_wdata),
        .ext_mem_rdata(ext_mem_rdata),
        .ext_mem_we(ext_mem_we),
        .ext_mem_re(ext_mem_re),
        .ext_mem_size(ext_mem_size),
        .ext_mem_req(ext_mem_req),
        .ext_mem_ready(ext_mem_ready),
        
        // LED peripheral
        .led_out(led_out),
        
        // System control
        .halted(halted),
        .instr_complete(instr_complete),
        
        // Debug outputs
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
    // Instruction Memory (Block RAM)
    // ============================================================
    // 4 KB instruction memory (1024 x 32-bit words)
    // Initialized with a simple test program
    bram_imem #(
        .ADDR_WIDTH(10),  // 2^10 = 1024 words = 4 KB
        .DATA_WIDTH(32)
    ) imem (
        .clk(sys_clk),
        .addr(imem_bram_addr),  // Use properly mapped BRAM address
        .rdata(imem_data),
        .req(imem_req),
        .ready(imem_ready)
    );
    
    // ============================================================
    // Data Memory (Block RAM)
    // ============================================================
    // 4 KB data memory (1024 x 32-bit words)
    bram_dmem #(
        .ADDR_WIDTH(10),  // 2^10 = 1024 words = 4 KB
        .DATA_WIDTH(32)
    ) dmem (
        .clk(sys_clk),
        .addr(dmem_bram_addr),  // Use properly mapped BRAM address
        .wdata(ext_mem_wdata),
        .rdata(ext_mem_rdata),
        .we(ext_mem_we),
        .re(ext_mem_re),
        .size(ext_mem_size),
        .req(ext_mem_req),
        .ready(ext_mem_ready)
    );
    
    // ============================================================
    // LED Output Assignment
    // ============================================================
    assign led = led_out;

endmodule

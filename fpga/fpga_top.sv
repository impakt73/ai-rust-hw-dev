// FPGA Top-Level Module for Alchitry Cu v1
// Wraps RISC-V CPU with on-chip block RAM and simple peripherals
// Board: Alchitry Cu v1 (iCE40-HX8K-CB132)
// Configurable extension support for resource-constrained FPGA targets

module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point (default: enabled)
) (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,
    
    // Reset button (active low)
    input  logic       rst_n_btn,
    
    // LED outputs (8 LEDs on Alchitry Cu main board)
    output logic [7:0] led
);

    // ============================================================
    // Clock and Reset
    // ============================================================
    logic rst_n;
    
    // Clock is already 100 MHz from the board
    // Note: For initial testing, we'll divide this down to a lower frequency
    // to ensure the design meets timing. Can be increased later.
    
    // Synchronize reset button (2-FF synchronizer)
    logic rst_n_sync1, rst_n_sync2;
    always_ff @(posedge clk) begin
        rst_n_sync1 <= rst_n_btn;
        rst_n_sync2 <= rst_n_sync1;
    end
    assign rst_n = rst_n_sync2;
    
    // ============================================================
    // Memory Configuration
    // ============================================================
    // Boot address: Start of instruction memory
    localparam logic [31:0] BOOT_ADDR = 32'h80000000;
    
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
        .clk(clk),
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
        .clk(clk),
        .addr(imem_addr[11:2]),  // Word-aligned (drop lower 2 bits)
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
        .clk(clk),
        .addr(ext_mem_addr[11:2]),  // Word-aligned (drop lower 2 bits)
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

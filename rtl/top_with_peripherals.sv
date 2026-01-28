// Top-Level Wrapper with RTL Peripheral Integration
// Wraps the RISC-V CPU core with RTL peripherals
// Routes RTL peripheral addresses internally, forwards others to external bus

module top_with_peripherals #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point (default: enabled)
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory interface (passed through to external bus)
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    output logic        imem_req,
    input  logic        imem_ready,
    
    // External data memory interface (for Rust peripherals + DRAM)
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
    
    // ============================================================
    // Internal CPU Memory Interface Signals
    // ============================================================
    logic [31:0] cpu_dmem_addr;
    logic [31:0] cpu_dmem_wdata;
    logic [31:0] cpu_dmem_rdata;
    logic        cpu_dmem_we;
    logic        cpu_dmem_re;
    logic [1:0]  cpu_dmem_size;
    logic        cpu_dmem_req;
    logic        cpu_dmem_ready;
    
    // ============================================================
    // LED Controller Interface Signals
    // ============================================================
    logic [31:0] led_rdata;
    logic        led_ready;
    
    // ============================================================
    // Address Decoder
    // ============================================================
    logic sel_led;
    logic sel_external;
    logic sel_unmapped_rtl;
    
    always_comb begin
        sel_led          = 1'b0;
        sel_unmapped_rtl = 1'b0;
        sel_external     = 1'b0;
        
        // Check if address is in LED range
        if (cpu_dmem_addr >= LED_BASE && cpu_dmem_addr < LED_LIMIT) begin
            sel_led = 1'b1;
        end
        // Check if address is in unmapped RTL peripheral space
        else if (cpu_dmem_addr >= RTL_PERIPH_BASE && cpu_dmem_addr < RTL_PERIPH_LIMIT) begin
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
        cpu_dmem_rdata = 32'h0;
        cpu_dmem_ready = 1'b0;
        
        // Select response source
        if (sel_led) begin
            cpu_dmem_rdata = led_rdata;
            cpu_dmem_ready = led_ready;
        end else if (sel_unmapped_rtl) begin
            // Unmapped RTL peripheral address - return zero and ready immediately
            cpu_dmem_rdata = 32'h0;
            cpu_dmem_ready = 1'b1;
            // Note: In simulation, this triggers a warning via $display in tests
        end else if (sel_external) begin
            cpu_dmem_rdata = ext_mem_rdata;
            cpu_dmem_ready = ext_mem_ready;
        end else begin
            // Should never reach here if decoder logic is correct
            cpu_dmem_rdata = 32'h0;
            cpu_dmem_ready = 1'b1;
        end
    end
    
    // ============================================================
    // External Bus Forwarding
    // ============================================================
    // Forward CPU requests to external bus (for Rust peripherals + DRAM)
    assign ext_mem_addr  = cpu_dmem_addr;
    assign ext_mem_wdata = cpu_dmem_wdata;
    assign ext_mem_size  = cpu_dmem_size;
    
    // Only assert request/enable if address is external
    assign ext_mem_req = cpu_dmem_req && sel_external;
    assign ext_mem_we  = cpu_dmem_we  && sel_external;
    assign ext_mem_re  = cpu_dmem_re  && sel_external;
    
    // ============================================================
    // CPU Core Instantiation
    // ============================================================
    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) cpu_core (
        .clk(clk),
        .rst_n(rst_n),
        .boot_addr(boot_addr),
        
        // Instruction memory (passed through)
        .imem_addr(imem_addr),
        .imem_data(imem_data),
        .imem_req(imem_req),
        .imem_ready(imem_ready),
        
        // Data memory (internal to wrapper)
        .dmem_addr(cpu_dmem_addr),
        .dmem_wdata(cpu_dmem_wdata),
        .dmem_rdata(cpu_dmem_rdata),
        .dmem_we(cpu_dmem_we),
        .dmem_re(cpu_dmem_re),
        .dmem_size(cpu_dmem_size),
        .dmem_req(cpu_dmem_req),
        .dmem_ready(cpu_dmem_ready),
        
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
    // LED Controller Instantiation
    // ============================================================
    led_controller led_ctrl (
        .clk(clk),
        .rst_n(rst_n),
        
        // CPU interface
        .addr(cpu_dmem_addr),
        .wdata(cpu_dmem_wdata),
        .rdata(led_rdata),
        .we(cpu_dmem_we && sel_led),
        .re(cpu_dmem_re && sel_led),
        .size(cpu_dmem_size),
        .ready(led_ready),
        
        // LED outputs
        .led_out(led_out)
    );

endmodule

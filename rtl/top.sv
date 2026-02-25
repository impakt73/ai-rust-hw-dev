// Top-Level Module
// Wraps the RISC-V CPU core with RTL peripherals
// Uses the bus module to route RTL peripheral requests
// External memory requests are routed directly from CPU to host_bus_interface
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
    // System Clock Frequency (used by Clock Peripheral)
    parameter int CLK_FREQ_HZ = 50_000_000,
    parameter int RESET_CYCLES = 8      // Number of cycles to hold reset after release
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        reset_request,
    
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
    
    // System LED output
    output logic [7:0]  sys_led_out,
    
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
    output logic [3:0]  debug_fsm_state,
    output logic        rst_n_out,
    output logic        cpu_booting
);

    // ============================================================
    // Reset Controller
    // ============================================================
    logic rst_n_internal;

    reset_controller #(
        .RESET_CYCLES(RESET_CYCLES)
    ) reset_ctrl (
        .clk(clk),
        .rst_n_in(rst_n),
        .reset_request(reset_request | sysctrl_sys_rst),
        .rst_n_out(rst_n_internal)
    );

    assign rst_n_out = rst_n_internal;

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
    // SRAM Peripheral Interface Signals
    // ============================================================
    logic [31:0] sram_addr;
    logic [31:0] sram_wdata;
    logic [31:0] sram_rdata;
    logic        sram_we;
    logic [1:0]  sram_size;
    logic        sram_req;
    logic        sram_ready;
    
    // ============================================================
    // System Controller Interface Signals
    // ============================================================
    logic [31:0] sysctrl_addr;
    logic [31:0] sysctrl_wdata;
    logic [31:0] sysctrl_rdata;
    logic        sysctrl_we;
    logic [1:0]  sysctrl_size;
    logic        sysctrl_req;
    logic        sysctrl_ready;
    
    // System Controller control signals
    logic        sysctrl_sys_rst;
    logic        sysctrl_cpu_rst_n;
    logic        sysctrl_cpu_boot;
    logic [31:0] sysctrl_cpu_boot_addr;
    logic        sysctrl_req_cpu_halt;
    logic        cpu_is_booting;
    logic        cpu_halted_internal;
    
    // ============================================================
    // CPU→Arbiter Signals (RTL peripheral accesses only)
    // ============================================================
    logic [31:0] cpu_to_arb_addr;
    logic [31:0] cpu_to_arb_wdata;
    logic [31:0] cpu_to_arb_rdata;
    logic        cpu_to_arb_we;
    logic [1:0]  cpu_to_arb_size;
    logic        cpu_to_arb_req;
    logic        cpu_to_arb_ready;
    
    // ============================================================
    // CPU→External Interface Signals (non-RTL peripheral accesses)
    // ============================================================
    logic [31:0] cpu_to_ext_addr;
    logic [31:0] cpu_to_ext_wdata;
    logic [31:0] cpu_to_ext_rdata;
    logic        cpu_to_ext_we;
    logic [1:0]  cpu_to_ext_size;
    logic        cpu_to_ext_req;
    logic        cpu_to_ext_ready;
    
    // ============================================================
    // Arbiter Output Signals (Arbiter → Bus)
    // ============================================================
    logic [31:0] arb_bus_addr;
    logic [31:0] arb_bus_wdata;
    logic [31:0] arb_bus_rdata;
    logic        arb_bus_we;
    logic [1:0]  arb_bus_size;
    logic        arb_bus_req;
    logic        arb_bus_ready;
    
    // ============================================================
    // Host Bus Master Interface Signals (Host → Arbiter)
    // ============================================================
    logic [31:0] host_master_addr;
    logic [31:0] host_master_wdata;
    logic [31:0] host_master_rdata;
    logic        host_master_we;
    logic [1:0]  host_master_size;
    logic        host_master_req;
    logic        host_master_ready;
    
    // Shared RTL peripheral range configuration for host_bus_mux and bus
    localparam RTL_PERIPH_BASE  = 32'h50000000;
    localparam RTL_PERIPH_LIMIT = 32'h60000000;
    
    // ============================================================
    // CPU Host-Bus Multiplexer
    // ============================================================
    host_bus_mux #(
        .RTL_PERIPH_BASE(RTL_PERIPH_BASE),
        .RTL_PERIPH_LIMIT(RTL_PERIPH_LIMIT)
    ) cpu_host_bus_mux (
        // CPU-side interface
        .cpu_addr(cpu_mem_addr),
        .cpu_wdata(cpu_mem_wdata),
        .cpu_rdata(cpu_mem_rdata),
        .cpu_we(cpu_mem_we),
        .cpu_size(cpu_mem_size),
        .cpu_req(cpu_mem_req),
        .cpu_ready(cpu_mem_ready),
        
        // System bus path (RTL peripherals)
        .sys_addr(cpu_to_arb_addr),
        .sys_wdata(cpu_to_arb_wdata),
        .sys_rdata(cpu_to_arb_rdata),
        .sys_we(cpu_to_arb_we),
        .sys_size(cpu_to_arb_size),
        .sys_req(cpu_to_arb_req),
        .sys_ready(cpu_to_arb_ready),
        
        // Host bus path (external memory / Rust peripherals)
        .host_addr(cpu_to_ext_addr),
        .host_wdata(cpu_to_ext_wdata),
        .host_rdata(cpu_to_ext_rdata),
        .host_we(cpu_to_ext_we),
        .host_size(cpu_to_ext_size),
        .host_req(cpu_to_ext_req),
        .host_ready(cpu_to_ext_ready)
    );
    
    // ============================================================
    // Bus Arbiter Instantiation
    // ============================================================
    // Arbitrates between CPU and Host master for bus access
    // Priority: Host > CPU
    bus_arbiter arbiter (
        .clk(clk),
        .rst_n(rst_n_internal),
        
        // CPU Master Interface
        .cpu_addr(cpu_to_arb_addr),
        .cpu_wdata(cpu_to_arb_wdata),
        .cpu_rdata(cpu_to_arb_rdata),
        .cpu_we(cpu_to_arb_we),
        .cpu_size(cpu_to_arb_size),
        .cpu_req(cpu_to_arb_req),
        .cpu_ready(cpu_to_arb_ready),
        
        // Host Master Interface (from host_bus_interface)
        .host_addr(host_master_addr),
        .host_wdata(host_master_wdata),
        .host_rdata(host_master_rdata),
        .host_we(host_master_we),
        .host_size(host_master_size),
        .host_req(host_master_req),
        .host_ready(host_master_ready),
        
        // Slave Interface (to system_bus)
        .bus_addr(arb_bus_addr),
        .bus_wdata(arb_bus_wdata),
        .bus_rdata(arb_bus_rdata),
        .bus_we(arb_bus_we),
        .bus_size(arb_bus_size),
        .bus_req(arb_bus_req),
        .bus_ready(arb_bus_ready)
    );
    
    // ============================================================
    // Bus Module Instantiation
    // ============================================================
    // Routes requests from arbiter to the appropriate peripheral based on address
    bus #(
        .RTL_PERIPH_BASE(RTL_PERIPH_BASE),
        .RTL_PERIPH_LIMIT(RTL_PERIPH_LIMIT)
    ) system_bus (
        .clk(clk),
        .rst_n(rst_n_internal),
        
        // Master interface (from Arbiter)
        .master_addr(arb_bus_addr),
        .master_wdata(arb_bus_wdata),
        .master_rdata(arb_bus_rdata),
        .master_we(arb_bus_we),
        .master_size(arb_bus_size),
        .master_req(arb_bus_req),
        .master_ready(arb_bus_ready),
        
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
        
        // SRAM Peripheral interface
        .sram_addr(sram_addr),
        .sram_wdata(sram_wdata),
        .sram_rdata(sram_rdata),
        .sram_we(sram_we),
        .sram_size(sram_size),
        .sram_req(sram_req),
        .sram_ready(sram_ready),
        
        // System Controller interface
        .sysctrl_addr(sysctrl_addr),
        .sysctrl_wdata(sysctrl_wdata),
        .sysctrl_rdata(sysctrl_rdata),
        .sysctrl_we(sysctrl_we),
        .sysctrl_size(sysctrl_size),
        .sysctrl_req(sysctrl_req),
        .sysctrl_ready(sysctrl_ready)
    );
    
    // ============================================================
    // Host Bus Interface Instantiation
    // ============================================================
    // Serializes external memory transactions to byte stream for host communication
    // - Slave interface: Receives CPU-initiated external memory requests from host_bus_mux
    // - Master interface: Sends Host-initiated requests to arbiter (currently unused)
    host_bus_interface host_bus_if (
        .clk(clk),
        .rst_n(rst_n_internal),
        
        // Bus Slave Interface (from host_bus_mux CPU external path)
        .addr(cpu_to_ext_addr),
        .wdata(cpu_to_ext_wdata),
        .rdata(cpu_to_ext_rdata),
        .we(cpu_to_ext_we),
        .size(cpu_to_ext_size),
        .req(cpu_to_ext_req),
        .ready(cpu_to_ext_ready),
        
        // Bus Master Interface (to Arbiter - Host→CPU path, currently unused)
        .host_bus_addr(host_master_addr),
        .host_bus_wdata(host_master_wdata),
        .host_bus_rdata(host_master_rdata),
        .host_bus_we(host_master_we),
        .host_bus_size(host_master_size),
        .host_bus_req(host_master_req),
        .host_bus_ready(host_master_ready),
        
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
    // CPU Reset Signal - Combined from internal reset and system controller
    // ============================================================
    // CPU is reset when either the internal reset or system controller requests it
    logic cpu_combined_rst_n;
    assign cpu_combined_rst_n = rst_n_internal & sysctrl_cpu_rst_n;
    
    // ============================================================
    // CPU Core Instantiation
    // ============================================================
    cpu #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) cpu_core (
        .clk(clk),
        .rst_n(cpu_combined_rst_n),
        .boot(sysctrl_cpu_boot),
        .req_halt(sysctrl_req_cpu_halt),
        .boot_addr(sysctrl_cpu_boot_addr),
        
        // Unified memory interface
        .mem_addr(cpu_mem_addr),
        .mem_wdata(cpu_mem_wdata),
        .mem_rdata(cpu_mem_rdata),
        .mem_we(cpu_mem_we),
        .mem_size(cpu_mem_size),
        .mem_req(cpu_mem_req),
        .mem_ready(cpu_mem_ready),
        
        // System control
        .halted(cpu_halted_internal),
        .instr_complete(instr_complete),
        
        // Debug signals
        .debug_rs1_data(debug_rs1_data),
        .debug_rs2_data(debug_rs2_data),
        .debug_rd_data(debug_rd_data),
        .debug_pc(debug_pc),
        .debug_instruction(debug_instruction),
        .debug_current_pc(debug_current_pc),
        .debug_current_instruction(debug_current_instruction),
        .debug_fsm_state(debug_fsm_state),
        
        // Boot state indicator
        .is_booting(cpu_is_booting)
    );
    
    // Pass through halted signal
    assign halted = cpu_halted_internal;
    
    // Pass through cpu boot state signal
    assign cpu_booting = cpu_is_booting;
    
    // ============================================================
    // LED Controller Instantiation
    // ============================================================
    led_controller_peripheral led_ctrl (
        .clk(clk),
        .rst_n(rst_n_internal),
        
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
        .rst_n(rst_n_internal),
        
        .addr(clock_addr),
        .wdata(clock_wdata),
        .rdata(clock_rdata),
        .we(clock_we),
        .req(clock_req),
        .size(clock_size),
        .ready(clock_ready)
    );
    
    // ============================================================
    // SRAM Peripheral Instantiation
    // ============================================================
    sram_peripheral sram_periph (
        .clk(clk),
        .rst_n(rst_n_internal),
        
        .addr(sram_addr),
        .wdata(sram_wdata),
        .rdata(sram_rdata),
        .we(sram_we),
        .req(sram_req),
        .size(sram_size),
        .ready(sram_ready)
    );
    
    // ============================================================
    // System Controller Instantiation
    // ============================================================
    system_controller sysctrl (
        .clk(clk),
        .rst_n(rst_n_internal),
        
        // Bus slave interface
        .addr(sysctrl_addr),
        .wdata(sysctrl_wdata),
        .rdata(sysctrl_rdata),
        .we(sysctrl_we),
        .req(sysctrl_req),
        .size(sysctrl_size),
        .ready(sysctrl_ready),
        
        // System control outputs
        .sys_rst(sysctrl_sys_rst),
        .cpu_rst_n(sysctrl_cpu_rst_n),
        .cpu_boot_addr(sysctrl_cpu_boot_addr),
        .cpu_boot(sysctrl_cpu_boot),
        .req_cpu_halt(sysctrl_req_cpu_halt),
        
        // CPU status inputs
        .cpu_halted(cpu_halted_internal),
        .cpu_booting(cpu_is_booting)
    );

    // ============================================================
    // System LED Controller Instantiation
    // ============================================================
    sys_led_controller #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ)
    ) sys_led_ctrl (
        .clk(clk),
        .rst_n(rst_n_internal),
        .cpu_booting(cpu_is_booting),
        .cpu_halted(cpu_halted_internal),
        .sys_led(sys_led_out)
    );

endmodule

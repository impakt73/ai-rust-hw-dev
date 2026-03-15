`default_nettype none
// Top-Level Module
// Wraps the RISC-V CPU core with RTL peripherals
// Uses registered_bus to route RTL peripheral requests
// External memory requests are routed from CPU to host_bus_interface
//
// CPU MEMORY CHANNELS: CPU uses separate address (A) and data (D) channels.
// host_bus_mux routes CPU A/D channels to either the external host bus
// interface or the RTL peripheral path. registered_bus arbitrates between
// CPU-originated RTL accesses and host-initiated RTL accesses, then routes
// requests to the downstream RTL peripherals.
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
    input wire logic        clk,
    input wire logic        rst,
    
    // Host TX Interface (to External Host)
    // Serialized bus transactions sent to host
    output logic [7:0]  host_tx_data,
    output logic        host_tx_valid,
    input wire logic        host_tx_ready,
    
    // Host RX Interface (from External Host)
    // Serialized bus transaction responses from host
    input wire logic [7:0]  host_rx_data,
    input wire logic        host_rx_valid,
    output logic        host_rx_ready,
    input wire logic        com_err,
    
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
    output logic        rst_out,
    output logic        cpu_booting,
    output logic [31:0] halted_value
);

    // ============================================================
    // Reset Controller
    // ============================================================
    logic rst_internal;

    reset_controller #(
        .RESET_CYCLES(RESET_CYCLES)
    ) reset_ctrl (
        .clk(clk),
        .rst_in(rst | sysctrl_sys_rst),
        .rst_out(rst_internal)
    );

    assign rst_out = rst_internal;

    // ============================================================
    // CPU <-> host_bus_mux Memory Channel Signals
    // ============================================================
    logic [31:0] cpu_mem_a_addr;
    logic [31:0] cpu_mem_a_wdata;
    logic        cpu_mem_a_we;
    logic [1:0]  cpu_mem_a_size;
    logic        cpu_mem_a_valid;
    logic        cpu_mem_a_ready;
    logic [31:0] cpu_mem_d_rdata;
    logic        cpu_mem_d_valid;
    logic        cpu_mem_d_ready;
    
    // ============================================================
    // LED Controller Interface Signals
    // ============================================================
    logic [31:0] led_mem_a_addr;
    logic [31:0] led_mem_a_wdata;
    logic        led_mem_a_we;
    logic [1:0]  led_mem_a_size;
    logic        led_mem_a_valid;
    logic        led_mem_a_ready;
    logic [31:0] led_mem_d_rdata;
    logic        led_mem_d_valid;
    logic        led_mem_d_ready;
    
    // ============================================================
    // Clock Peripheral Interface Signals
    // ============================================================
    logic [31:0] clock_mem_a_addr;
    logic [31:0] clock_mem_a_wdata;
    logic        clock_mem_a_we;
    logic [1:0]  clock_mem_a_size;
    logic        clock_mem_a_valid;
    logic        clock_mem_a_ready;
    logic [31:0] clock_mem_d_rdata;
    logic        clock_mem_d_valid;
    logic        clock_mem_d_ready;
    
    // ============================================================
    // SRAM Peripheral Interface Signals
    // ============================================================
    logic [31:0] sram_mem_a_addr;
    logic [31:0] sram_mem_a_wdata;
    logic        sram_mem_a_we;
    logic [1:0]  sram_mem_a_size;
    logic        sram_mem_a_valid;
    logic        sram_mem_a_ready;
    logic [31:0] sram_mem_d_rdata;
    logic        sram_mem_d_valid;
    logic        sram_mem_d_ready;
    
    // ============================================================
    // System Controller Interface Signals
    // ============================================================
    logic [31:0] sysctrl_mem_a_addr;
    logic [31:0] sysctrl_mem_a_wdata;
    logic        sysctrl_mem_a_we;
    logic [1:0]  sysctrl_mem_a_size;
    logic        sysctrl_mem_a_valid;
    logic        sysctrl_mem_a_ready;
    logic [31:0] sysctrl_mem_d_rdata;
    logic        sysctrl_mem_d_valid;
    logic        sysctrl_mem_d_ready;
    
    // System Controller control signals
    logic        sysctrl_sys_rst;
    logic        sysctrl_cpu_rst;
    logic        sysctrl_cpu_boot;
    logic [31:0] sysctrl_cpu_boot_addr;
    logic        sysctrl_req_cpu_halt;
    logic [31:0] sysctrl_halted_value;
    logic        cpu_is_booting;
    logic        cpu_halted_internal;
    
    // ============================================================
    // host_bus_mux -> registered_bus Signals (CPU RTL peripheral accesses only)
    // ============================================================
    logic [31:0] cpu_to_arb_a_addr;
    logic [31:0] cpu_to_arb_a_wdata;
    logic        cpu_to_arb_a_we;
    logic [1:0]  cpu_to_arb_a_size;
    logic        cpu_to_arb_a_valid;
    logic        cpu_to_arb_a_ready;
    logic [31:0] cpu_to_arb_d_rdata;
    logic        cpu_to_arb_d_valid;
    logic        cpu_to_arb_d_ready;

    // ============================================================
    // CPU→External Interface Signals (non-RTL peripheral accesses)
    // ============================================================
    logic [31:0] cpu_to_ext_a_addr;
    logic [31:0] cpu_to_ext_a_wdata;
    logic        cpu_to_ext_a_we;
    logic [1:0]  cpu_to_ext_a_size;
    logic        cpu_to_ext_a_valid;
    logic        cpu_to_ext_a_ready;
    logic [31:0] cpu_to_ext_d_rdata;
    logic        cpu_to_ext_d_valid;
    logic        cpu_to_ext_d_ready;
    
    // ============================================================
    // registered_bus Master/Slave Wiring
    // ============================================================
    logic [31:0] host_mem_a_addr;
    logic [31:0] host_mem_a_wdata;
    logic        host_mem_a_we;
    logic [1:0]  host_mem_a_size;
    logic        host_mem_a_valid;
    logic        host_mem_a_ready;
    logic [31:0] host_mem_d_rdata;
    logic        host_mem_d_valid;
    logic        host_mem_d_ready;

    logic [63:0]      registered_master_mem_a_addr;
    logic [63:0]      registered_master_mem_a_wdata;
    logic [1:0]       registered_master_mem_a_we;
    logic [3:0]       registered_master_mem_a_size;
    logic [1:0]       registered_master_mem_a_valid;
    logic [1:0]       registered_master_mem_a_ready;
    logic [63:0]      registered_master_mem_d_rdata;
    logic [1:0]       registered_master_mem_d_valid;
    logic [1:0]       registered_master_mem_d_ready;

    logic [127:0]     registered_slave_base_addr;
    logic [127:0]     registered_slave_addr_size;
    logic [127:0]     registered_slave_mem_a_addr;
    logic [127:0]     registered_slave_mem_a_wdata;
    logic [3:0]       registered_slave_mem_a_we;
    logic [7:0]       registered_slave_mem_a_size;
    logic [3:0]       registered_slave_mem_a_valid;
    logic [3:0]       registered_slave_mem_a_ready;
    logic [127:0]     registered_slave_mem_d_rdata;
    logic [3:0]       registered_slave_mem_d_valid;
    logic [3:0]       registered_slave_mem_d_ready;

    logic             sys_bus_handshake;

    // ============================================================
    // CPU Reset Signal - Combined from internal reset and system controller
    // ============================================================
    // CPU is reset when either the internal reset or system controller requests it
    logic cpu_combined_rst;
    assign cpu_combined_rst = rst_internal | sysctrl_cpu_rst;
    
    // ============================================================
    // CPU Host-Bus Multiplexer
    // ============================================================
    host_bus_mux cpu_host_bus_mux (
        .clk(clk),
        .rst(rst_internal),

        // CPU-side interface
        .cpu_mem_a_addr(cpu_mem_a_addr),
        .cpu_mem_a_wdata(cpu_mem_a_wdata),
        .cpu_mem_a_we(cpu_mem_a_we),
        .cpu_mem_a_size(cpu_mem_a_size),
        .cpu_mem_a_valid(cpu_mem_a_valid),
        .cpu_mem_a_ready(cpu_mem_a_ready),
        .cpu_mem_d_rdata(cpu_mem_d_rdata),
        .cpu_mem_d_valid(cpu_mem_d_valid),
        .cpu_mem_d_ready(cpu_mem_d_ready),
        
        // System bus path (RTL peripherals)
        .sys_mem_a_addr(cpu_to_arb_a_addr),
        .sys_mem_a_wdata(cpu_to_arb_a_wdata),
        .sys_mem_a_we(cpu_to_arb_a_we),
        .sys_mem_a_size(cpu_to_arb_a_size),
        .sys_mem_a_valid(cpu_to_arb_a_valid),
        .sys_mem_a_ready(cpu_to_arb_a_ready),
        .sys_mem_d_rdata(cpu_to_arb_d_rdata),
        .sys_mem_d_valid(cpu_to_arb_d_valid),
        .sys_mem_d_ready(cpu_to_arb_d_ready),
        
        // Host bus path (external memory / Rust peripherals)
        .host_mem_a_addr(cpu_to_ext_a_addr),
        .host_mem_a_wdata(cpu_to_ext_a_wdata),
        .host_mem_a_we(cpu_to_ext_a_we),
        .host_mem_a_size(cpu_to_ext_a_size),
        .host_mem_a_valid(cpu_to_ext_a_valid),
        .host_mem_a_ready(cpu_to_ext_a_ready),
        .host_mem_d_rdata(cpu_to_ext_d_rdata),
        .host_mem_d_valid(cpu_to_ext_d_valid),
        .host_mem_d_ready(cpu_to_ext_d_ready)
    );

    assign registered_master_mem_a_addr[31:0] = host_mem_a_addr;
    assign registered_master_mem_a_wdata[31:0] = host_mem_a_wdata;
    assign registered_master_mem_a_we[0] = host_mem_a_we;
    assign registered_master_mem_a_size[1:0] = host_mem_a_size;
    assign registered_master_mem_a_valid[0] = host_mem_a_valid;
    assign host_mem_a_ready = registered_master_mem_a_ready[0];
    assign host_mem_d_rdata = registered_master_mem_d_rdata[31:0];
    assign host_mem_d_valid = registered_master_mem_d_valid[0];
    assign registered_master_mem_d_ready[0] = host_mem_d_ready;

    assign registered_master_mem_a_addr[63:32] = cpu_to_arb_a_addr;
    assign registered_master_mem_a_wdata[63:32] = cpu_to_arb_a_wdata;
    assign registered_master_mem_a_we[1] = cpu_to_arb_a_we;
    assign registered_master_mem_a_size[3:2] = cpu_to_arb_a_size;
    assign registered_master_mem_a_valid[1] = cpu_to_arb_a_valid;
    assign cpu_to_arb_a_ready = registered_master_mem_a_ready[1];
    assign cpu_to_arb_d_rdata = registered_master_mem_d_rdata[63:32];
    assign cpu_to_arb_d_valid = registered_master_mem_d_valid[1];
    assign registered_master_mem_d_ready[1] = cpu_to_arb_d_ready;

    assign registered_slave_base_addr[31:0] = 32'h2000_0000;
    assign registered_slave_addr_size[31:0] = 32'h0000_0010;
    assign registered_slave_base_addr[63:32] = 32'h5000_0000;
    assign registered_slave_addr_size[63:32] = 32'h0000_0010;
    assign registered_slave_base_addr[95:64] = 32'h6000_0000;
    assign registered_slave_addr_size[95:64] = 32'h0000_0010;
    assign registered_slave_base_addr[127:96] = 32'h7000_0000;
    assign registered_slave_addr_size[127:96] = 32'h0000_3000;

    assign sysctrl_mem_a_addr = registered_slave_mem_a_addr[31:0];
    assign sysctrl_mem_a_wdata = registered_slave_mem_a_wdata[31:0];
    assign sysctrl_mem_a_we = registered_slave_mem_a_we[0];
    assign sysctrl_mem_a_size = registered_slave_mem_a_size[1:0];
    assign sysctrl_mem_a_valid = registered_slave_mem_a_valid[0];
    assign registered_slave_mem_a_ready[0] = sysctrl_mem_a_ready;
    assign registered_slave_mem_d_rdata[31:0] = sysctrl_mem_d_rdata;
    assign registered_slave_mem_d_valid[0] = sysctrl_mem_d_valid;
    assign sysctrl_mem_d_ready = registered_slave_mem_d_ready[0];

    assign led_mem_a_addr = registered_slave_mem_a_addr[63:32];
    assign led_mem_a_wdata = registered_slave_mem_a_wdata[63:32];
    assign led_mem_a_we = registered_slave_mem_a_we[1];
    assign led_mem_a_size = registered_slave_mem_a_size[3:2];
    assign led_mem_a_valid = registered_slave_mem_a_valid[1];
    assign registered_slave_mem_a_ready[1] = led_mem_a_ready;
    assign registered_slave_mem_d_rdata[63:32] = led_mem_d_rdata;
    assign registered_slave_mem_d_valid[1] = led_mem_d_valid;
    assign led_mem_d_ready = registered_slave_mem_d_ready[1];

    assign clock_mem_a_addr = registered_slave_mem_a_addr[95:64];
    assign clock_mem_a_wdata = registered_slave_mem_a_wdata[95:64];
    assign clock_mem_a_we = registered_slave_mem_a_we[2];
    assign clock_mem_a_size = registered_slave_mem_a_size[5:4];
    assign clock_mem_a_valid = registered_slave_mem_a_valid[2];
    assign registered_slave_mem_a_ready[2] = clock_mem_a_ready;
    assign registered_slave_mem_d_rdata[95:64] = clock_mem_d_rdata;
    assign registered_slave_mem_d_valid[2] = clock_mem_d_valid;
    assign clock_mem_d_ready = registered_slave_mem_d_ready[2];

    assign sram_mem_a_addr = registered_slave_mem_a_addr[127:96];
    assign sram_mem_a_wdata = registered_slave_mem_a_wdata[127:96];
    assign sram_mem_a_we = registered_slave_mem_a_we[3];
    assign sram_mem_a_size = registered_slave_mem_a_size[7:6];
    assign sram_mem_a_valid = registered_slave_mem_a_valid[3];
    assign registered_slave_mem_a_ready[3] = sram_mem_a_ready;
    assign registered_slave_mem_d_rdata[127:96] = sram_mem_d_rdata;
    assign registered_slave_mem_d_valid[3] = sram_mem_d_valid;
    assign sram_mem_d_ready = registered_slave_mem_d_ready[3];

    assign sys_bus_handshake =
        (host_mem_d_valid && host_mem_d_ready) ||
        (cpu_to_arb_d_valid && cpu_to_arb_d_ready);

    // ============================================================
    // Registered Bus Instantiation
    // ============================================================
    registered_bus #(
        .NUM_MASTERS(2),
        .NUM_SLAVES(4)
    ) rtl_registered_bus (
        .clk(clk),
        .rst(rst_internal),

        .master_mem_a_addr(registered_master_mem_a_addr),
        .master_mem_a_wdata(registered_master_mem_a_wdata),
        .master_mem_a_we(registered_master_mem_a_we),
        .master_mem_a_size(registered_master_mem_a_size),
        .master_mem_a_valid(registered_master_mem_a_valid),
        .master_mem_a_ready(registered_master_mem_a_ready),

        .master_mem_d_rdata(registered_master_mem_d_rdata),
        .master_mem_d_valid(registered_master_mem_d_valid),
        .master_mem_d_ready(registered_master_mem_d_ready),

        .slave_base_addr(registered_slave_base_addr),
        .slave_addr_size(registered_slave_addr_size),

        .slave_mem_a_addr(registered_slave_mem_a_addr),
        .slave_mem_a_wdata(registered_slave_mem_a_wdata),
        .slave_mem_a_we(registered_slave_mem_a_we),
        .slave_mem_a_size(registered_slave_mem_a_size),
        .slave_mem_a_valid(registered_slave_mem_a_valid),
        .slave_mem_a_ready(registered_slave_mem_a_ready),

        .slave_mem_d_rdata(registered_slave_mem_d_rdata),
        .slave_mem_d_valid(registered_slave_mem_d_valid),
        .slave_mem_d_ready(registered_slave_mem_d_ready)
    );
    
    // ============================================================
    // Host Bus Interface Instantiation
    // ============================================================
    // Serializes external memory transactions to byte stream for host communication
    // - Slave interface: Receives CPU-initiated external memory requests from host_bus_mux
    // - Master interface: Sends Host-initiated requests to arbiter (currently unused)
    host_bus_interface host_bus_if (
        .clk(clk),
        .rst(rst_internal),
        
        // CPU Slave Interface (from host_bus_mux CPU external path)
        .mem_a_addr(cpu_to_ext_a_addr),
        .mem_a_wdata(cpu_to_ext_a_wdata),
        .mem_a_we(cpu_to_ext_a_we),
        .mem_a_size(cpu_to_ext_a_size),
        .mem_a_valid(cpu_to_ext_a_valid),
        .mem_a_ready(cpu_to_ext_a_ready),
        .mem_d_rdata(cpu_to_ext_d_rdata),
        .mem_d_valid(cpu_to_ext_d_valid),
        .mem_d_ready(cpu_to_ext_d_ready),
        
        // Host-initiated master interface (to registered_bus)
        .host_mem_a_addr(host_mem_a_addr),
        .host_mem_a_wdata(host_mem_a_wdata),
        .host_mem_a_we(host_mem_a_we),
        .host_mem_a_size(host_mem_a_size),
        .host_mem_a_valid(host_mem_a_valid),
        .host_mem_a_ready(host_mem_a_ready),
        .host_mem_d_rdata(host_mem_d_rdata),
        .host_mem_d_valid(host_mem_d_valid),
        .host_mem_d_ready(host_mem_d_ready),
        
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
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) cpu_core (
        .clk(clk),
        .rst(cpu_combined_rst),
        .boot(sysctrl_cpu_boot),
        .req_halt(sysctrl_req_cpu_halt),
        .boot_addr(sysctrl_cpu_boot_addr),
        
        // Memory address/data channels
        .mem_a_addr(cpu_mem_a_addr),
        .mem_a_wdata(cpu_mem_a_wdata),
        .mem_a_we(cpu_mem_a_we),
        .mem_a_size(cpu_mem_a_size),
        .mem_a_valid(cpu_mem_a_valid),
        .mem_a_ready(cpu_mem_a_ready),
        .mem_d_rdata(cpu_mem_d_rdata),
        .mem_d_valid(cpu_mem_d_valid),
        .mem_d_ready(cpu_mem_d_ready),
        
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
        .rst(rst_internal),

        .mem_a_addr(led_mem_a_addr),
        .mem_a_wdata(led_mem_a_wdata),
        .mem_a_we(led_mem_a_we),
        .mem_a_size(led_mem_a_size),
        .mem_a_valid(led_mem_a_valid),
        .mem_a_ready(led_mem_a_ready),
        .mem_d_rdata(led_mem_d_rdata),
        .mem_d_valid(led_mem_d_valid),
        .mem_d_ready(led_mem_d_ready),

        .led_out(led_out)
    );
    
    // ============================================================
    // Clock Peripheral Instantiation
    // ============================================================
    clock_peripheral #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ)
    ) clock_periph (
        .clk(clk),
        .rst(rst_internal),

        .mem_a_addr(clock_mem_a_addr),
        .mem_a_wdata(clock_mem_a_wdata),
        .mem_a_we(clock_mem_a_we),
        .mem_a_size(clock_mem_a_size),
        .mem_a_valid(clock_mem_a_valid),
        .mem_a_ready(clock_mem_a_ready),
        .mem_d_rdata(clock_mem_d_rdata),
        .mem_d_valid(clock_mem_d_valid),
        .mem_d_ready(clock_mem_d_ready)
    );
    
    // ============================================================
    // SRAM Peripheral Instantiation
    // ============================================================
    sram_peripheral sram_periph (
        .clk(clk),
        .rst(rst_internal),

        .mem_a_addr(sram_mem_a_addr),
        .mem_a_wdata(sram_mem_a_wdata),
        .mem_a_we(sram_mem_a_we),
        .mem_a_size(sram_mem_a_size),
        .mem_a_valid(sram_mem_a_valid),
        .mem_a_ready(sram_mem_a_ready),
        .mem_d_rdata(sram_mem_d_rdata),
        .mem_d_valid(sram_mem_d_valid),
        .mem_d_ready(sram_mem_d_ready)
    );
    
    // ============================================================
    // System Controller Instantiation
    // ============================================================
    system_controller sysctrl (
        .clk(clk),
        .rst(rst_internal),

        .mem_a_addr(sysctrl_mem_a_addr),
        .mem_a_wdata(sysctrl_mem_a_wdata),
        .mem_a_we(sysctrl_mem_a_we),
        .mem_a_size(sysctrl_mem_a_size),
        .mem_a_valid(sysctrl_mem_a_valid),
        .mem_a_ready(sysctrl_mem_a_ready),
        .mem_d_rdata(sysctrl_mem_d_rdata),
        .mem_d_valid(sysctrl_mem_d_valid),
        .mem_d_ready(sysctrl_mem_d_ready),
        
        // System control outputs
        .sys_rst(sysctrl_sys_rst),
        .cpu_rst(sysctrl_cpu_rst),
        .cpu_boot_addr(sysctrl_cpu_boot_addr),
        .cpu_boot(sysctrl_cpu_boot),
        .req_cpu_halt(sysctrl_req_cpu_halt),
        .halted_value(sysctrl_halted_value),
        
        // CPU status inputs
        .cpu_halted(cpu_halted_internal),
        .cpu_booting(cpu_is_booting)
    );

    assign halted_value = sysctrl_halted_value;

    // ============================================================
    // System LED Controller Instantiation
    // ============================================================
    sys_led_controller #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ)
    ) sys_led_ctrl (
        .clk(clk),
        .rst(rst_internal),
        .cpu_booting(cpu_is_booting),
        .cpu_halted(cpu_halted_internal),
        .instr_complete(instr_complete),
        .sys_bus_handshake(sys_bus_handshake),
        .host_bus_rx_handshake(host_rx_valid & host_rx_ready),
        .host_bus_tx_handshake(host_tx_valid & host_tx_ready),
        .com_err(com_err),
        .sys_led(sys_led_out)
    );

endmodule
`default_nettype wire

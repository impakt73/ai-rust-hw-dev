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
    parameter bit ENABLE_SDRAM = 1'b0,
    parameter bit ENABLE_GFX2D = 1'b0,
    parameter bit ENABLE_AUDIOSYS = 1'b0,
    parameter bit ENABLE_APF_BUS_BRIDGE = 1'b0,
    parameter int unsigned INTERRUPT_CTRL_BASE_ADDR = 32'h4000_0000,
    parameter int unsigned INTERRUPT_CTRL_ADDR_SIZE = 32'h0000_0020,
    parameter int unsigned INTERRUPT_CTRL_NUM_SOURCES = 5,
    // System Clock Frequency (used by system controller elapsed-time registers)
    parameter int CLK_FREQ_HZ = 50_000_000,
    parameter int RESET_CYCLES = 8,      // Number of cycles to hold reset after release
    parameter int unsigned GFX2D_BASE_ADDR = 32'h3000_0000,
    parameter int unsigned GFX2D_ADDR_SIZE = 32'h0000_6400,
    parameter int unsigned SDRAM_BASE_ADDR = 32'h1000_0000,
    parameter int unsigned SDRAM_ADDR_SIZE = 32'h0400_0000,
    parameter int unsigned AUDIOSYS_BASE_ADDR = 32'h6000_0000,
    parameter int unsigned AUDIOSYS_ADDR_SIZE = 32'h0000_0020,
    parameter int unsigned AUDIOSYS_FIFO_DEPTH = 1024,
    parameter bit ENABLE_GAMEPAD = 1'b0,
    parameter int unsigned GAMEPAD_BASE_ADDR = 32'h5000_0000,
    parameter int unsigned GAMEPAD_ADDR_SIZE = 32'h0000_0010,
    parameter int unsigned GFX2D_VIDEO_ACTIVE_WIDTH = 256,
    parameter int unsigned GFX2D_VIDEO_ACTIVE_HEIGHT = 224,
    parameter int unsigned GFX2D_VIDEO_H_FRONT_PORCH = 10,
    parameter int unsigned GFX2D_VIDEO_H_SYNC_WIDTH = 1,
    parameter int unsigned GFX2D_VIDEO_H_BACK_PORCH = 133,
    parameter int unsigned GFX2D_VIDEO_V_FRONT_PORCH = 10,
    parameter int unsigned GFX2D_VIDEO_V_SYNC_WIDTH = 1,
    parameter int unsigned GFX2D_VIDEO_V_BACK_PORCH = 277,
    parameter bit GFX2D_VIDEO_HSYNC_ACTIVE_HIGH = 1'b1,
    parameter bit GFX2D_VIDEO_VSYNC_ACTIVE_HIGH = 1'b1,
    parameter int unsigned GFX2D_TILE_WIDTH = 8,
    parameter int unsigned GFX2D_TILE_HEIGHT = 8,
    parameter int unsigned GFX2D_TILE_COLUMNS = 32,
    parameter int unsigned GFX2D_TILE_ROWS = 32,
    parameter AUDIOSYS_INIT_FILE = "rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/sine_table_init.hex"
) (
    input wire logic        clk,
    input wire logic        video_clk,
    input wire logic        audio_clk,
    input wire logic        sdram_clk,
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
    output logic [31:0] halted_value,
    output logic [23:0] video_rgb,
    output logic        video_de,
    output logic        video_skip,
    output logic        video_vs,
    output logic        video_hs,
    output logic        audio_dac,
    output logic        audio_lrclk,
    output logic             sdram_word_rd,
    output logic             sdram_word_wr,
    output logic [23:0]      sdram_word_addr,
    output logic [31:0]      sdram_word_data,
    input  wire logic [31:0] sdram_word_q,
    input  wire logic        sdram_word_busy,

    // External CPU boot control (driven by board-specific logic; tie to 0 when unused)
    input  wire logic        ext_cpu_boot,
    input  wire logic [31:0] ext_cpu_boot_addr,

    // Gamepad input (active-high button state; ignored when ENABLE_GAMEPAD=0)
    input  wire logic [9:0]  gamepad_in,

    // Analogue Pocket bridge bus (ignored when ENABLE_APF_BUS_BRIDGE=0)
    input  wire logic [31:0] apf_bridge_addr,
    input  wire logic        apf_bridge_rd,
    output logic             apf_bridge_rd_ready,
    input  wire logic        apf_bridge_wr,
    output logic             apf_bridge_wr_ready,
    input  wire logic [31:0] apf_bridge_wr_data,
    output logic [31:0]      apf_bridge_rd_data
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
    // SDRAM Peripheral Interface Signals
    // ============================================================
    logic [31:0] sdram_mem_a_addr;
    logic [31:0] sdram_mem_a_wdata;
    logic        sdram_mem_a_we;
    logic [1:0]  sdram_mem_a_size;
    logic        sdram_mem_a_valid;
    logic        sdram_mem_a_ready;
    logic [31:0] sdram_mem_d_rdata;
    logic        sdram_mem_d_valid;
    logic        sdram_mem_d_ready;
    
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
    logic        external_meip;
    logic [INTERRUPT_CTRL_NUM_SOURCES-1:0] external_irq_sources;
    localparam int unsigned AUDIOSYS_IRQ_SOURCE_INDEX = 4;
    localparam int unsigned AUDIOSYS_IRQ_SOURCE_COUNT_MIN = AUDIOSYS_IRQ_SOURCE_INDEX + 1;

    initial begin
        if (ENABLE_AUDIOSYS && (INTERRUPT_CTRL_NUM_SOURCES < AUDIOSYS_IRQ_SOURCE_COUNT_MIN)) begin
            $fatal(
                1,
                "top: audiosys interrupt wiring requires INTERRUPT_CTRL_NUM_SOURCES >= %0d, got %0d",
                AUDIOSYS_IRQ_SOURCE_COUNT_MIN,
                INTERRUPT_CTRL_NUM_SOURCES
            );
        end
    end
    // Slave 0 = system controller, slave 1 = SRAM, slave 2 = interrupt controller,
    // slave 3 = optional SDRAM,
    // and each enabled optional peripheral shifts the later indices forward.
    localparam int unsigned INTC_SLAVE_INDEX = 2;
    localparam int unsigned SDRAM_SLAVE_INDEX = 3;
    localparam int unsigned GFX2D_SLAVE_INDEX = 3 + (ENABLE_SDRAM ? 1 : 0);
    localparam int unsigned AUDIOSYS_SLAVE_INDEX =
        GFX2D_SLAVE_INDEX + (ENABLE_GFX2D ? 1 : 0);
    localparam int unsigned GAMEPAD_SLAVE_INDEX =
        AUDIOSYS_SLAVE_INDEX + (ENABLE_AUDIOSYS ? 1 : 0);
    localparam int unsigned NUM_RTL_SLAVES =
        3
        + (ENABLE_SDRAM ? 1 : 0)
        + (ENABLE_GFX2D ? 1 : 0)
        + (ENABLE_AUDIOSYS ? 1 : 0)
        + (ENABLE_GAMEPAD ? 1 : 0);

    // ============================================================
    // External Interrupt Controller Interface Signals
    // ============================================================
    logic [31:0] intc_mem_a_addr;
    logic [31:0] intc_mem_a_wdata;
    logic        intc_mem_a_we;
    logic [1:0]  intc_mem_a_size;
    logic        intc_mem_a_valid;
    logic        intc_mem_a_ready;
    logic [31:0] intc_mem_d_rdata;
    logic        intc_mem_d_valid;
    logic        intc_mem_d_ready;

    // ============================================================
    // GFX2D Peripheral Interface Signals
    // ============================================================
    logic [31:0] gfx2d_mem_a_addr;
    logic [31:0] gfx2d_mem_a_wdata;
    logic        gfx2d_mem_a_we;
    logic [1:0]  gfx2d_mem_a_size;
    logic        gfx2d_mem_a_valid;
    logic        gfx2d_mem_a_ready;
    logic [31:0] gfx2d_mem_d_rdata;
    logic        gfx2d_mem_d_valid;
    logic        gfx2d_mem_d_ready;

    // ============================================================
    // Audiosys Peripheral Interface Signals
    // ============================================================
    logic [31:0] audiosys_mem_a_addr;
    logic [31:0] audiosys_mem_a_wdata;
    logic        audiosys_mem_a_we;
    logic [1:0]  audiosys_mem_a_size;
    logic        audiosys_mem_a_valid;
    logic        audiosys_mem_a_ready;
    logic [31:0] audiosys_mem_d_rdata;
    logic        audiosys_mem_d_valid;
    logic        audiosys_mem_d_ready;
    logic        audiosys_fifo_low_water_irq;
    
    // ============================================================
    // Gamepad Peripheral Interface Signals
    // ============================================================
    logic [31:0] gamepad_mem_a_addr;
    logic [31:0] gamepad_mem_a_wdata;
    logic        gamepad_mem_a_we;
    logic [1:0]  gamepad_mem_a_size;
    logic        gamepad_mem_a_valid;
    logic        gamepad_mem_a_ready;
    logic [31:0] gamepad_mem_d_rdata;
    logic        gamepad_mem_d_valid;
    logic        gamepad_mem_d_ready;
    
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

    logic [31:0] apf_bus_mem_a_addr;
    logic [31:0] apf_bus_mem_a_wdata;
    logic        apf_bus_mem_a_we;
    logic [1:0]  apf_bus_mem_a_size;
    logic        apf_bus_mem_a_valid;
    logic        apf_bus_mem_a_ready;
    logic [31:0] apf_bus_mem_d_rdata;
    logic        apf_bus_mem_d_valid;
    logic        apf_bus_mem_d_ready;

    // registered_bus uses fixed-priority arbitration: host stays highest priority,
    // the optional APF bridge sits ahead of the CPU path, and the CPU remains lowest.
    localparam int unsigned NON_APF_MASTER_COUNT = 2;
    localparam int unsigned HOST_RTL_MASTER_INDEX = 0;
    localparam int unsigned APF_BRIDGE_MASTER_INDEX = 1;
    localparam int unsigned CPU_RTL_MASTER_INDEX = ENABLE_APF_BUS_BRIDGE ? 2 : 1;
    localparam int unsigned HOST_RTL_MASTER_ADDR_BIT_OFFSET = 32 * HOST_RTL_MASTER_INDEX;
    localparam int unsigned HOST_RTL_MASTER_SIZE_BIT_OFFSET = 2 * HOST_RTL_MASTER_INDEX;
    localparam int unsigned APF_BRIDGE_MASTER_ADDR_BIT_OFFSET = 32 * APF_BRIDGE_MASTER_INDEX;
    localparam int unsigned APF_BRIDGE_MASTER_SIZE_BIT_OFFSET = 2 * APF_BRIDGE_MASTER_INDEX;
    localparam int unsigned CPU_RTL_MASTER_ADDR_BIT_OFFSET = 32 * CPU_RTL_MASTER_INDEX;
    localparam int unsigned CPU_RTL_MASTER_SIZE_BIT_OFFSET = 2 * CPU_RTL_MASTER_INDEX;
    localparam int unsigned NUM_RTL_MASTERS =
        NON_APF_MASTER_COUNT + (ENABLE_APF_BUS_BRIDGE ? 1 : 0);

    logic [NUM_RTL_MASTERS*32-1:0] registered_master_mem_a_addr;
    logic [NUM_RTL_MASTERS*32-1:0] registered_master_mem_a_wdata;
    logic [NUM_RTL_MASTERS-1:0]    registered_master_mem_a_we;
    logic [NUM_RTL_MASTERS*2-1:0]  registered_master_mem_a_size;
    logic [NUM_RTL_MASTERS-1:0]    registered_master_mem_a_valid;
    logic [NUM_RTL_MASTERS-1:0]    registered_master_mem_a_ready;
    logic [NUM_RTL_MASTERS*32-1:0] registered_master_mem_d_rdata;
    logic [NUM_RTL_MASTERS-1:0]    registered_master_mem_d_valid;
    logic [NUM_RTL_MASTERS-1:0]    registered_master_mem_d_ready;

    logic [NUM_RTL_SLAVES*32-1:0] registered_slave_base_addr;
    logic [NUM_RTL_SLAVES*32-1:0] registered_slave_addr_size;
    logic [NUM_RTL_SLAVES*32-1:0] registered_slave_mem_a_addr;
    logic [NUM_RTL_SLAVES*32-1:0] registered_slave_mem_a_wdata;
    logic [NUM_RTL_SLAVES-1:0]    registered_slave_mem_a_we;
    logic [NUM_RTL_SLAVES*2-1:0]  registered_slave_mem_a_size;
    logic [NUM_RTL_SLAVES-1:0]    registered_slave_mem_a_valid;
    logic [NUM_RTL_SLAVES-1:0]    registered_slave_mem_a_ready;
    logic [NUM_RTL_SLAVES*32-1:0] registered_slave_mem_d_rdata;
    logic [NUM_RTL_SLAVES-1:0]    registered_slave_mem_d_valid;
    logic [NUM_RTL_SLAVES-1:0]    registered_slave_mem_d_ready;

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

    assign registered_master_mem_a_addr[HOST_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32] = host_mem_a_addr;
    assign registered_master_mem_a_wdata[HOST_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32] =
        host_mem_a_wdata;
    assign registered_master_mem_a_we[HOST_RTL_MASTER_INDEX] = host_mem_a_we;
    assign registered_master_mem_a_size[HOST_RTL_MASTER_SIZE_BIT_OFFSET+1 -: 2] = host_mem_a_size;
    assign registered_master_mem_a_valid[HOST_RTL_MASTER_INDEX] = host_mem_a_valid;
    assign host_mem_a_ready = registered_master_mem_a_ready[HOST_RTL_MASTER_INDEX];
    assign host_mem_d_rdata =
        registered_master_mem_d_rdata[HOST_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32];
    assign host_mem_d_valid = registered_master_mem_d_valid[HOST_RTL_MASTER_INDEX];
    assign registered_master_mem_d_ready[HOST_RTL_MASTER_INDEX] = host_mem_d_ready;

    assign registered_master_mem_a_addr[CPU_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32] = cpu_to_arb_a_addr;
    assign registered_master_mem_a_wdata[CPU_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32] =
        cpu_to_arb_a_wdata;
    assign registered_master_mem_a_we[CPU_RTL_MASTER_INDEX] = cpu_to_arb_a_we;
    assign registered_master_mem_a_size[CPU_RTL_MASTER_SIZE_BIT_OFFSET+1 -: 2] = cpu_to_arb_a_size;
    assign registered_master_mem_a_valid[CPU_RTL_MASTER_INDEX] = cpu_to_arb_a_valid;
    assign cpu_to_arb_a_ready = registered_master_mem_a_ready[CPU_RTL_MASTER_INDEX];
    assign cpu_to_arb_d_rdata =
        registered_master_mem_d_rdata[CPU_RTL_MASTER_ADDR_BIT_OFFSET+31 -: 32];
    assign cpu_to_arb_d_valid = registered_master_mem_d_valid[CPU_RTL_MASTER_INDEX];
    assign registered_master_mem_d_ready[CPU_RTL_MASTER_INDEX] = cpu_to_arb_d_ready;

    generate
        if (ENABLE_APF_BUS_BRIDGE) begin : gen_apf_bridge_master_wiring
            assign registered_master_mem_a_addr[APF_BRIDGE_MASTER_ADDR_BIT_OFFSET+31 -: 32] =
                apf_bus_mem_a_addr;
            assign registered_master_mem_a_wdata[APF_BRIDGE_MASTER_ADDR_BIT_OFFSET+31 -: 32] =
                apf_bus_mem_a_wdata;
            assign registered_master_mem_a_we[APF_BRIDGE_MASTER_INDEX] = apf_bus_mem_a_we;
            assign registered_master_mem_a_size[APF_BRIDGE_MASTER_SIZE_BIT_OFFSET+1 -: 2] =
                apf_bus_mem_a_size;
            assign registered_master_mem_a_valid[APF_BRIDGE_MASTER_INDEX] = apf_bus_mem_a_valid;
            assign apf_bus_mem_a_ready =
                registered_master_mem_a_ready[APF_BRIDGE_MASTER_INDEX];
            assign apf_bus_mem_d_rdata =
                registered_master_mem_d_rdata[APF_BRIDGE_MASTER_ADDR_BIT_OFFSET+31 -: 32];
            assign apf_bus_mem_d_valid =
                registered_master_mem_d_valid[APF_BRIDGE_MASTER_INDEX];
            assign registered_master_mem_d_ready[APF_BRIDGE_MASTER_INDEX] = apf_bus_mem_d_ready;
        end else begin : gen_apf_bridge_master_disabled
            assign apf_bus_mem_a_ready = 1'b0;
            assign apf_bus_mem_d_rdata = 32'h0000_0000;
            assign apf_bus_mem_d_valid = 1'b0;
        end
    endgenerate

    assign registered_slave_base_addr[31:0] = 32'h2000_0000;
    assign registered_slave_addr_size[31:0] = 32'h0000_0028;
    assign registered_slave_base_addr[63:32] = 32'h7000_0000;
    assign registered_slave_addr_size[63:32] = 32'h0000_3000;
    assign registered_slave_base_addr[(32*INTC_SLAVE_INDEX)+31 -: 32] = INTERRUPT_CTRL_BASE_ADDR;
    assign registered_slave_addr_size[(32*INTC_SLAVE_INDEX)+31 -: 32] = INTERRUPT_CTRL_ADDR_SIZE;
    // Packed registered_bus slave vectors use slice N at bits [32N+31:32N] for
    // addresses/data and bits [2N+1:2N] for transfer size.
    generate
        if (ENABLE_SDRAM) begin : gen_sdram_bus_map
            assign registered_slave_base_addr[(32*SDRAM_SLAVE_INDEX)+31 -: 32] = SDRAM_BASE_ADDR;
            assign registered_slave_addr_size[(32*SDRAM_SLAVE_INDEX)+31 -: 32] = SDRAM_ADDR_SIZE;
        end
    endgenerate

    generate
        if (ENABLE_GFX2D) begin : gen_gfx2d_bus_map
            assign registered_slave_base_addr[(32*GFX2D_SLAVE_INDEX)+31 -: 32] = GFX2D_BASE_ADDR;
            assign registered_slave_addr_size[(32*GFX2D_SLAVE_INDEX)+31 -: 32] = GFX2D_ADDR_SIZE;
        end
    endgenerate

    generate
        if (ENABLE_AUDIOSYS) begin : gen_audiosys_bus_map
            assign registered_slave_base_addr[(32*AUDIOSYS_SLAVE_INDEX)+31 -: 32] = AUDIOSYS_BASE_ADDR;
            assign registered_slave_addr_size[(32*AUDIOSYS_SLAVE_INDEX)+31 -: 32] = AUDIOSYS_ADDR_SIZE;
        end
    endgenerate

    generate
        if (ENABLE_GAMEPAD) begin : gen_gamepad_bus_map
            assign registered_slave_base_addr[(32*GAMEPAD_SLAVE_INDEX)+31 -: 32] = GAMEPAD_BASE_ADDR;
            assign registered_slave_addr_size[(32*GAMEPAD_SLAVE_INDEX)+31 -: 32] = GAMEPAD_ADDR_SIZE;
        end
    endgenerate

    assign sysctrl_mem_a_addr = registered_slave_mem_a_addr[31:0];
    assign sysctrl_mem_a_wdata = registered_slave_mem_a_wdata[31:0];
    assign sysctrl_mem_a_we = registered_slave_mem_a_we[0];
    assign sysctrl_mem_a_size = registered_slave_mem_a_size[1:0];
    assign sysctrl_mem_a_valid = registered_slave_mem_a_valid[0];
    assign registered_slave_mem_a_ready[0] = sysctrl_mem_a_ready;
    assign registered_slave_mem_d_rdata[31:0] = sysctrl_mem_d_rdata;
    assign registered_slave_mem_d_valid[0] = sysctrl_mem_d_valid;
    assign sysctrl_mem_d_ready = registered_slave_mem_d_ready[0];

    assign sram_mem_a_addr = registered_slave_mem_a_addr[63:32];
    assign sram_mem_a_wdata = registered_slave_mem_a_wdata[63:32];
    assign sram_mem_a_we = registered_slave_mem_a_we[1];
    assign sram_mem_a_size = registered_slave_mem_a_size[3:2];
    assign sram_mem_a_valid = registered_slave_mem_a_valid[1];
    assign registered_slave_mem_a_ready[1] = sram_mem_a_ready;
    assign registered_slave_mem_d_rdata[63:32] = sram_mem_d_rdata;
    assign registered_slave_mem_d_valid[1] = sram_mem_d_valid;
    assign sram_mem_d_ready = registered_slave_mem_d_ready[1];

    assign intc_mem_a_addr = registered_slave_mem_a_addr[(32*INTC_SLAVE_INDEX)+31 -: 32];
    assign intc_mem_a_wdata = registered_slave_mem_a_wdata[(32*INTC_SLAVE_INDEX)+31 -: 32];
    assign intc_mem_a_we = registered_slave_mem_a_we[INTC_SLAVE_INDEX];
    assign intc_mem_a_size = registered_slave_mem_a_size[(2*INTC_SLAVE_INDEX)+1 -: 2];
    assign intc_mem_a_valid = registered_slave_mem_a_valid[INTC_SLAVE_INDEX];
    assign registered_slave_mem_a_ready[INTC_SLAVE_INDEX] = intc_mem_a_ready;
    assign registered_slave_mem_d_rdata[(32*INTC_SLAVE_INDEX)+31 -: 32] = intc_mem_d_rdata;
    assign registered_slave_mem_d_valid[INTC_SLAVE_INDEX] = intc_mem_d_valid;
    assign intc_mem_d_ready = registered_slave_mem_d_ready[INTC_SLAVE_INDEX];

    generate
        if (ENABLE_SDRAM) begin : gen_sdram_bus_wiring
            assign sdram_mem_a_addr = registered_slave_mem_a_addr[(32*SDRAM_SLAVE_INDEX)+31 -: 32];
            assign sdram_mem_a_wdata = registered_slave_mem_a_wdata[(32*SDRAM_SLAVE_INDEX)+31 -: 32];
            assign sdram_mem_a_we = registered_slave_mem_a_we[SDRAM_SLAVE_INDEX];
            assign sdram_mem_a_size = registered_slave_mem_a_size[(2*SDRAM_SLAVE_INDEX)+1 -: 2];
            assign sdram_mem_a_valid = registered_slave_mem_a_valid[SDRAM_SLAVE_INDEX];
            assign registered_slave_mem_a_ready[SDRAM_SLAVE_INDEX] = sdram_mem_a_ready;
            assign registered_slave_mem_d_rdata[(32*SDRAM_SLAVE_INDEX)+31 -: 32] =
                sdram_mem_d_rdata;
            assign registered_slave_mem_d_valid[SDRAM_SLAVE_INDEX] = sdram_mem_d_valid;
            assign sdram_mem_d_ready = registered_slave_mem_d_ready[SDRAM_SLAVE_INDEX];
        end else begin : gen_sdram_bus_disabled
            assign sdram_mem_a_addr = 32'h0000_0000;
            assign sdram_mem_a_wdata = 32'h0000_0000;
            assign sdram_mem_a_we = 1'b0;
            assign sdram_mem_a_size = 2'b00;
            assign sdram_mem_a_valid = 1'b0;
            assign sdram_mem_d_ready = 1'b0;
        end
    endgenerate

    generate
        if (ENABLE_GFX2D) begin : gen_gfx2d_bus_wiring
            assign gfx2d_mem_a_addr = registered_slave_mem_a_addr[(32*GFX2D_SLAVE_INDEX)+31 -: 32];
            assign gfx2d_mem_a_wdata = registered_slave_mem_a_wdata[(32*GFX2D_SLAVE_INDEX)+31 -: 32];
            assign gfx2d_mem_a_we = registered_slave_mem_a_we[GFX2D_SLAVE_INDEX];
            assign gfx2d_mem_a_size = registered_slave_mem_a_size[(2*GFX2D_SLAVE_INDEX)+1 -: 2];
            assign gfx2d_mem_a_valid = registered_slave_mem_a_valid[GFX2D_SLAVE_INDEX];
            assign registered_slave_mem_a_ready[GFX2D_SLAVE_INDEX] = gfx2d_mem_a_ready;
            assign registered_slave_mem_d_rdata[(32*GFX2D_SLAVE_INDEX)+31 -: 32] = gfx2d_mem_d_rdata;
            assign registered_slave_mem_d_valid[GFX2D_SLAVE_INDEX] = gfx2d_mem_d_valid;
            assign gfx2d_mem_d_ready = registered_slave_mem_d_ready[GFX2D_SLAVE_INDEX];
        end else begin : gen_gfx2d_bus_disabled
            assign gfx2d_mem_a_addr = 32'h0000_0000;
            assign gfx2d_mem_a_wdata = 32'h0000_0000;
            assign gfx2d_mem_a_we = 1'b0;
            assign gfx2d_mem_a_size = 2'b00;
            assign gfx2d_mem_a_valid = 1'b0;
            assign gfx2d_mem_d_ready = 1'b0;
        end
    endgenerate

    generate
        if (ENABLE_AUDIOSYS) begin : gen_audiosys_bus_wiring
            assign audiosys_mem_a_addr = registered_slave_mem_a_addr[(32*AUDIOSYS_SLAVE_INDEX)+31 -: 32];
            assign audiosys_mem_a_wdata = registered_slave_mem_a_wdata[(32*AUDIOSYS_SLAVE_INDEX)+31 -: 32];
            assign audiosys_mem_a_we = registered_slave_mem_a_we[AUDIOSYS_SLAVE_INDEX];
            assign audiosys_mem_a_size = registered_slave_mem_a_size[(2*AUDIOSYS_SLAVE_INDEX)+1 -: 2];
            assign audiosys_mem_a_valid = registered_slave_mem_a_valid[AUDIOSYS_SLAVE_INDEX];
            assign registered_slave_mem_a_ready[AUDIOSYS_SLAVE_INDEX] = audiosys_mem_a_ready;
            assign registered_slave_mem_d_rdata[(32*AUDIOSYS_SLAVE_INDEX)+31 -: 32] = audiosys_mem_d_rdata;
            assign registered_slave_mem_d_valid[AUDIOSYS_SLAVE_INDEX] = audiosys_mem_d_valid;
            assign audiosys_mem_d_ready = registered_slave_mem_d_ready[AUDIOSYS_SLAVE_INDEX];
        end else begin : gen_audiosys_bus_disabled
            assign audiosys_mem_a_addr = 32'h0000_0000;
            assign audiosys_mem_a_wdata = 32'h0000_0000;
            assign audiosys_mem_a_we = 1'b0;
            assign audiosys_mem_a_size = 2'b00;
            assign audiosys_mem_a_valid = 1'b0;
            assign audiosys_mem_d_ready = 1'b0;
        end
    endgenerate

    generate
        if (ENABLE_GAMEPAD) begin : gen_gamepad_bus_wiring
            assign gamepad_mem_a_addr  = registered_slave_mem_a_addr[(32*GAMEPAD_SLAVE_INDEX)+31 -: 32];
            assign gamepad_mem_a_wdata = registered_slave_mem_a_wdata[(32*GAMEPAD_SLAVE_INDEX)+31 -: 32];
            assign gamepad_mem_a_we    = registered_slave_mem_a_we[GAMEPAD_SLAVE_INDEX];
            assign gamepad_mem_a_size  = registered_slave_mem_a_size[(2*GAMEPAD_SLAVE_INDEX)+1 -: 2];
            assign gamepad_mem_a_valid = registered_slave_mem_a_valid[GAMEPAD_SLAVE_INDEX];
            assign registered_slave_mem_a_ready[GAMEPAD_SLAVE_INDEX]              = gamepad_mem_a_ready;
            assign registered_slave_mem_d_rdata[(32*GAMEPAD_SLAVE_INDEX)+31 -: 32] = gamepad_mem_d_rdata;
            assign registered_slave_mem_d_valid[GAMEPAD_SLAVE_INDEX]              = gamepad_mem_d_valid;
            assign gamepad_mem_d_ready = registered_slave_mem_d_ready[GAMEPAD_SLAVE_INDEX];
        end else begin : gen_gamepad_bus_disabled
            assign gamepad_mem_a_addr  = 32'h0000_0000;
            assign gamepad_mem_a_wdata = 32'h0000_0000;
            assign gamepad_mem_a_we    = 1'b0;
            assign gamepad_mem_a_size  = 2'b00;
            assign gamepad_mem_a_valid = 1'b0;
            assign gamepad_mem_d_ready = 1'b0;
        end
    endgenerate

    assign sys_bus_handshake =
        (host_mem_d_valid && host_mem_d_ready) ||
        (cpu_to_arb_d_valid && cpu_to_arb_d_ready);

    // ============================================================
    // Registered Bus Instantiation
    // ============================================================
    registered_bus #(
        .NUM_MASTERS(NUM_RTL_MASTERS),
        .NUM_SLAVES(NUM_RTL_SLAVES)
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
        .msip(1'b0),
        .mtip(1'b0),
        .meip(external_meip),
        
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

    generate
        if (ENABLE_APF_BUS_BRIDGE) begin : gen_apf_bus_bridge
            apf_bus_bridge apf_bridge (
                .clk(clk),
                .rst(rst_internal),
                .bridge_addr(apf_bridge_addr),
                .bridge_rd(apf_bridge_rd),
                .bridge_rd_ready(apf_bridge_rd_ready),
                .bridge_wr(apf_bridge_wr),
                .bridge_wr_ready(apf_bridge_wr_ready),
                .bridge_wr_data(apf_bridge_wr_data),
                .bridge_rd_data(apf_bridge_rd_data),
                .mem_a_addr(apf_bus_mem_a_addr),
                .mem_a_wdata(apf_bus_mem_a_wdata),
                .mem_a_we(apf_bus_mem_a_we),
                .mem_a_size(apf_bus_mem_a_size),
                .mem_a_valid(apf_bus_mem_a_valid),
                .mem_a_ready(apf_bus_mem_a_ready),
                .mem_d_rdata(apf_bus_mem_d_rdata),
                .mem_d_valid(apf_bus_mem_d_valid),
                .mem_d_ready(apf_bus_mem_d_ready)
            );
        end else begin : gen_no_apf_bus_bridge
            assign apf_bus_mem_a_addr = 32'h0000_0000;
            assign apf_bus_mem_a_wdata = 32'h0000_0000;
            assign apf_bus_mem_a_we = 1'b0;
            assign apf_bus_mem_a_size = 2'b00;
            assign apf_bus_mem_a_valid = 1'b0;
            assign apf_bus_mem_d_ready = 1'b0;
            assign apf_bridge_rd_ready = 1'b0;
            assign apf_bridge_wr_ready = 1'b0;
            assign apf_bridge_rd_data = 32'h0000_0000;
        end
    endgenerate
    
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

    always_comb begin
        external_irq_sources = '0;
        if (ENABLE_AUDIOSYS && (INTERRUPT_CTRL_NUM_SOURCES > AUDIOSYS_IRQ_SOURCE_INDEX)) begin
            external_irq_sources[AUDIOSYS_IRQ_SOURCE_INDEX] = audiosys_fifo_low_water_irq;
        end
    end

    external_interrupt_controller #(
        .NUM_SOURCES(INTERRUPT_CTRL_NUM_SOURCES)
    ) external_interrupt_ctrl (
        .clk(clk),
        .rst(rst_internal),
        .irq_sources(external_irq_sources),
        .mem_a_addr(intc_mem_a_addr),
        .mem_a_wdata(intc_mem_a_wdata),
        .mem_a_we(intc_mem_a_we),
        .mem_a_size(intc_mem_a_size),
        .mem_a_valid(intc_mem_a_valid),
        .mem_a_ready(intc_mem_a_ready),
        .mem_d_rdata(intc_mem_d_rdata),
        .mem_d_valid(intc_mem_d_valid),
        .mem_d_ready(intc_mem_d_ready),
        .meip(external_meip)
    );

    generate
        if (ENABLE_SDRAM) begin : gen_sdram_peripheral
            sdram_peripheral #(
                .BASE_ADDR(SDRAM_BASE_ADDR),
                .ADDR_SIZE(SDRAM_ADDR_SIZE)
            ) sdram_periph (
                .sys_clk(clk),
                .sdram_clk(sdram_clk),
                .rst(rst_internal),
                .mem_a_addr(sdram_mem_a_addr),
                .mem_a_wdata(sdram_mem_a_wdata),
                .mem_a_we(sdram_mem_a_we),
                .mem_a_size(sdram_mem_a_size),
                .mem_a_valid(sdram_mem_a_valid),
                .mem_a_ready(sdram_mem_a_ready),
                .mem_d_rdata(sdram_mem_d_rdata),
                .mem_d_valid(sdram_mem_d_valid),
                .mem_d_ready(sdram_mem_d_ready),
                .word_rd(sdram_word_rd),
                .word_wr(sdram_word_wr),
                .word_addr(sdram_word_addr),
                .word_data(sdram_word_data),
                .word_q(sdram_word_q),
                .word_busy(sdram_word_busy)
            );
        end else begin : gen_no_sdram_peripheral
            assign sdram_mem_a_ready = 1'b1;
            assign sdram_mem_d_rdata = 32'h0000_0000;
            assign sdram_mem_d_valid = 1'b0;
            assign sdram_word_rd = 1'b0;
            assign sdram_word_wr = 1'b0;
            assign sdram_word_addr = 24'h0;
            assign sdram_word_data = 32'h0000_0000;
        end
    endgenerate

    generate
        if (ENABLE_GFX2D) begin : gen_gfx2d_peripheral
            gfx2d_peripheral #(
                .VIDEO_ACTIVE_WIDTH(GFX2D_VIDEO_ACTIVE_WIDTH),
                .VIDEO_ACTIVE_HEIGHT(GFX2D_VIDEO_ACTIVE_HEIGHT),
                .VIDEO_H_FRONT_PORCH(GFX2D_VIDEO_H_FRONT_PORCH),
                .VIDEO_H_SYNC_WIDTH(GFX2D_VIDEO_H_SYNC_WIDTH),
                .VIDEO_H_BACK_PORCH(GFX2D_VIDEO_H_BACK_PORCH),
                .VIDEO_V_FRONT_PORCH(GFX2D_VIDEO_V_FRONT_PORCH),
                .VIDEO_V_SYNC_WIDTH(GFX2D_VIDEO_V_SYNC_WIDTH),
                .VIDEO_V_BACK_PORCH(GFX2D_VIDEO_V_BACK_PORCH),
                .VIDEO_HSYNC_ACTIVE_HIGH(GFX2D_VIDEO_HSYNC_ACTIVE_HIGH),
                .VIDEO_VSYNC_ACTIVE_HIGH(GFX2D_VIDEO_VSYNC_ACTIVE_HIGH),
                .TILE_WIDTH(GFX2D_TILE_WIDTH),
                .TILE_HEIGHT(GFX2D_TILE_HEIGHT),
                .TILE_COLUMNS(GFX2D_TILE_COLUMNS),
                .TILE_ROWS(GFX2D_TILE_ROWS)
            ) gfx2d_periph (
                .sys_clk(clk),
                .video_clk(video_clk),
                .rst(rst_internal),
                .mem_a_addr(gfx2d_mem_a_addr),
                .mem_a_wdata(gfx2d_mem_a_wdata),
                .mem_a_we(gfx2d_mem_a_we),
                .mem_a_size(gfx2d_mem_a_size),
                .mem_a_valid(gfx2d_mem_a_valid),
                .mem_a_ready(gfx2d_mem_a_ready),
                .mem_d_rdata(gfx2d_mem_d_rdata),
                .mem_d_valid(gfx2d_mem_d_valid),
                .mem_d_ready(gfx2d_mem_d_ready),
                .video_rgb(video_rgb),
                .video_de(video_de),
                .video_skip(video_skip),
                .video_vs(video_vs),
                .video_hs(video_hs)
            );
        end else begin : gen_no_gfx2d_peripheral
            assign gfx2d_mem_a_ready = 1'b1;
            assign gfx2d_mem_d_rdata = 32'h0000_0000;
            assign gfx2d_mem_d_valid = 1'b0;
            assign video_rgb = 24'h00_00_00;
            assign video_de = 1'b0;
            assign video_skip = 1'b0;
            assign video_vs = 1'b0;
            assign video_hs = 1'b0;
        end
    endgenerate

    generate
        if (ENABLE_AUDIOSYS) begin : gen_audiosys_peripheral
            audiosys_peripheral #(
                .AUDIO_FIFO_DEPTH(AUDIOSYS_FIFO_DEPTH),
                .INIT_FILE(AUDIOSYS_INIT_FILE)
            ) audiosys_periph (
                .sys_clk(clk),
                .audio_clk(audio_clk),
                .rst(rst_internal),
                .mem_a_addr(audiosys_mem_a_addr),
                .mem_a_wdata(audiosys_mem_a_wdata),
                .mem_a_we(audiosys_mem_a_we),
                .mem_a_size(audiosys_mem_a_size),
                .mem_a_valid(audiosys_mem_a_valid),
                .mem_a_ready(audiosys_mem_a_ready),
                .mem_d_rdata(audiosys_mem_d_rdata),
                .mem_d_valid(audiosys_mem_d_valid),
                .mem_d_ready(audiosys_mem_d_ready),
                .audio_dac(audio_dac),
                .audio_lrclk(audio_lrclk),
                .fifo_low_water_irq(audiosys_fifo_low_water_irq)
            );
        end else begin : gen_no_audiosys_peripheral
            assign audiosys_mem_a_ready = 1'b1;
            assign audiosys_mem_d_rdata = 32'h0000_0000;
            assign audiosys_mem_d_valid = 1'b0;
            assign audio_dac = 1'b0;
            assign audio_lrclk = 1'b0;
            assign audiosys_fifo_low_water_irq = 1'b0;
        end
    endgenerate

    generate
        if (ENABLE_GAMEPAD) begin : gen_gamepad_peripheral
            gamepad_peripheral gamepad_periph (
                .clk(clk),
                .rst(rst_internal),
                .gamepad_in(gamepad_in),
                .mem_a_addr(gamepad_mem_a_addr),
                .mem_a_wdata(gamepad_mem_a_wdata),
                .mem_a_we(gamepad_mem_a_we),
                .mem_a_size(gamepad_mem_a_size),
                .mem_a_valid(gamepad_mem_a_valid),
                .mem_a_ready(gamepad_mem_a_ready),
                .mem_d_rdata(gamepad_mem_d_rdata),
                .mem_d_valid(gamepad_mem_d_valid),
                .mem_d_ready(gamepad_mem_d_ready)
            );
        end else begin : gen_no_gamepad_peripheral
            assign gamepad_mem_a_ready = 1'b1;
            assign gamepad_mem_d_rdata = 32'h0000_0000;
            assign gamepad_mem_d_valid = 1'b0;
        end
    endgenerate
    
    // ============================================================
    // System Controller Instantiation
    // ============================================================
    system_controller #(
        .CLK_FREQ_HZ(CLK_FREQ_HZ)
    ) sysctrl (
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
        .led_out(led_out),
        
        // External boot inputs (board-level; tied low on non-Pocket targets)
        .ext_cpu_boot(ext_cpu_boot),
        .ext_cpu_boot_addr(ext_cpu_boot_addr),
        
        // CPU status inputs
        .cpu_halted(cpu_halted_internal),
        .cpu_booting(cpu_is_booting),
        .cpu_pc(debug_current_pc),
        .cpu_instr(debug_current_instruction)
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

// FPGA Top-Level Module for Alchitry Cu v1
// Wraps RISC-V CPU with host communication via USB serial
// Board: Alchitry Cu v1 (iCE40-HX8K-CB132)
// Configurable extension support for resource-constrained FPGA targets
//
// Architecture:
// - CPU communicates with host via serialized bus transactions
// - USB serial provides host communication channel via UART
// - Host computer handles external memory (DRAM) accesses
// - CPU's internal UART is looped back for self-test

module fpga_top #(
    parameter bit ENABLE_M_EXT = 1'b0,  // RV32M extension: Multiply/Divide (disabled for iCE40 resources)
    parameter bit ENABLE_F_EXT = 1'b0   // RV32F extension: Floating-Point (disabled for iCE40 resources)
) (
    // Clock input (100 MHz on-board oscillator)
    input  logic       clk,
    
    // Reset button (active low)
    input  logic       rst_n_btn,
    
    // LED outputs (8 LEDs on Alchitry Cu main board)
    output logic [7:0] led,
    
    // USB Serial
    input  logic       usb_rx,
    output logic       usb_tx,
    
    // IO Shield - LEDs (24 LEDs in 3 groups of 8)
    output logic [23:0] io_led,
    
    // IO Shield - DIP Switches (24 switches in 3 groups of 8)
    input  logic [23:0] io_dip,
    
    // IO Shield - Buttons (5 buttons, directly active high)
    input  logic [4:0]  io_button,
    
    // IO Shield - Seven-Segment Display (active low signals)
    output logic [3:0]  io_sel,   // Digit selection (active low: 0=enabled)
    output logic [7:0]  io_seg    // Segment outputs (active low: 0=lit)
);

    // ============================================================
    // PLL Configuration - Generate 25 MHz from 100 MHz input
    // ============================================================
    // Using iCE40 PLL to divide 100 MHz input to 25 MHz for timing closure
    // PLL parameters calculated for: 100 MHz input -> 25 MHz output
    // DIVR = 0, DIVF = 7, DIVQ = 5 gives: 100 * (7+1) / (2^5) = 100 * 8 / 32 = 25 MHz
    
    logic pll_clk_global; // PLL output on global clock network (25 MHz)
    logic pll_locked;     // PLL lock indicator
    
    SB_PLL40_CORE #(
        .FEEDBACK_PATH("SIMPLE"),
        .DIVR(4'b0000),        // DIVR = 0
        .DIVF(7'b0000111),     // DIVF = 7
        .DIVQ(3'b101),         // DIVQ = 5 (divide by 32)
        .FILTER_RANGE(3'b001)  // Filter range for 100 MHz input
    ) pll_inst (
        .REFERENCECLK(clk),
        .PLLOUTCORE(),         // Unused - use global network instead
        .PLLOUTGLOBAL(pll_clk_global),  // Drive system clock via global clock network
        .LOCK(pll_locked),
        .BYPASS(1'b0),
        .RESETB(1'b1)
    );
    
    // Use PLL global clock output for all internal logic
    // This reduces clock skew and improves timing closure
    logic sys_clk;
    assign sys_clk = pll_clk_global;
    
    // ============================================================
    // Reset Synchronization and Power-On Reset Controller
    // ============================================================
    logic rst_n;
    
    // Synchronize reset button to PLL clock domain (2-FF synchronizer)
    // Also incorporate PLL lock status into reset
    logic rst_n_sync1, rst_n_sync2;
    always_ff @(posedge sys_clk) begin
        rst_n_sync1 <= rst_n_btn & pll_locked;
        rst_n_sync2 <= rst_n_sync1;
    end
    
    // Reset controller for robust power-on reset
    // Holds reset asserted for RESET_CYCLES after input reset deasserts
    // Also supports soft reset requests from on-board logic
    logic reset_request;  // Soft reset request (active high) - currently unused
    assign reset_request = 1'b0;  // No soft reset source connected yet
    
    reset_controller #(
        .RESET_CYCLES(8)
    ) reset_ctrl (
        .clk(sys_clk),
        .rst_n_in(rst_n_sync2),
        .reset_request(reset_request),
        .rst_n_out(rst_n)
    );
    
    // ============================================================
    // Boot Configuration
    // ============================================================
    // Boot address: Start of instruction memory (DRAM base)
    // Memory is accessed via host computer through USB serial
    localparam logic [31:0] BOOT_ADDR = 32'h80000000;
    
    // ============================================================
    // Host Bus Interface Signals
    // ============================================================
    // Serialized bus transactions between CPU and host UART
    logic [7:0] host_tx_data;
    logic       host_tx_valid;
    logic       host_tx_ready;
    logic [7:0] host_rx_data;
    logic       host_rx_valid;
    logic       host_rx_ready;
    
    // CPU's internal UART signals (for loopback)
    logic cpu_uart_tx;
    logic cpu_uart_rx;
    
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
    // CPU's internal UART is looped back for self-test
    // Memory access is handled via host bus interface through USB
    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .UART_CLK_FREQ_HZ(25_000_000),  // 25 MHz (PLL output)
        .UART_BAUD_RATE(115200),
        .ENABLE_UART_LOOPBACK(1'b0)     // Disable loopback - we handle it externally
    ) cpu_inst (
        .clk(sys_clk),
        .rst_n(rst_n),
        .boot_addr(BOOT_ADDR),
        
        // Host bus interface (serialized memory transactions)
        .host_tx_data(host_tx_data),
        .host_tx_valid(host_tx_valid),
        .host_tx_ready(host_tx_ready),
        .host_rx_data(host_rx_data),
        .host_rx_valid(host_rx_valid),
        .host_rx_ready(host_rx_ready),
        
        // LED peripheral
        .led_out(led_out),
        
        // CPU's internal UART - loopback for self-test
        .uart_tx(cpu_uart_tx),
        .uart_rx(cpu_uart_rx),
        
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
    // CPU Internal UART Loopback
    // ============================================================
    // Connect CPU's internal UART TX to RX for self-test
    assign cpu_uart_rx = cpu_uart_tx;
    
    // ============================================================
    // Host Communication UART
    // ============================================================
    // UART for host bus interface communication via USB serial
    // This UART converts between serial USB and the parallel
    // host_tx/host_rx byte stream interface
    
    // Host UART internal signals
    logic [31:0] host_uart_addr;
    logic [31:0] host_uart_wdata;
    logic [31:0] host_uart_rdata;
    logic        host_uart_we;
    logic [1:0]  host_uart_size;
    logic        host_uart_req;
    logic        host_uart_ready;
    
    // Host UART TX FIFO interface (CPU -> Host)
    // We write host_tx_data to the UART when host_tx_valid is high
    // UART TX ready is fed back to host_tx_ready
    
    // Host UART RX FIFO interface (Host -> CPU)
    // We read from UART and provide data via host_rx_data
    // host_rx_valid indicates data available, host_rx_ready is CPU's ack
    
    // For simplicity, we use direct UART TX/RX signals instead of the bus interface
    // The host_bus_interface in the top module expects a simple byte stream
    
    // UART signals for host communication
    logic host_uart_tx_out;
    logic host_uart_rx_in;
    
    // Connect to USB serial pins
    assign usb_tx = host_uart_tx_out;
    assign host_uart_rx_in = usb_rx;
    
    // Use UART for host communication
    // Note: The UART module has a bus interface, but we need a streaming interface.
    // We'll use a simple approach: drive the UART via its bus interface registers.
    
    // UART register addresses (relative to base)
    localparam UART_TXDATA_OFFSET = 8'h00;
    localparam UART_RXDATA_OFFSET = 8'h04;
    localparam UART_STATUS_OFFSET = 8'h08;
    
    // State machine for host UART communication
    typedef enum logic [2:0] {
        HOST_UART_IDLE,
        HOST_UART_TX_WRITE,
        HOST_UART_TX_WAIT,
        HOST_UART_RX_READ,
        HOST_UART_RX_WAIT
    } host_uart_state_t;
    
    host_uart_state_t host_uart_state;
    
    // Host UART state machine
    always_ff @(posedge sys_clk or negedge rst_n) begin
        if (!rst_n) begin
            host_uart_state <= HOST_UART_IDLE;
            host_tx_ready <= 1'b0;
            host_rx_valid <= 1'b0;
            host_rx_data <= 8'h0;
            host_uart_addr <= 32'h0;
            host_uart_wdata <= 32'h0;
            host_uart_we <= 1'b0;
            host_uart_req <= 1'b0;
        end else begin
            case (host_uart_state)
                HOST_UART_IDLE: begin
                    host_uart_req <= 1'b0;
                    
                    // Priority: TX has priority over RX
                    if (host_tx_valid && !host_tx_ready) begin
                        // CPU wants to send a byte - write to UART TXDATA
                        host_uart_addr <= {24'h0, UART_TXDATA_OFFSET};
                        host_uart_wdata <= {24'h0, host_tx_data};
                        host_uart_we <= 1'b1;
                        host_uart_req <= 1'b1;
                        host_uart_state <= HOST_UART_TX_WRITE;
                    end else if (!host_rx_valid || host_rx_ready) begin
                        // CPU is ready for data or we haven't provided any - check UART RX
                        host_uart_addr <= {24'h0, UART_RXDATA_OFFSET};
                        host_uart_we <= 1'b0;
                        host_uart_req <= 1'b1;
                        host_uart_state <= HOST_UART_RX_READ;
                        // Clear rx_valid until we have new data
                        if (host_rx_ready) begin
                            host_rx_valid <= 1'b0;
                        end
                    end
                end
                
                HOST_UART_TX_WRITE: begin
                    if (host_uart_ready) begin
                        host_tx_ready <= 1'b1;  // Acknowledge the byte
                        host_uart_req <= 1'b0;
                        host_uart_state <= HOST_UART_TX_WAIT;
                    end
                end
                
                HOST_UART_TX_WAIT: begin
                    // Wait one cycle for handshake
                    host_tx_ready <= 1'b0;
                    host_uart_state <= HOST_UART_IDLE;
                end
                
                HOST_UART_RX_READ: begin
                    if (host_uart_ready) begin
                        // Check if data was available (non-zero when RX FIFO not empty)
                        if (host_uart_rdata != 32'h0) begin
                            host_rx_data <= host_uart_rdata[7:0];
                            host_rx_valid <= 1'b1;
                        end
                        host_uart_req <= 1'b0;
                        host_uart_state <= HOST_UART_RX_WAIT;
                    end
                end
                
                HOST_UART_RX_WAIT: begin
                    // Wait for CPU to acknowledge
                    if (host_rx_ready) begin
                        host_rx_valid <= 1'b0;
                    end
                    host_uart_state <= HOST_UART_IDLE;
                end
                
                default: host_uart_state <= HOST_UART_IDLE;
            endcase
        end
    end
    
    // Host UART bus size (always byte access)
    assign host_uart_size = 2'b00;
    
    // Instantiate host communication UART
    uart #(
        .CLK_FREQ_HZ(25_000_000),  // 25 MHz (PLL output)
        .BAUD_RATE(115200),
        .FIFO_DEPTH(8)
    ) host_uart_inst (
        .clk(sys_clk),
        .rst_n(rst_n),
        
        // Bus slave interface (directly driven by state machine)
        .addr(host_uart_addr),
        .wdata(host_uart_wdata),
        .rdata(host_uart_rdata),
        .we(host_uart_we),
        .req(host_uart_req),
        .size(host_uart_size),
        .ready(host_uart_ready),
        
        // External UART pins - connected to USB serial
        .tx_out(host_uart_tx_out),
        .rx_in(host_uart_rx_in)
    );
    
    // ============================================================
    // LED Output Assignment
    // ============================================================
    assign led = led_out;
    
    // Assign 8-bit LED pattern to all 3 IO Shield LED groups
    assign io_led[7:0]   = led_out;
    assign io_led[15:8]  = led_out;
    assign io_led[23:16] = led_out;
    
    // ============================================================
    // Button Counter Logic (also increments on led_out changes)
    // ============================================================
    // Synchronize buttons to system clock domain (2-FF synchronizer)
    // Note: This is a simple demo implementation without debouncing.
    // For production use, add a debounce timer (~10-20ms stable period).
    logic [4:0] io_button_sync1, io_button_sync2;
    logic [4:0] io_button_prev;
    logic [7:0] led_out_prev;
    logic [7:0] button_counter;
    
    always_ff @(posedge sys_clk) begin
        if (!rst_n) begin
            io_button_sync1 <= 5'b0;
            io_button_sync2 <= 5'b0;
            io_button_prev  <= 5'b0;
            led_out_prev    <= 8'b0;
            button_counter  <= 8'b0;
        end else begin
            // 2-FF synchronizer for buttons
            io_button_sync1 <= io_button;
            io_button_sync2 <= io_button_sync1;
            
            // Edge detection: increment on any rising edge of any button
            io_button_prev <= io_button_sync2;
            
            // Track previous led_out value for change detection
            led_out_prev <= led_out;

            // Increment counter on button press OR led_out change
            if (|(io_button_sync2 & ~io_button_prev) || (led_out != led_out_prev)) begin
                button_counter <= button_counter + 8'd1;
            end
        end
    end
    
    // ============================================================
    // Seven-Segment Display - Rotating Segment Pattern
    // ============================================================
    // Hardware uses active-low signals:
    //   io_sel: 0 = digit enabled, 1 = digit disabled
    //   io_seg: 0 = segment lit, 1 = segment off
    // Segment layout:
    //       a(0)
    //      -----
    //  f(5)|     |b(1)
    //      --g(6)--
    //  e(4)|     |c(2)
    //      -----
    //       d(3)   .dp(7)
    //
    // The outer ring pattern (traveling around the edge clockwise):
    // Position 0: a, Position 1: b, Position 2: c,
    // Position 3: d, Position 4: e, Position 5: f
    
    // Enable all digits (active-low: output 0 to enable)
    assign io_sel = 4'b0000;
    
    // Rotating segment pattern - lights one outer segment at a time
    // Use lower 3 bits of counter, wrap at 6 for the 6 outer segments
    logic [2:0] seg_position;
    logic [7:0] seg_pattern;
    
    // Calculate position (0-5) for the 6 outer segments
    always_comb begin
        // Use modulo to wrap counter to 0-5 range for 6 outer segments
        seg_position = 3'(button_counter % 8'd6);
        
        // Generate pattern: only one segment lit (active-high internally)
        // Segments: a=0, b=1, c=2, d=3, e=4, f=5
        case (seg_position)
            3'd0: seg_pattern = 8'b00000001;  // a lit
            3'd1: seg_pattern = 8'b00000010;  // b lit
            3'd2: seg_pattern = 8'b00000100;  // c lit
            3'd3: seg_pattern = 8'b00001000;  // d lit
            3'd4: seg_pattern = 8'b00010000;  // e lit
            3'd5: seg_pattern = 8'b00100000;  // f lit
            default: seg_pattern = 8'b00000001;  // a lit (fallback)
        endcase
    end
    
    // Output inverted pattern (active-low: 0 = segment lit)
    assign io_seg = ~seg_pattern;

endmodule

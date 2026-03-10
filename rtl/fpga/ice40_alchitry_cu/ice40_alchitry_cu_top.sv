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

module ice40_alchitry_cu_top #(
    parameter bit ENABLE_M_EXT = 1'b0,  // RV32M extension: Multiply/Divide (disabled by default for iCE40 resources)
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
    logic rst_n_btn_sync2;
    // Keep synchronizer reset deasserted so it can safely sample the async button
    // even while downstream reset is asserted.
    ff_sync #(
        .STAGES(2),
        .WIDTH(1)
    ) rst_n_btn_sync_inst (
        .clk(clk),
        .rst_n(1'b1),
        .din(rst_n_btn),
        .dout(rst_n_btn_sync2)
    );
    
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
        .RESETB(rst_n_btn_sync2)
    );
    
    // Use PLL global clock output for all internal logic
    // This reduces clock skew and improves timing closure
    logic sys_clk;
    assign sys_clk = pll_clk_global;
    
    // Synchronize PLL lock to system clock domain (2-FF synchronizer)
    logic pll_locked_sync2;
    ff_sync #(
        .STAGES(2),
        .WIDTH(1)
    ) pll_locked_sync_inst (
        .clk(sys_clk),
        .rst_n(1'b1),
        .din(pll_locked),
        .dout(pll_locked_sync2)
    );
    
    // LED controller output
    logic [7:0]  led_out;
    
    // System LED output (from system controller)
    logic [7:0]  sys_led_out;
    logic rst_n_core;
    
    fpga_common_top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(25_000_000),
        .RESET_CYCLES(25_000_000)
    ) fpga_common_top_inst (
        .sys_clk(sys_clk),
        .rst_n(pll_locked_sync2),
        .usb_rx(usb_rx),
        .usb_tx(usb_tx),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .rst_n_core(rst_n_core)
    );
    
    // ============================================================
    // LED Output Assignment
    // ============================================================
    // Main board LEDs driven by system controller
    assign led = sys_led_out;
    
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
    logic [2:0] seg_position_reg;
    
    always_ff @(posedge sys_clk) begin
        if (!rst_n_core) begin
            io_button_sync1 <= 5'b0;
            io_button_sync2 <= 5'b0;
            io_button_prev  <= 5'b0;
            led_out_prev    <= 8'b0;
            seg_position_reg <= 3'b0;
        end else begin
            // 2-FF synchronizer for buttons
            io_button_sync1 <= io_button;
            io_button_sync2 <= io_button_sync1;
            
            // Edge detection: increment on any rising edge of any button
            io_button_prev <= io_button_sync2;
            
            // Track previous led_out value for change detection
            led_out_prev <= led_out;

            // Advance segment position on button press OR led_out change
            if ((|(io_button_sync2 & ~io_button_prev)) || (led_out != led_out_prev)) begin
                seg_position_reg <= (seg_position_reg == 3'd5) ? 3'd0 : (seg_position_reg + 3'd1);
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
    logic [7:0] seg_pattern;
    
    always_comb begin
        // Generate pattern: only one segment lit (active-high internally)
        // Segments: a=0, b=1, c=2, d=3, e=4, f=5
        case (seg_position_reg)
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

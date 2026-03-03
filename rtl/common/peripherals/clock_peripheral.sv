// Clock Peripheral
// Provides elapsed time counters since reset
// Memory-mapped at 0x51000000 in RTL peripheral address space
//
// FPGA-optimized design: uses cascaded counters with pulse signals
// instead of large counters and division operations.
//
// Minimum clock frequency: 1 MHz (enforced via assertion)
//
// Registers (all read-only):
//   0x00: ELAPSED_US - Elapsed time in microseconds
//   0x04: ELAPSED_MS - Elapsed time in milliseconds
//   0x08: ELAPSED_S  - Elapsed time in seconds

module clock_peripheral #(
    parameter int CLK_FREQ_HZ = 1_000_000  // Default 1 MHz (minimum supported)
) (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit)
    input  logic [31:0] wdata,     // Write data (ignored - read-only peripheral)
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable (ignored)
    input  logic        req,       // Memory request
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word)
    output logic        ready      // Operation complete (always ready)
);

    // ============================================================
    // Parameter Validation
    // ============================================================
    // Enforce minimum clock frequency of 1 MHz
    // This simplifies the design by ensuring 1 cycle = 1 µs at minimum
    initial begin
        if (CLK_FREQ_HZ < 1_000_000) begin
            $error("CLK_FREQ_HZ must be >= 1,000,000 (1 MHz). Got: %0d", CLK_FREQ_HZ);
        end
    end

    // Register offsets
    localparam ELAPSED_US_OFFSET = 32'h00;
    localparam ELAPSED_MS_OFFSET = 32'h04;
    localparam ELAPSED_S_OFFSET  = 32'h08;
    
    // Clock peripheral is single-cycle - always ready
    assign ready = 1'b1;
    
    // ============================================================
    // FPGA-Optimized Cascaded Counter Design
    // ============================================================
    // Uses small counters with pulse signals to avoid large counters
    // and division/multiplication operations.
    //
    // Architecture:
    // 1. Cycle counter counts up to CYCLES_PER_US, then pulses microsecond_elapsed
    // 2. Microsecond sub-counter counts 0-999, pulses millisecond_elapsed at 1000
    // 3. Millisecond sub-counter counts 0-999, increments seconds register at 1000
    
    // Calculate cycles per microsecond at compile time
    localparam int CYCLES_PER_US = CLK_FREQ_HZ / 1_000_000;
    
    // Calculate bit width needed for cycle counter
    // Need to count from 0 to (CYCLES_PER_US - 1)
    // $clog2(1) = 0, so use at least 1 bit
    localparam int CYCLE_COUNTER_WIDTH = (CYCLES_PER_US > 1) ? $clog2(CYCLES_PER_US) : 1;
    
    // ============================================================
    // Counter Registers
    // ============================================================
    
    // Cycle counter - counts cycles until one microsecond elapses
    logic [CYCLE_COUNTER_WIDTH-1:0] cycle_counter;
    
    // Microsecond sub-counter - counts 0-999 microseconds within a millisecond
    logic [9:0] us_sub_counter;
    
    // Millisecond sub-counter - counts 0-999 milliseconds within a second
    logic [9:0] ms_sub_counter;
    
    // Elapsed time registers (32-bit, read via memory interface)
    logic [31:0] elapsed_us;
    logic [31:0] elapsed_ms;
    logic [31:0] elapsed_s;
    
    // Pulse signals for cascading
    logic microsecond_elapsed;
    logic millisecond_elapsed;
    logic second_elapsed;
    
    // ============================================================
    // Cycle Counter - counts up to CYCLES_PER_US
    // ============================================================
    // When CYCLES_PER_US = 1 (at exactly 1 MHz), every cycle is a microsecond
    // When CYCLES_PER_US > 1 (at higher frequencies), count cycles until 1 µs
    generate
        if (CYCLES_PER_US == 1) begin : gen_1mhz
            // At exactly 1 MHz: 1 cycle = 1 microsecond
            // No need for cycle counter, every cycle triggers microsecond_elapsed
            assign microsecond_elapsed = 1'b1;
            
            // Keep cycle_counter at 0 (unused but declared for consistency)
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    cycle_counter <= '0;
                end else begin
                    cycle_counter <= '0;
                end
            end
        end else begin : gen_high_freq
            // At higher frequencies (> 1 MHz): count cycles until 1 µs
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    cycle_counter <= '0;
                end else if (cycle_counter >= CYCLE_COUNTER_WIDTH'(CYCLES_PER_US - 1)) begin
                    cycle_counter <= '0;
                end else begin
                    cycle_counter <= cycle_counter + 1'b1;
                end
            end
            
            // Microsecond elapsed when cycle counter wraps
            assign microsecond_elapsed = (cycle_counter >= CYCLE_COUNTER_WIDTH'(CYCLES_PER_US - 1));
        end
    endgenerate
    
    // ============================================================
    // Microseconds Elapsed Register
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            elapsed_us <= 32'h0;
        end else if (microsecond_elapsed) begin
            elapsed_us <= elapsed_us + 32'h1;
        end
    end
    
    // ============================================================
    // Microsecond Sub-Counter (0-999) for Millisecond Rollover
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            us_sub_counter <= 10'h0;
        end else if (microsecond_elapsed) begin
            if (us_sub_counter >= 10'd999) begin
                us_sub_counter <= 10'h0;
            end else begin
                us_sub_counter <= us_sub_counter + 10'h1;
            end
        end
    end
    
    // Millisecond elapsed when microsecond sub-counter wraps from 999 to 0
    assign millisecond_elapsed = microsecond_elapsed && (us_sub_counter >= 10'd999);
    
    // ============================================================
    // Milliseconds Elapsed Register
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            elapsed_ms <= 32'h0;
        end else if (millisecond_elapsed) begin
            elapsed_ms <= elapsed_ms + 32'h1;
        end
    end
    
    // ============================================================
    // Millisecond Sub-Counter (0-999) for Second Rollover
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            ms_sub_counter <= 10'h0;
        end else if (millisecond_elapsed) begin
            if (ms_sub_counter >= 10'd999) begin
                ms_sub_counter <= 10'h0;
            end else begin
                ms_sub_counter <= ms_sub_counter + 10'h1;
            end
        end
    end
    
    // Second elapsed when millisecond sub-counter wraps from 999 to 0
    assign second_elapsed = millisecond_elapsed && (ms_sub_counter >= 10'd999);
    
    // ============================================================
    // Seconds Elapsed Register
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            elapsed_s <= 32'h0;
        end else if (second_elapsed) begin
            elapsed_s <= elapsed_s + 32'h1;
        end
    end
    
    // ============================================================
    // Read Logic
    // ============================================================
    // Read-only peripheral - writes are ignored
    // verilator lint_off UNUSEDSIGNAL
    // Suppress warnings for unused write signals
    logic unused_we;
    logic [31:0] unused_wdata;
    logic [1:0] unused_size;
    assign unused_we = we;
    assign unused_wdata = wdata;
    assign unused_size = size;
    // verilator lint_on UNUSEDSIGNAL
    
    always_comb begin
        rdata = 32'h0;
        
        if (req && !we) begin
            // Read based on address offset
            case (addr[3:0])  // Use lower 4 bits for register offset
                ELAPSED_US_OFFSET[3:0]: rdata = elapsed_us;
                ELAPSED_MS_OFFSET[3:0]: rdata = elapsed_ms;
                ELAPSED_S_OFFSET[3:0]:  rdata = elapsed_s;
                default:                rdata = 32'h0;
            endcase
        end
    end

endmodule

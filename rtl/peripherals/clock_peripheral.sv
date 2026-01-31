// Clock Peripheral
// Provides elapsed time counters since reset
// Memory-mapped at 0x51000000 in RTL peripheral address space
//
// Registers (all read-only):
//   0x00: ELAPSED_US - Elapsed time in microseconds
//   0x04: ELAPSED_MS - Elapsed time in milliseconds
//   0x08: ELAPSED_S  - Elapsed time in seconds

module clock_peripheral #(
    parameter int CLK_FREQ_HZ = 1000  // Default 1 kHz for fast testbench testing
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

    // Register offsets
    localparam ELAPSED_US_OFFSET = 32'h00;
    localparam ELAPSED_MS_OFFSET = 32'h04;
    localparam ELAPSED_S_OFFSET  = 32'h08;
    
    // Clock peripheral is single-cycle - always ready
    assign ready = 1'b1;
    
    // ============================================================
    // Time Counter Logic
    // ============================================================
    
    // Cycle counter - counts clock cycles since reset (64-bit)
    logic [63:0] cycle_counter;
    
    // Derived time values (32-bit for memory-mapped access)
    logic [31:0] elapsed_us;
    logic [31:0] elapsed_ms;
    logic [31:0] elapsed_s;
    
    // Calculate cycles needed for each time unit
    // Use localparam for compile-time constants
    // Note: Using explicit 64-bit widths for Yosys compatibility
    localparam [63:0] CYCLES_PER_MS = 64'(CLK_FREQ_HZ) / 64'd1_000;
    localparam [63:0] CYCLES_PER_S  = 64'(CLK_FREQ_HZ);
    
    // Handle low frequency case (< 1 MHz) for CYCLES_PER_US
    // If CLK_FREQ_HZ < 1 MHz, we need to scale differently to avoid divide by zero
    generate
        if (CLK_FREQ_HZ >= 1_000_000) begin : gen_us_high_freq
            // Normal case: clock >= 1 MHz, divide to get microseconds
            localparam [63:0] CYCLES_PER_US = 64'(CLK_FREQ_HZ) / 64'd1_000_000;
            assign elapsed_us = 32'(cycle_counter / CYCLES_PER_US);
        end else begin : gen_us_low_freq
            // Low frequency case: multiply cycles to get microseconds
            // For 1 kHz: multiply cycles by 1000 to get microseconds
            localparam [63:0] US_SCALE = 64'd1_000_000 / 64'(CLK_FREQ_HZ);
            assign elapsed_us = 32'(cycle_counter * US_SCALE);
        end
    endgenerate
    
    // Milliseconds and seconds: division is safe when CLK_FREQ_HZ >= 1000
    // (the minimum sensible clock frequency)
    assign elapsed_ms = 32'(cycle_counter / CYCLES_PER_MS);
    assign elapsed_s  = 32'(cycle_counter / CYCLES_PER_S);
    
    // Cycle counter - increments every clock cycle
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            cycle_counter <= 64'h0;
        end else begin
            cycle_counter <= cycle_counter + 64'h1;
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

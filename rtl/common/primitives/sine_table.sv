`default_nettype none
// Pipelined Sine Table
// Generates signed samples of a full-period sine wave from a quarter-wave ROM.
//
// The full period is reconstructed from a quarter-wave ROM by exploiting
// the sine symmetry across the four quadrants:
//
//   Q0 (0   .. N/4-1): rising  0..+peak   - read ROM forward, no invert
//   Q1 (N/4 .. N/2-1): falling +peak..0   - read ROM backward (mirror), no invert
//   Q2 (N/2 .. 3N/4-1): falling 0..-peak  - read ROM forward, invert result
//   Q3 (3N/4.. N-1):   rising  -peak..0   - read ROM backward (mirror), invert result
//
// The two MSBs of the index encode the quadrant:
//   index[IDX_W-1]   (MSB)       -> invert_result : negate sample for Q2/Q3
//   index[IDX_W-2]   (next MSB)  -> invert_index  : mirror ROM address for Q1/Q3
//   index[IDX_W-3:0] (low bits)  -> quarter-wave address base
//
// Pipeline stages (4 clock cycles total latency):
//   Stage 1 : Decode quadrant, conditionally bit-invert quarter address,
//             register ROM address and invert_result flag.
//   Stage 2 : First cycle of synchronous ROM read (BRAM pipeline stage 1).
//   Stage 3 : Second cycle of synchronous ROM read (BRAM pipeline stage 2).
//   Stage 4 : Conditionally two's-complement the ROM result; register output.
//
// Parameters:
//   TABLE_SIZE   - Full table size (number of samples per period; power-of-2, >= 4)
//   SAMPLE_WIDTH - Bit-width of each signed sample
//   INIT_FILE    - Path passed to $readmemh; must contain TABLE_SIZE/4 entries
//                  generated using mid-tread sample positions and symmetric
//                  signed scaling around zero

module sine_table #(
    parameter int TABLE_SIZE   = 1024,
    parameter int SAMPLE_WIDTH = 16,
    parameter     INIT_FILE    = ""
) (
    input  wire logic                          clk,
    input  wire logic [$clog2(TABLE_SIZE)-1:0] index,
    output      logic [SAMPLE_WIDTH-1:0]       sample
);

    // -----------------------------------------------------------------------
    // Derived widths
    // -----------------------------------------------------------------------
    localparam int IDX_W   = $clog2(TABLE_SIZE);  // full-period index width
    localparam int QADDR_W = IDX_W - 2;           // quarter-wave address width

    // Parameter validation (simulation only)
    initial begin
        if (TABLE_SIZE < 4) begin
            $fatal(1, "sine_table: TABLE_SIZE must be >= 4, got %0d", TABLE_SIZE);
        end
        if ((TABLE_SIZE & (TABLE_SIZE - 1)) != 0) begin
            $fatal(1, "sine_table: TABLE_SIZE must be a power of 2, got %0d", TABLE_SIZE);
        end
        if (SAMPLE_WIDTH < 2) begin
            $fatal(1, "sine_table: SAMPLE_WIDTH must be >= 2, got %0d", SAMPLE_WIDTH);
        end
    end

    // -----------------------------------------------------------------------
    // Stage 1 – quadrant decode and ROM address formation
    // -----------------------------------------------------------------------
    // Combinatorial decode from input index
    logic                invert_result;   // negate output for Q2 / Q3
    logic                invert_index;    // mirror ROM address for Q1 / Q3
    logic [QADDR_W-1:0]  qaddr;           // raw lower bits of index
    logic [QADDR_W-1:0]  rom_addr_comb;   // conditionally bit-inverted address

    assign invert_result  = index[IDX_W-1];
    assign invert_index   = index[IDX_W-2];
    assign qaddr          = index[QADDR_W-1:0];
    assign rom_addr_comb  = invert_index ? ~qaddr : qaddr;

    // Stage 1 registered outputs
    logic [QADDR_W-1:0]  rom_addr_r;
    logic                invert_result_r;

    always_ff @(posedge clk) begin
        rom_addr_r      <= rom_addr_comb;
        invert_result_r <= invert_result;
    end

    // -----------------------------------------------------------------------
    // Stages 2–3 – synchronous ROM (sync_sprom provides exactly 2-cycle latency)
    // -----------------------------------------------------------------------
    logic [SAMPLE_WIDTH-1:0] rom_data;

    sync_sprom #(
        .DATA_WIDTH (SAMPLE_WIDTH),
        .ADDR_WIDTH (QADDR_W),
        .INIT_FILE  (INIT_FILE)
    ) u_rom (
        .clk   (clk),
        .addr  (rom_addr_r),
        .rdata (rom_data)
    );

    // Delay invert_result through the two ROM pipeline stages so it arrives
    // aligned with rom_data at the input of Stage 4.
    logic invert_result_r2;
    logic invert_result_r3;

    always_ff @(posedge clk) begin
        invert_result_r2 <= invert_result_r;
        invert_result_r3 <= invert_result_r2;
    end

    // -----------------------------------------------------------------------
    // Stage 4 – conditional two's-complement and output register
    // -----------------------------------------------------------------------
    always_ff @(posedge clk) begin
        sample <= invert_result_r3 ? -rom_data : rom_data;
    end

endmodule
`default_nettype wire

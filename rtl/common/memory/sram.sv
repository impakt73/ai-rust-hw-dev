`default_nettype none
// Single-clock SRAM with 32-bit words and byte write masking.
// Designed to infer block RAM on FPGA targets.
//
// READ-DURING-WRITE BEHAVIOR:
// - Read-first semantics are intentional for this module.
// - When we=1 and waddr==raddr in the same clock edge, rdata captures the
//   pre-write memory contents two cycles later.
//
// BOUNDS BEHAVIOUR:
// - Write addresses >= DEPTH are silently ignored (write bounds check).
// - Read addresses >= DEPTH return implementation-defined values.
//   Callers must not present out-of-bounds read addresses; the sram_peripheral
//   module guarantees this for the RTL peripheral at 0x70000000.
// - When DEPTH == (1 << ADDR_WIDTH) the write bounds comparison is a static
//   tautology that synthesis eliminates, keeping the full-range path identical
//   to what was previously the gen_full_range generate block.
//
// BRAM INITIALIZATION NOTE:
// - Gowin synthesis requires aggregate default initialization for large RAMs.
// - Yosys does not currently accept that syntax for unpacked memory arrays, so
//   use a Yosys-only initial loop fallback selected with YOSYS.
//
// GLOBAL BUFFER NOTE (iCE40):
// - The read path is intentionally unconditional (no in-bounds guard on
//   read_data_pipe) so that the synthesiser maps the pipeline to clean
//   SB_RAM40_4K + SB_DFF primitives without SB_DFFSR reset cells.  The
//   previous gen_bounded_range conditional-zero read pattern synthesised to
//   SB_DFFSR cells whose 32-wide reset buses were promoted to two of the
//   eight iCE40 global buffers, wasting a scarce resource.
module sram #(
    parameter int ADDR_WIDTH = 8,
    parameter int DEPTH      = (1 << ADDR_WIDTH)
) (
    input wire logic                  clk,
    input wire logic                  we,
    input wire logic [3:0]            wmask,
    input wire logic [ADDR_WIDTH-1:0] waddr,
    input wire logic [31:0]           wdata,
    input wire logic [ADDR_WIDTH-1:0] raddr,
    output logic [31:0]           rdata
);

    (* ram_style = "block" *) logic [7:0] mem0 [0:DEPTH-1];
    (* ram_style = "block" *) logic [7:0] mem1 [0:DEPTH-1];
    (* ram_style = "block" *) logic [7:0] mem2 [0:DEPTH-1];
    (* ram_style = "block" *) logic [7:0] mem3 [0:DEPTH-1];

    logic [31:0] read_data_pipe;

    always_ff @(posedge clk) begin
        // Write with bounds guard.  For power-of-two DEPTH the comparison
        // ({1'b0,waddr} < DEPTH) is always true so synthesis folds it away.
        if (we && ({1'b0, waddr} < (ADDR_WIDTH+1)'(DEPTH))) begin
            if (wmask[0]) mem0[waddr] <= wdata[7:0];
            if (wmask[1]) mem1[waddr] <= wdata[15:8];
            if (wmask[2]) mem2[waddr] <= wdata[23:16];
            if (wmask[3]) mem3[waddr] <= wdata[31:24];
        end
        // Read-first behaviour: same-cycle read and write to the same address
        // returns the old memory contents after the internal output pipeline
        // latency.  The unconditional read maps to clean BRAM + DFF primitives
        // without synchronous-clear DFFSR cells.
        read_data_pipe <= {mem3[raddr], mem2[raddr], mem1[raddr], mem0[raddr]};
        rdata <= read_data_pipe;
    end

endmodule
`default_nettype wire

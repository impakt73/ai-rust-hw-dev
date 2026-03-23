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

`ifdef ALTERA_RESERVED_QIS
    logic [31:0] ram_q_b;
    logic [31:0] read_data_pipe;

    altsyncram #(
        .address_reg_b("CLOCK0"),
        .byte_size(8),
        .clock_enable_input_a("BYPASS"),
        .clock_enable_input_b("BYPASS"),
        .clock_enable_output_b("BYPASS"),
        .intended_device_family("Cyclone V"),
        .numwords_a(DEPTH),
        .numwords_b(DEPTH),
        .operation_mode("DUAL_PORT"),
        .outdata_aclr_b("NONE"),
        .outdata_reg_b("UNREGISTERED"),
        .power_up_uninitialized("FALSE"),
        .read_during_write_mode_mixed_ports("OLD_DATA"),
        .width_a(32),
        .width_b(32),
        .width_byteena_a(4),
        .widthad_a(ADDR_WIDTH),
        .widthad_b(ADDR_WIDTH)
    ) ram_block (
        .address_a(waddr),
        .address_b(raddr),
        .byteena_a(wmask),
        .clock0(clk),
        .data_a(wdata),
        .wren_a(we),
        .q_b(ram_q_b),
        .aclr0(1'b0),
        .aclr1(1'b0),
        .addressstall_a(1'b0),
        .addressstall_b(1'b0),
        .byteena_b(1'b1),
        .clock1(1'b1),
        .clocken0(1'b1),
        .clocken1(1'b1),
        .clocken2(1'b1),
        .clocken3(1'b1),
        .data_b(32'b0),
        .eccstatus(),
        .q_a(),
        .rden_a(1'b1),
        .rden_b(1'b1),
        .wren_b(1'b0)
    );

    always_ff @(posedge clk) begin
        read_data_pipe <= ram_q_b;
        rdata <= read_data_pipe;
    end
`else
    (* ram_style = "block" *) logic [31:0] mem [0:DEPTH-1]
`ifdef YOSYS
    ;
`else
    = '{default: '0};
`endif
    logic [31:0] read_data_pipe;

`ifdef YOSYS
    integer init_idx;
    initial begin
        for (init_idx = 0; init_idx < DEPTH; init_idx = init_idx + 1) begin
            mem[init_idx] = '0;
        end
    end
`endif

    always_ff @(posedge clk) begin
        // Write with bounds guard.  For power-of-two DEPTH the comparison
        // ({1'b0,waddr} < DEPTH) is always true so synthesis folds it away.
        if (we && ({1'b0, waddr} < (ADDR_WIDTH+1)'(DEPTH))) begin
            if (wmask[0]) mem[waddr][7:0]   <= wdata[7:0];
            if (wmask[1]) mem[waddr][15:8]  <= wdata[15:8];
            if (wmask[2]) mem[waddr][23:16] <= wdata[23:16];
            if (wmask[3]) mem[waddr][31:24] <= wdata[31:24];
        end
        // Read-first behaviour: same-cycle read and write to the same address
        // returns the old memory contents after the internal output pipeline
        // latency.  The unconditional read maps to clean BRAM + DFF primitives
        // without synchronous-clear DFFSR cells.
        read_data_pipe <= mem[raddr];
        rdata <= read_data_pipe;
    end
`endif

endmodule
`default_nettype wire

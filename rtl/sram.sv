// Single-clock SRAM with 32-bit words and byte write masking.
// Designed to infer block RAM on FPGA targets.
//
// READ-DURING-WRITE BEHAVIOR:
// - Read-first semantics are intentional for this module.
// - When we=1 and waddr==raddr in the same clock edge, rdata captures the
//   pre-write memory contents.
//
// BRAM INITIALIZATION NOTE:
// - The zero-initialization loop below relies on Yosys/iCE40 BRAM init support,
//   which is supported by this project's target FPGA/toolchain.
module sram #(
    parameter int ADDR_WIDTH = 8,
    parameter int DEPTH      = (1 << ADDR_WIDTH)
) (
    input  logic                  clk,
    input  logic                  we,
    input  logic [3:0]            wmask,
    input  logic [ADDR_WIDTH-1:0] waddr,
    input  logic [31:0]           wdata,
    input  logic [ADDR_WIDTH-1:0] raddr,
    output logic [31:0]           rdata
);

    logic [31:0] mem [0:DEPTH-1];
    initial begin
        for (int i = 0; i < DEPTH; i = i + 1) begin
            mem[i] = 32'b0;
        end
    end

    if (DEPTH == (1 << ADDR_WIDTH)) begin : gen_full_range
        always_ff @(posedge clk) begin
            if (we) begin
                if (wmask[0]) mem[waddr][7:0]   <= wdata[7:0];
                if (wmask[1]) mem[waddr][15:8]  <= wdata[15:8];
                if (wmask[2]) mem[waddr][23:16] <= wdata[23:16];
                if (wmask[3]) mem[waddr][31:24] <= wdata[31:24];
            end
            // Read-first behavior: same-cycle read and write to same address returns
            // the old memory contents.
            rdata <= mem[raddr];
        end
    end else begin : gen_bounded_range
        always_ff @(posedge clk) begin
            if (we && ({1'b0, waddr} < (ADDR_WIDTH+1)'(DEPTH))) begin
                if (wmask[0]) mem[waddr][7:0]   <= wdata[7:0];
                if (wmask[1]) mem[waddr][15:8]  <= wdata[15:8];
                if (wmask[2]) mem[waddr][23:16] <= wdata[23:16];
                if (wmask[3]) mem[waddr][31:24] <= wdata[31:24];
            end
            // Read-first behavior: same-cycle read and write to same address returns
            // the old memory contents.
            if ({1'b0, raddr} < (ADDR_WIDTH+1)'(DEPTH)) begin
                rdata <= mem[raddr];
            end else begin
                rdata <= 32'b0;
            end
        end
    end

endmodule

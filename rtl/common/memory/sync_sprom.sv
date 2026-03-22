`default_nettype none
// Synchronous Single-Port ROM Module
// One read port, single clock
// Designed to infer block RAM / ROM on FPGA
//
// BRAM INFERENCE REQUIREMENTS (iCE40 / Yosys):
// - Synchronous reads (output registered on clock edge)
// - Exhaustive assignment to output (no conditional output)
// - ROM contents initialized via $readmemh
//
// This module is parameterized for width and depth.
// Read data is available two clk cycles after the address is presented.

module sync_sprom #(
    parameter int DATA_WIDTH   = 32,
    parameter int ADDR_WIDTH   = 8,  // 256 entries by default
    parameter INIT_FILE = ""
) (
    input  wire logic                  clk,
    input  wire logic [ADDR_WIDTH-1:0] addr,
    output      logic [DATA_WIDTH-1:0] rdata
);

    // Memory array
    // Depth is 2^ADDR_WIDTH entries
    (* ram_style = "block" *) logic [DATA_WIDTH-1:0] mem [0:(1<<ADDR_WIDTH)-1];

    initial begin
`ifndef SYNTHESIS
        if (INIT_FILE == "") begin
            $fatal(1, "sync_sprom requires a non-empty INIT_FILE parameter");
        end
`endif
        if (INIT_FILE != "") begin
        $readmemh(INIT_FILE, mem);
        end
    end

    // Read port - synchronous read (required for BRAM inference)
    // Keep an extra internal pipeline register between the memory output and
    // the externally visible output register.
    logic [DATA_WIDTH-1:0] rdata_pipe;

    always_ff @(posedge clk) begin
        rdata_pipe <= mem[addr];
        rdata <= rdata_pipe;
    end

endmodule
`default_nettype wire

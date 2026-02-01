// Synchronous Simple Dual-Port RAM Module
// One write port, one read port, shared clock
// Designed to infer to iCE40 Block RAM on FPGA
//
// BRAM INFERENCE REQUIREMENTS (iCE40 / Yosys):
// - Single clock for both read and write
// - Synchronous reads (output registered on clock edge)
// - Exhaustive assignment to output (no conditional output)
// - Simple dual-port: one write port, one read port
//
// This module is parameterized for width and depth.
// For iCE40-HX8K: Each SB_RAM40_4K block is 256x16 or 512x8 etc.
// Yosys will combine blocks as needed for wider/deeper configurations.

module sync_dpram #(
    parameter int DATA_WIDTH = 32,
    parameter int ADDR_WIDTH = 8   // 256 entries by default
) (
    input  logic                    clk,
    
    // Write port
    input  logic                    we,
    input  logic [ADDR_WIDTH-1:0]   waddr,
    input  logic [DATA_WIDTH-1:0]   wdata,
    
    // Read port
    input  logic [ADDR_WIDTH-1:0]   raddr,
    output logic [DATA_WIDTH-1:0]   rdata
);

    // Memory array
    // Depth is 2^ADDR_WIDTH entries
    logic [DATA_WIDTH-1:0] mem [0:(1<<ADDR_WIDTH)-1];

    // Write port - synchronous write
    always_ff @(posedge clk) begin
        if (we) begin
            mem[waddr] <= wdata;
        end
    end

    // Read port - synchronous read (required for BRAM inference)
    // Output is registered, data available one cycle after address
    always_ff @(posedge clk) begin
        rdata <= mem[raddr];
    end

endmodule

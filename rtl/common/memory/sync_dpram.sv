`default_nettype none
// Synchronous Simple Dual-Port RAM Module
// One write port, one read port, separate clocks
// Designed to infer to iCE40 Block RAM on FPGA
//
// BRAM INFERENCE REQUIREMENTS (iCE40 / Yosys):
// - Separate read/write clocks are supported by iCE40 BRAM primitives
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
    input wire logic                    wclk,
    input wire logic                    rclk,
    
    // Write port
    input wire logic                    we,
    input wire logic [ADDR_WIDTH-1:0]   waddr,
    input wire logic [DATA_WIDTH-1:0]   wdata,
    
    // Read port
    input wire logic [ADDR_WIDTH-1:0]   raddr,
    output logic [DATA_WIDTH-1:0]   rdata
);

    localparam int DEPTH = (1 << ADDR_WIDTH);

    // Memory array
    // Depth is 2^ADDR_WIDTH entries
    (* ram_style = "block" *) logic [DATA_WIDTH-1:0] mem [0:DEPTH-1]
`ifdef YOSYS
    ;
`else
    = '{default: '0};
`endif

`ifdef YOSYS
    integer init_idx;
    initial begin
        for (init_idx = 0; init_idx < DEPTH; init_idx = init_idx + 1) begin
            mem[init_idx] = '0;
        end
    end
`endif

    // Write port - synchronous write
    always_ff @(posedge wclk) begin
        if (we) begin
            mem[waddr] <= wdata;
        end
    end

    // Read port - synchronous read (required for BRAM inference)
    // Xilinx timing needs an extra internal pipeline register between the BRAM
    // output and the externally visible output register. This makes read data
    // available two rclk cycles after the address is presented.
    logic [DATA_WIDTH-1:0] rdata_pipe;

    always_ff @(posedge rclk) begin
        rdata_pipe <= mem[raddr];
        rdata <= rdata_pipe;
    end

endmodule
`default_nettype wire

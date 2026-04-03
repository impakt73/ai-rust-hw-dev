`default_nettype none
// Synchronous Simple Dual-Port RAM Module
// One write port, one read port, separate clocks
// Designed to infer to FPGA block RAM when the target toolchain supports it
//
// BRAM INFERENCE REQUIREMENTS (portable Yosys-style flow):
// - Separate read/write clocks are supported by common FPGA simple dual-port RAM primitives
// - Synchronous reads (output registered on clock edge)
// - Exhaustive assignment to output (no conditional output)
// - Simple dual-port: one write port, one read port
//
// This module is parameterized for width and depth.
// The exact mapped primitive shape is target-dependent; synthesis combines blocks
// as needed for wider/deeper configurations.

module sync_dpram #(
    parameter int DATA_WIDTH = 32,
    parameter int ADDR_WIDTH = 8,  // 256 entries by default
    parameter bit INIT_ZERO = 1'b0
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
    (* ram_style = "block" *) logic [DATA_WIDTH-1:0] mem [0:DEPTH-1];

    generate
        if (INIT_ZERO) begin : gen_init_zero
`ifdef YOSYS
            // Yosys prefers an explicit per-element initialization loop
            int unsigned i;
            initial begin
                for (i = 0; i < DEPTH; i++) begin
                    mem[i] = '0;
                end
            end
`else
            // Some vendor flows infer BRAM initialization more reliably
            // from a single aggregate assignment than from a loop.
            initial begin
                mem = '{default: '0};
            end
`endif
        end
    endgenerate

    // Write port - synchronous write
    always_ff @(posedge wclk) begin
        if (we) begin
            /* verilator lint_off WIDTHEXPAND */
            /* verilator lint_off WIDTHTRUNC */
            mem[waddr] <= wdata;
            /* verilator lint_on WIDTHTRUNC */
            /* verilator lint_on WIDTHEXPAND */
        end
    end

    // Read port - synchronous read (required for BRAM inference)
    // Xilinx timing needs an extra internal pipeline register between the BRAM
    // output and the externally visible output register. This makes read data
    // available two rclk cycles after the address is presented.
    logic [DATA_WIDTH-1:0] rdata_pipe;

    always_ff @(posedge rclk) begin
        /* verilator lint_off WIDTHEXPAND */
        /* verilator lint_off WIDTHTRUNC */
        rdata_pipe <= mem[raddr];
        /* verilator lint_on WIDTHTRUNC */
        /* verilator lint_on WIDTHEXPAND */
        rdata <= rdata_pipe;
    end

endmodule
`default_nettype wire

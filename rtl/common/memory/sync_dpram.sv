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
    input  logic                    wclk,
    input  logic                    rclk,
    
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
    (* ram_style = "block" *) logic [DATA_WIDTH-1:0] mem [0:(1<<ADDR_WIDTH)-1];

    // Initialize memory to 0 to ensure x0 register starts at 0 (required for RISC-V)
    // Uses initial block which Yosys supports for BRAM initialization
    integer i;
    initial begin
        for (i = 0; i < (1 << ADDR_WIDTH); i = i + 1) begin
            mem[i] = {DATA_WIDTH{1'b0}};
        end
    end

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

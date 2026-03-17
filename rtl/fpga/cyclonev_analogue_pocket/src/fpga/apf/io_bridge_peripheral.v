module io_bridge_peripheral (
    input  wire        clk,
    input  wire        reset_n,
    input  wire        endian_little,
    output wire [31:0] pmp_addr,
    output wire        pmp_addr_valid,
    output wire        pmp_rd,
    input  wire [31:0] pmp_rd_data,
    output wire        pmp_wr,
    output wire [31:0] pmp_wr_data,
    inout  wire        phy_spimosi,
    inout  wire        phy_spimiso,
    inout  wire        phy_spiclk,
    input  wire        phy_spiss
);
    assign pmp_addr = 32'h0;
    assign pmp_addr_valid = 1'b0;
    assign pmp_rd = 1'b0;
    assign pmp_wr = 1'b0;
    assign pmp_wr_data = 32'h0;
    assign phy_spimosi = 1'bz;
    assign phy_spimiso = 1'bz;
    assign phy_spiclk = 1'bz;
endmodule

// BRAM-Based Register File Module
// 32x32-bit register file for RISC-V RV32I using dual BRAM copies
// x0 is hardwired to 0
//
// DUAL-COPY BRAM ARCHITECTURE:
// ============================
// iCE40 BRAM blocks (SB_RAM40_4K) are 256x16-bit with pseudo-dual-port capability:
//   - One dedicated read port
//   - One dedicated write port
//
// PROBLEM: RISC-V requires 2 simultaneous reads (rs1, rs2) + 1 write (rd)
//
// SOLUTION: Use TWO BRAM copies, each storing the complete register file:
//   - BRAM_A: Provides rs1 read data (via its read port)
//   - BRAM_B: Provides rs2 read data (via its read port)
//   - Writes go to BOTH BRAMs simultaneously (via their write ports)
//
// RESOURCE USAGE:
//   - Without this: ~400 LUTs for 32x32-bit distributed RAM + muxing
//   - With this: 4 BRAM blocks (2 copies × 2 blocks per copy for 32-bit width)
//   - Trade-off: Uses BRAM (plentiful on iCE40-HX8K) instead of LUTs (constrained)
//
// BRAM CONFIGURATION:
//   Each 32-bit register requires 2× SB_RAM40_4K blocks (16-bit each)
//   Two copies means 4 BRAM blocks total (out of 32 available on iCE40-HX8K)
//
// LATENCY:
//   - Reads: 1 cycle (synchronous BRAM read)
//   - Writes: 1 cycle (synchronous BRAM write)
//   - NOTE: CPU must account for 1-cycle read latency in FSM

module regfile_bram (
    input  logic        clk,
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // Read address 1
    input  logic [4:0]  rs2_addr,     // Read address 2
    input  logic [4:0]  rd_addr,      // Write address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data      // Read data 2
);

    // ============================================================
    // BRAM Instance Declarations
    // ============================================================
    // We use 256x16 BRAM mode (smallest available on iCE40)
    // Address bits: 8 bits for 256 entries, we only use lower 5 bits
    // Data bits: 16 bits per block, need 2 blocks per copy for 32-bit width
    
    // Read data from BRAM (before x0 muxing)
    logic [15:0] rs1_data_lo, rs1_data_hi;
    logic [15:0] rs2_data_lo, rs2_data_hi;
    
    // Combined 32-bit read data from BRAM
    logic [31:0] rs1_data_bram;
    logic [31:0] rs2_data_bram;
    
    assign rs1_data_bram = {rs1_data_hi, rs1_data_lo};
    assign rs2_data_bram = {rs2_data_hi, rs2_data_lo};
    
    // Write enable gated to prevent x0 writes
    logic we_gated;
    assign we_gated = we && (rd_addr != 5'd0);
    
    // Extend 5-bit register addresses to 8-bit BRAM addresses
    logic [7:0] rs1_addr_ext, rs2_addr_ext, rd_addr_ext;
    assign rs1_addr_ext = {3'b000, rs1_addr};
    assign rs2_addr_ext = {3'b000, rs2_addr};
    assign rd_addr_ext = {3'b000, rd_addr};
    
    // ============================================================
    // BRAM Copy A - Provides rs1 read data
    // ============================================================
    // Lower 16 bits of Copy A
    SB_RAM40_4K #(
        .WRITE_MODE(0),  // 256x16 mode
        .READ_MODE(0)    // 256x16 mode
    ) bram_a_lo (
        .RDATA(rs1_data_lo),
        .RADDR(rs1_addr_ext),
        .RCLK(clk),
        .RCLKE(1'b1),
        .RE(1'b1),
        .WDATA(rd_data[15:0]),
        .WADDR(rd_addr_ext),
        .WCLK(clk),
        .WCLKE(1'b1),
        .WE(we_gated)
    );
    
    // Upper 16 bits of Copy A
    SB_RAM40_4K #(
        .WRITE_MODE(0),  // 256x16 mode
        .READ_MODE(0)    // 256x16 mode
    ) bram_a_hi (
        .RDATA(rs1_data_hi),
        .RADDR(rs1_addr_ext),
        .RCLK(clk),
        .RCLKE(1'b1),
        .RE(1'b1),
        .WDATA(rd_data[31:16]),
        .WADDR(rd_addr_ext),
        .WCLK(clk),
        .WCLKE(1'b1),
        .WE(we_gated)
    );
    
    // ============================================================
    // BRAM Copy B - Provides rs2 read data
    // ============================================================
    // Lower 16 bits of Copy B
    SB_RAM40_4K #(
        .WRITE_MODE(0),  // 256x16 mode
        .READ_MODE(0)    // 256x16 mode
    ) bram_b_lo (
        .RDATA(rs2_data_lo),
        .RADDR(rs2_addr_ext),
        .RCLK(clk),
        .RCLKE(1'b1),
        .RE(1'b1),
        .WDATA(rd_data[15:0]),
        .WADDR(rd_addr_ext),
        .WCLK(clk),
        .WCLKE(1'b1),
        .WE(we_gated)
    );
    
    // Upper 16 bits of Copy B
    SB_RAM40_4K #(
        .WRITE_MODE(0),  // 256x16 mode
        .READ_MODE(0)    // 256x16 mode
    ) bram_b_hi (
        .RDATA(rs2_data_hi),
        .RADDR(rs2_addr_ext),
        .RCLK(clk),
        .RCLKE(1'b1),
        .RE(1'b1),
        .WDATA(rd_data[31:16]),
        .WADDR(rd_addr_ext),
        .WCLK(clk),
        .WCLKE(1'b1),
        .WE(we_gated)
    );
    
    // ============================================================
    // x0 Handling - Override BRAM output with 0 for register x0
    // ============================================================
    // BRAM reads have 1-cycle latency, so we need to register the address
    // to know if we're reading x0 when data becomes available
    logic [4:0] rs1_addr_reg, rs2_addr_reg;
    
    always_ff @(posedge clk) begin
        rs1_addr_reg <= rs1_addr;
        rs2_addr_reg <= rs2_addr;
    end
    
    // Output mux: return 0 for x0, otherwise BRAM data
    assign rs1_data = (rs1_addr_reg == 5'd0) ? 32'd0 : rs1_data_bram;
    assign rs2_data = (rs2_addr_reg == 5'd0) ? 32'd0 : rs2_data_bram;

endmodule

// Register File Module
// 32x32-bit register file for RISC-V RV32I
//
// DUAL-BANKED BRAM ARCHITECTURE:
// ==============================
// RISC-V requires 2 simultaneous reads (rs1, rs2) + 1 write (rd).
// iCE40 BRAM blocks are simple dual-port (1 read + 1 write port).
//
// SOLUTION: Use TWO BRAM copies (banks), each storing the complete register file:
//   - Bank A: Provides rs1 read data (via its read port)
//   - Bank B: Provides rs2 read data (via its read port)
//   - Writes go to BOTH banks simultaneously (via their write ports)
//
// RESOURCE USAGE:
//   - 4 BRAM blocks total (2 banks × 2 blocks per bank for 32-bit width)
//   - Each sync_dpram instance uses 256x32-bit = 2× iCE40 SB_RAM40_4K blocks
//
// LATENCY:
//   - Reads: 2 cycles (synchronous BRAM read + output pipeline register)
//   - Writes: 1 cycle (synchronous BRAM write)
//
// x0 HANDLING:
//   - BRAM is initialized to 0 (via sync_dpram initialization)
//   - Writes to x0 are blocked by the CPU (we is gated in cpu.sv)
//   - No special read logic needed since x0 is initialized to 0 and never written

module regfile (
    input  logic        clk,
    input  logic        we,           // Write enable (already gated for x0 in CPU)
    input  logic [4:0]  rs1_addr,     // Read address 1
    input  logic [4:0]  rs2_addr,     // Read address 2
    input  logic [4:0]  rd_addr,      // Write address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data      // Read data 2
);

    // ============================================================
    // Bank A - Provides rs1 read data
    // ============================================================
    sync_dpram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(8)   // 256 entries (only 32 used, but helps BRAM inference)
    ) bank_a (
        .wclk(clk),
        .rclk(clk),
        .we(we),
        .waddr({3'b000, rd_addr}),   // Zero-extend to 8 bits
        .wdata(rd_data),
        .raddr({3'b000, rs1_addr}),  // Zero-extend to 8 bits
        .rdata(rs1_data)
    );

    // ============================================================
    // Bank B - Provides rs2 read data
    // ============================================================
    sync_dpram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(8)   // 256 entries (only 32 used, but helps BRAM inference)
    ) bank_b (
        .wclk(clk),
        .rclk(clk),
        .we(we),
        .waddr({3'b000, rd_addr}),   // Zero-extend to 8 bits
        .wdata(rd_data),
        .raddr({3'b000, rs2_addr}),  // Zero-extend to 8 bits
        .rdata(rs2_data)
    );

endmodule

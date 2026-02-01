// Register File Module
// 32x32-bit register file for RISC-V RV32I
// x0 is hardwired to 0
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
//   - Reads: 1 cycle (synchronous BRAM read)
//   - Writes: 1 cycle (synchronous BRAM write)
//   - NOTE: CPU must provide registered addresses (address registration is in CPU)
//
// x0 HANDLING:
//   - Writes to x0 are blocked (we is gated)
//   - Reads from x0 return 0 (output mux based on registered address)

module regfile (
    input  logic        clk,
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // Read address 1
    input  logic [4:0]  rs2_addr,     // Read address 2
    input  logic [4:0]  rd_addr,      // Write address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data      // Read data 2
);

    // Write enable gated to prevent x0 writes
    logic we_gated;
    assign we_gated = we && (rd_addr != 5'd0);

    // Raw BRAM read outputs (before x0 muxing)
    logic [31:0] rs1_data_bram;
    logic [31:0] rs2_data_bram;

    // ============================================================
    // Bank A - Provides rs1 read data
    // ============================================================
    sync_dpram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(8)   // 256 entries (only 32 used, but helps BRAM inference)
    ) bank_a (
        .clk(clk),
        .we(we_gated),
        .waddr({3'b000, rd_addr}),   // Zero-extend to 8 bits
        .wdata(rd_data),
        .raddr({3'b000, rs1_addr}),  // Zero-extend to 8 bits
        .rdata(rs1_data_bram)
    );

    // ============================================================
    // Bank B - Provides rs2 read data
    // ============================================================
    sync_dpram #(
        .DATA_WIDTH(32),
        .ADDR_WIDTH(8)   // 256 entries (only 32 used, but helps BRAM inference)
    ) bank_b (
        .clk(clk),
        .we(we_gated),
        .waddr({3'b000, rd_addr}),   // Zero-extend to 8 bits
        .wdata(rd_data),
        .raddr({3'b000, rs2_addr}),  // Zero-extend to 8 bits
        .rdata(rs2_data_bram)
    );

    // ============================================================
    // x0 Handling - Override BRAM output with 0 for register x0
    // ============================================================
    // BRAM reads have 1-cycle latency (address in, data out next cycle).
    // We register the address here to know which register was requested
    // when the BRAM data becomes available.
    logic [4:0] rs1_addr_reg, rs2_addr_reg;

    always_ff @(posedge clk) begin
        rs1_addr_reg <= rs1_addr;
        rs2_addr_reg <= rs2_addr;
    end

    // Output mux: return 0 for x0, otherwise BRAM data
    assign rs1_data = (rs1_addr_reg == 5'd0) ? 32'd0 : rs1_data_bram;
    assign rs2_data = (rs2_addr_reg == 5'd0) ? 32'd0 : rs2_data_bram;

endmodule

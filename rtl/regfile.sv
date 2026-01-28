// Register File Module
// 32x32-bit register file for RISC-V RV32I
// x0 is hardwired to 0
//
// BRAM INFERENCE ANALYSIS FOR iCE40-HX8K:
// ========================================
// iCE40 BRAM blocks are 256x16-bit (4Kbit each). Yosys infers BRAM when:
//   1. Memory depth >= 256 entries (our regfile has only 32 entries)
//   2. Synchronous read (read address registered, data available next cycle)
//   3. Synchronous write
//
// PROBLEM: This register file does NOT meet BRAM inference criteria because:
//   - Depth is 32 entries (< 256 minimum for iCE40 BRAM inference)
//   - Asynchronous reads are REQUIRED by the multi-cycle CPU architecture
//     (DECODE state needs immediate access to register values)
//
// WORKAROUND: The USE_BRAM parameter enables a "read-registered" mode where:
//   - Register file uses distributed RAM (LUTs) as before
//   - Read outputs are optionally registered for timing improvement
//   - This does NOT save LUTs but improves Fmax by breaking combinational paths
//
// FUTURE OPTIMIZATION: To actually use BRAM, the CPU architecture would need:
//   1. Pipeline the DECODE stage to tolerate 1-cycle read latency
//   2. Add bypass/forwarding logic for back-to-back read-after-write hazards
//   3. Potentially increase depth to 256 entries (waste 224 entries)
//
// CURRENT RECOMMENDATION: Keep USE_BRAM=0 (async reads, LUT-based storage)
// Estimated LUT usage: ~400 LUTs (5.3% of iCE40-HX8K)

module regfile #(
    parameter bit USE_BRAM = 1'b0  // 0 = Async reads (LUT-based), 1 = Sync reads (register outputs)
) (
    input  logic        clk,
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // Read address 1
    input  logic [4:0]  rs2_addr,     // Read address 2
    input  logic [4:0]  rd_addr,      // Write address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data      // Read data 2
);

    // 32x32-bit register array (stored in LUTs, not BRAM - see comments above)
    logic [31:0] registers [31:0];

    // Internal read values (before optional output registering)
    logic [31:0] rs1_data_int;
    logic [31:0] rs2_data_int;

    // Read operations (combinational/asynchronous from register array)
    always_comb begin
        // x0 is always 0 (RISC-V architectural requirement)
        if (rs1_addr == 5'd0)
            rs1_data_int = 32'd0;
        else
            rs1_data_int = registers[rs1_addr];
    end

    always_comb begin
        // x0 is always 0
        if (rs2_addr == 5'd0)
            rs2_data_int = 32'd0;
        else
            rs2_data_int = registers[rs2_addr];
    end

    // Write operation (synchronous)
    always_ff @(posedge clk) begin
        // Only write if write enable is high and address is not x0
        if (we && rd_addr != 5'd0) begin
            registers[rd_addr] <= rd_data;
        end
    end

    // Output path: Optional registering for timing improvement
    generate
        if (USE_BRAM) begin : gen_registered_outputs
            // Registered outputs (adds 1 cycle latency, improves Fmax)
            // NOTE: This does NOT infer BRAM on iCE40 due to small depth (32 entries < 256)
            // It only registers the outputs to break combinational paths
            always_ff @(posedge clk) begin
                rs1_data <= rs1_data_int;
                rs2_data <= rs2_data_int;
            end
        end else begin : gen_async_outputs
            // Direct combinational outputs (zero latency, required by current CPU architecture)
            always_comb begin
                rs1_data = rs1_data_int;
                rs2_data = rs2_data_int;
            end
        end
    endgenerate

endmodule

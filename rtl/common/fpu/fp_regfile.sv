`default_nettype none
// Floating Point Register File Module
// 32x32-bit FP register file for RISC-V RV32F extension
// Unlike integer x0, all FP registers (f0-f31) are writable
//
// BRAM INFERENCE ANALYSIS FOR iCE40-HX8K:
// ========================================
// iCE40 BRAM blocks are 256x16-bit (4Kbit each). Yosys infers BRAM when:
//   1. Memory depth >= 256 entries (our FP regfile has only 32 entries)
//   2. Synchronous read (read address registered, data available next cycle)
//   3. Synchronous write
//
// PROBLEM: This FP register file does NOT meet BRAM inference criteria because:
//   - Depth is 32 entries (< 256 minimum for iCE40 BRAM inference)
//   - Asynchronous reads are REQUIRED by the multi-cycle CPU architecture
//     (FP operations need immediate access to operands in EXECUTE state)
//   - Three simultaneous read ports (rs1, rs2, rs3) for FMA operations
//
// WORKAROUND: The REGISTER_OUTPUTS parameter enables a "read-registered" mode where:
//   - FP register file uses distributed RAM (LUTs) as before
//   - Read outputs are optionally registered for timing improvement
//   - This does NOT save LUTs but improves Fmax by breaking combinational paths
//
// FUTURE OPTIMIZATION: To actually use BRAM, the CPU architecture would need:
//   1. Pipeline the FPU to tolerate 1-cycle read latency
//   2. Add bypass/forwarding logic for back-to-back read-after-write hazards
//   3. Use 3 separate BRAM blocks for 3 read ports (increases resource usage!)
//   4. Potentially increase depth to 256 entries (waste 224 entries)
//
// CURRENT RECOMMENDATION: Keep REGISTER_OUTPUTS=0 (async reads, LUT-based storage)
// Estimated LUT usage: ~680 LUTs (8.9% of iCE40-HX8K)

module fp_regfile #(
    parameter bit REGISTER_OUTPUTS = 1'b0  // 0 = Async reads (LUT-based), 1 = Sync reads (register outputs)
) (
    input wire logic        clk,
    input wire logic        rst_n,        // Active-low reset
    input wire logic        we,           // Write enable
    input wire logic [4:0]  rs1_addr,     // FP source register 1 address
    input wire logic [4:0]  rs2_addr,     // FP source register 2 address
    input wire logic [4:0]  rs3_addr,     // FP source register 3 address (for FMADD, etc.)
    input wire logic [4:0]  rd_addr,      // FP destination register address
    input wire logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read data 1
    output logic [31:0] rs2_data,     // Read data 2
    output logic [31:0] rs3_data      // Read data 3
);

    // 32x32-bit FP register array (stored in LUTs, not BRAM - see comments above)
    logic [31:0] fp_registers [31:0];

    // Internal read values (before optional output registering)
    logic [31:0] rs1_data_int;
    logic [31:0] rs2_data_int;
    logic [31:0] rs3_data_int;

    // Read operations (combinational/asynchronous from register array)
    // All FP registers are readable (no special handling like x0)
    always_comb begin
        rs1_data_int = fp_registers[rs1_addr];
        rs2_data_int = fp_registers[rs2_addr];
        rs3_data_int = fp_registers[rs3_addr];
    end

    // Write operation (synchronous)
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            // Reset all FP registers to +0.0 (0x00000000)
            for (int i = 0; i < 32; i++) begin
                fp_registers[i] <= 32'h00000000;
            end
        end else if (we) begin
            // All FP registers can be written (no x0-like restriction)
            fp_registers[rd_addr] <= rd_data;
        end
    end

    // Output path: Optional registering for timing improvement
    generate
        if (REGISTER_OUTPUTS) begin : gen_registered_outputs
            // Registered outputs (adds 1 cycle latency, improves Fmax)
            // NOTE: This does NOT infer BRAM on iCE40 due to small depth (32 entries < 256)
            // It only registers the outputs to break combinational paths
            always_ff @(posedge clk) begin
                rs1_data <= rs1_data_int;
                rs2_data <= rs2_data_int;
                rs3_data <= rs3_data_int;
            end
        end else begin : gen_async_outputs
            // Direct combinational outputs (zero latency, required by current CPU architecture)
            assign rs1_data = rs1_data_int;
            assign rs2_data = rs2_data_int;
            assign rs3_data = rs3_data_int;
        end
    endgenerate

endmodule
`default_nettype wire

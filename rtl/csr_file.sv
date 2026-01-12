// CSR (Control and Status Register) File Module
// Implements RISC-V CSR register file and operations
// Includes support for F extension FCSR/FRM/FFLAGS

module csr_file (
    input  logic        clk,
    input  logic        rst_n,
    
    // Control signals
    input  logic        is_csr,
    input  logic [2:0]  funct3,
    input  logic [4:0]  rs1,
    
    // Data signals
    input  logic [11:0] csr_addr,
    input  logic [31:0] rs1_data,
    input  logic [31:0] fcsr,        // F extension: FCSR value from top module
    
    // Output
    output logic [31:0] csr_rdata
);

    // CSR registers (4096 possible, but we only implement a few)
    logic [31:0] csr_registers [0:4095];
    
    // Read CSR value (with F extension support)
    always_comb begin
        case (csr_addr)
            12'h001: csr_rdata = {27'h0, fcsr[4:0]};  // FFLAGS - exception flags
            12'h002: csr_rdata = {29'h0, fcsr[7:5]};  // FRM - rounding mode
            12'h003: csr_rdata = fcsr;                 // FCSR - full register
            default: csr_rdata = csr_registers[csr_addr];  // Other CSRs
        endcase
    end
    
    // CSR write logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            // Initialize all CSRs to 0
            for (int i = 0; i < 4096; i++) begin
                csr_registers[i] = 32'h0;  // Use blocking assignment for initialization
            end
        end else if (is_csr) begin
            // CSR write operations
            // Note: FCSR/FRM/FFLAGS (0x001, 0x002, 0x003) are handled in top.sv, not here
            case (funct3)
                3'b001: csr_registers[csr_addr] <= rs1_data;                                     // CSRRW
                3'b010: if (rs1 != 5'b0) csr_registers[csr_addr] <= csr_rdata | rs1_data;        // CSRRS (no write when rs1 == x0)
                3'b011: if (rs1 != 5'b0) csr_registers[csr_addr] <= csr_rdata & ~rs1_data;       // CSRRC (no write when rs1 == x0)
                3'b101: csr_registers[csr_addr] <= {27'b0, rs1};                                 // CSRRWI
                3'b110: if (rs1 != 5'b0) csr_registers[csr_addr] <= csr_rdata | {27'b0, rs1};    // CSRRSI (no write when zimm[4:0] == 0)
                3'b111: if (rs1 != 5'b0) csr_registers[csr_addr] <= csr_rdata & ~{27'b0, rs1};   // CSRRCI (no write when zimm[4:0] == 0)
                default: ; // Do nothing
            endcase
        end
    end

endmodule

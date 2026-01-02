// CSR (Control and Status Register) File Module
// Implements RISC-V CSR register file and operations

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
    
    // Output
    output logic [31:0] csr_rdata
);

    // CSR registers (4096 possible, but we only implement a few)
    logic [31:0] csr_registers [0:4095];
    
    // Read CSR value
    assign csr_rdata = csr_registers[csr_addr];
    
    // CSR write logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            // Initialize all CSRs to 0
            for (int i = 0; i < 4096; i++) begin
                csr_registers[i] = 32'h0;  // Use blocking assignment for initialization
            end
        end else if (is_csr) begin
            // CSR write operations
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

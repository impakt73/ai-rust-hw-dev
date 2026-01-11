// Writeback Multiplexer Module
// Selects the appropriate data to write back to the register file
// Supports atomic operations (A extension)

module writeback_mux (
    // Control signals
    input  logic [6:0]  opcode,
    input  logic        jump,
    input  logic        is_csr,
    input  logic        mem_to_reg,
    input  logic        is_lr,          // A extension: LR.W instruction
    input  logic        is_sc,          // A extension: SC.W instruction
    input  logic        is_amo,         // A extension: AMO instruction
    input  logic        sc_success,     // A extension: SC success flag
    
    // Data inputs
    input  logic [31:0] pc,
    input  logic [31:0] imm_u,
    input  logic [31:0] alu_result,
    input  logic [31:0] csr_rdata,
    input  logic [31:0] formatted_load_data,
    
    // Output
    output logic [31:0] rd_data
);

    // Write-back data selection
    always_comb begin
        if (is_amo) begin
            // AMO instruction - Return original value from memory
            rd_data = formatted_load_data;
        end else if (is_sc) begin
            // SC.W - Return 0 for success, 1 for failure
            rd_data = {31'b0, ~sc_success};
        end else if (opcode == 7'b0110111) begin
            // LUI - Load Upper Immediate
            rd_data = imm_u;
        end else if (opcode == 7'b0010111) begin
            // AUIPC - Add Upper Immediate to PC
            rd_data = pc + imm_u;
        end else if (jump) begin
            // JAL/JALR - Store return address (PC + 4)
            rd_data = pc + 32'd4;
        end else if (is_csr) begin
            // CSR instruction - Return old CSR value
            rd_data = csr_rdata;
        end else if (mem_to_reg || is_lr) begin
            // Load instruction or LR.W - Use formatted memory data
            rd_data = formatted_load_data;
        end else begin
            // ALU result
            rd_data = alu_result;
        end
    end

endmodule

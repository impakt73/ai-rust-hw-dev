// Memory Interface Module
// Handles memory operation sizing and load data formatting

module mem_interface (
    // Control signals
    input  logic [2:0]  funct3,
    input  logic        mem_write,
    input  logic        mem_read,
    
    // Data signals
    input  logic [31:0] alu_result,
    input  logic [31:0] rs2_data,
    input  logic [31:0] dmem_rdata,
    
    // Memory interface outputs
    output logic [31:0] dmem_addr,
    output logic [31:0] dmem_wdata,
    output logic        dmem_we,
    output logic        dmem_re,
    output logic [1:0]  dmem_size,
    
    // Formatted load data
    output logic [31:0] formatted_load_data
);

    // Data memory address and control
    assign dmem_addr = alu_result;
    assign dmem_wdata = rs2_data;  // Pass data directly - no formatting needed
    assign dmem_we = mem_write;
    assign dmem_re = mem_read;
    
    // Encode memory operation size from funct3
    // funct3[1:0] distinguishes byte (00), halfword (01), word (10)
    // For loads: LB=000, LH=001, LW=010, LBU=100, LHU=101
    // For stores: SB=000, SH=001, SW=010
    assign dmem_size = funct3[1:0];
    
    // Load data sign/zero extension based on funct3
    // The simulator will return the exact byte/halfword requested
    // We only need to perform sign/zero extension here
    always_comb begin
        case (funct3)
            3'b000: begin // LB - Load Byte (sign-extended)
                formatted_load_data = {{24{dmem_rdata[7]}}, dmem_rdata[7:0]};
            end
            3'b001: begin // LH - Load Halfword (sign-extended)
                formatted_load_data = {{16{dmem_rdata[15]}}, dmem_rdata[15:0]};
            end
            3'b100: begin // LBU - Load Byte Unsigned (zero-extended)
                formatted_load_data = {24'b0, dmem_rdata[7:0]};
            end
            3'b101: begin // LHU - Load Halfword Unsigned (zero-extended)
                formatted_load_data = {16'b0, dmem_rdata[15:0]};
            end
            default: formatted_load_data = dmem_rdata; // LW - Load Word
        endcase
    end

endmodule

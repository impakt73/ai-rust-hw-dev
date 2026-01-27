// Block RAM Data Memory
// Single-port read/write memory for data storage
// Synthesizes to on-chip BRAM on iCE40 FPGA

module bram_dmem #(
    parameter ADDR_WIDTH = 10,  // 2^10 = 1024 words
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,
    input  logic [DATA_WIDTH-1:0]   wdata,
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    we,
    input  logic                    re,
    input  logic [1:0]              size,  // 00=byte, 01=half, 10=word
    input  logic                    req,
    output logic                    ready
);

    // Memory array
    logic [DATA_WIDTH-1:0] mem [0:(2**ADDR_WIDTH)-1];
    
    // Initialize memory to zero
    initial begin
        for (int i = 0; i < (2**ADDR_WIDTH); i++) begin
            mem[i] = 32'h0;
        end
    end
    
    // Read/write logic with 1-cycle latency
    logic [DATA_WIDTH-1:0] rdata_reg;
    logic ready_reg;
    
    always_ff @(posedge clk) begin
        if (req) begin
            if (we) begin
                // Write logic with byte lane support
                case (size)
                    2'b00: begin  // Byte write
                        case (addr[1:0])
                            2'b00: mem[addr][7:0]   <= wdata[7:0];
                            2'b01: mem[addr][15:8]  <= wdata[7:0];
                            2'b10: mem[addr][23:16] <= wdata[7:0];
                            2'b11: mem[addr][31:24] <= wdata[7:0];
                        endcase
                    end
                    2'b01: begin  // Halfword write
                        if (addr[1] == 1'b0) begin
                            mem[addr][15:0] <= wdata[15:0];
                        end else begin
                            mem[addr][31:16] <= wdata[15:0];
                        end
                    end
                    2'b10: begin  // Word write
                        mem[addr] <= wdata;
                    end
                    default: ;
                endcase
                rdata_reg <= 32'h0;
            end else if (re) begin
                // Read logic
                rdata_reg <= mem[addr];
            end else begin
                rdata_reg <= 32'h0;
            end
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

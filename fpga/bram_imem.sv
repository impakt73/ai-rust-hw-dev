// Block RAM Instruction Memory
// Single-port read-only memory for instruction storage
// Synthesizes to on-chip BRAM on iCE40 FPGA

module bram_imem #(
    parameter ADDR_WIDTH = 10,  // 2^10 = 1024 words
    parameter DATA_WIDTH = 32
) (
    input  logic                    clk,
    input  logic [ADDR_WIDTH-1:0]   addr,
    output logic [DATA_WIDTH-1:0]   rdata,
    input  logic                    req,
    output logic                    ready
);

    // Memory array
    logic [DATA_WIDTH-1:0] mem [0:(2**ADDR_WIDTH)-1];
    
    // Initialize memory with a simple LED blink program
    // This can be replaced with a program loaded from a file during synthesis
    initial begin
        // Simple test program: Write pattern to LED controller then halt
        // Address 0x80000000 (instruction memory base)
        
        // lui x15, 0x50000  // Load LED base address (0x50000000)
        mem[0] = 32'h50000_7B7;
        
        // addi x14, x0, 0xAA  // Load pattern 0xAA
        mem[1] = 32'h0AA00_713;
        
        // sw x14, 0(x15)  // Write to LED register
        mem[2] = 32'h00E7A023;
        
        // Loop: addi x13, x0, 0  // nop (add 0 to x0, write to x13)
        mem[3] = 32'h00000_693;
        
        // j Loop  // Jump to self (infinite loop)
        mem[4] = 32'hFFFFFFEF;  // jal x0, -4 (loop forever)
        
        // Fill rest with NOPs
        for (int i = 5; i < (2**ADDR_WIDTH); i++) begin
            mem[i] = 32'h00000013;  // addi x0, x0, 0 (NOP)
        end
    end
    
    // Read logic with 1-cycle latency
    logic [DATA_WIDTH-1:0] rdata_reg;
    logic ready_reg;
    
    always_ff @(posedge clk) begin
        if (req) begin
            rdata_reg <= mem[addr];
            ready_reg <= 1'b1;
        end else begin
            ready_reg <= 1'b0;
        end
    end
    
    assign rdata = rdata_reg;
    assign ready = ready_reg;

endmodule

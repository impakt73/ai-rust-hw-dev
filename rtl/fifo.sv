// FIFO Module for Memory-Mapped I/O Communication
// Provides a simple FIFO buffer with memory-mapped control and status registers
//
// Memory Map (base address provided by parent):
//   Offset 0x0: DATA register (write to push, read to pop)
//   Offset 0x4: STATUS register (read-only)
//     Bit 0: FIFO Empty (1 = empty, 0 = has data)
//     Bit 1: FIFO Full (1 = full, 0 = has space)
//     Bits 31-2: Reserved (read as 0)

module fifo #(
    parameter DEPTH = 16,  // FIFO depth (number of entries)
    parameter WIDTH = 8    // Data width (8 bits for byte-oriented communication)
) (
    input  logic             clk,
    input  logic             rst_n,
    
    // Write interface (from CPU)
    input  logic             wr_en,
    input  logic [WIDTH-1:0] wr_data,
    
    // Read interface (to external callback)
    input  logic             rd_en,
    output logic [WIDTH-1:0] rd_data,
    
    // Status flags
    output logic             empty,
    output logic             full
);

    // FIFO storage
    logic [WIDTH-1:0] mem [0:DEPTH-1];
    
    // Pointers (need one extra bit for full/empty detection)
    logic [$clog2(DEPTH):0] wr_ptr;
    logic [$clog2(DEPTH):0] rd_ptr;
    
    // Full and empty detection
    assign empty = (wr_ptr == rd_ptr);
    assign full = (wr_ptr[$clog2(DEPTH)] != rd_ptr[$clog2(DEPTH)]) && 
                  (wr_ptr[$clog2(DEPTH)-1:0] == rd_ptr[$clog2(DEPTH)-1:0]);
    
    // Write logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            wr_ptr <= '0;
        end else if (wr_en && !full) begin
            mem[wr_ptr[$clog2(DEPTH)-1:0]] <= wr_data;
            wr_ptr <= wr_ptr + 1;
        end
    end
    
    // Read logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            rd_ptr <= '0;
        end else if (rd_en && !empty) begin
            rd_ptr <= rd_ptr + 1;
        end
    end
    
    // Output data
    assign rd_data = mem[rd_ptr[$clog2(DEPTH)-1:0]];

endmodule

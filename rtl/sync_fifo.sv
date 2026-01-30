// Synchronous FIFO Module
// Generic FIFO with configurable data width and depth
// Supports single-cycle read and write operations
//
// Parameters:
//   WIDTH - Data width in bits (default: 8)
//   DEPTH - Number of entries (must be power of 2, >= 2, default: 8)
//
// Interface:
//   clk     - System clock
//   rst_n   - Asynchronous active-low reset
//   wr_en   - Write enable (data written on rising edge when not full)
//   rd_en   - Read enable (advances read pointer on rising edge when not empty)
//   wdata   - Data to write
//   rdata   - Data at read pointer (combinatorial output)
//   full    - FIFO is full, cannot accept more data
//   empty   - FIFO is empty, no data available
//   count   - Number of entries currently in FIFO

module sync_fifo #(
    parameter int WIDTH = 8,
    parameter int DEPTH = 8
) (
    input  logic             clk,
    input  logic             rst_n,
    
    // Write interface
    input  logic             wr_en,
    input  logic [WIDTH-1:0] wdata,
    
    // Read interface
    input  logic             rd_en,
    output logic [WIDTH-1:0] rdata,
    
    // Status outputs
    output logic             full,
    output logic             empty,
    output logic [$clog2(DEPTH):0] count
);

    // Parameter validation (simulation only)
    initial begin
        // Validate DEPTH is power of 2
        if ((DEPTH & (DEPTH - 1)) != 0 || DEPTH < 2) begin
            $fatal(1, "sync_fifo: DEPTH must be power of 2 and >= 2, got %0d", DEPTH);
        end
    end

    // FIFO storage
    logic [WIDTH-1:0] mem [0:DEPTH-1];
    
    // Pointers
    logic [$clog2(DEPTH)-1:0] wr_ptr;
    logic [$clog2(DEPTH)-1:0] rd_ptr;
    
    // Status flags
    assign full  = (count == DEPTH[$clog2(DEPTH):0]);
    assign empty = (count == '0);
    
    // Combinatorial read data output
    assign rdata = mem[rd_ptr];
    
    // FIFO management logic
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            wr_ptr <= '0;
            rd_ptr <= '0;
            count  <= '0;
        end else begin
            // Handle pointer and count updates based on read/write operations
            case ({wr_en && !full, rd_en && !empty})
                2'b10: begin  // Write only
                    wr_ptr <= wr_ptr + 1'b1;
                    count  <= count + 1'b1;
                end
                2'b01: begin  // Read only
                    rd_ptr <= rd_ptr + 1'b1;
                    count  <= count - 1'b1;
                end
                2'b11: begin  // Simultaneous read/write: count unchanged
                    wr_ptr <= wr_ptr + 1'b1;
                    rd_ptr <= rd_ptr + 1'b1;
                end
                default: ;  // 2'b00: no change
            endcase
            
            // Write to FIFO memory (only when write is valid)
            if (wr_en && !full) begin
                mem[wr_ptr] <= wdata;
            end
        end
    end

endmodule

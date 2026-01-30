// Generic Single-Master Bus Module
// Routes memory requests from a single master to multiple slave peripherals
// based on configurable address ranges.
//
// Features:
// - Parameterizable number of slave peripherals (up to 8)
// - Configurable base address and size for each slave
// - Address-based request routing with priority to lower slave indices
// - Optional default slave for catch-all routing (enabled via DEFAULT_SLAVE_IDX >= 0)
// - Response multiplexing from active slave back to master
// - Handles unmapped addresses gracefully (returns zero, asserts ready)
//
// Default Slave Feature:
// When DEFAULT_SLAVE_IDX is set to a valid slave index (0 to NUM_SLAVES-1),
// that slave receives all requests that don't match any other slave's address range.
// This is useful for external memory interfaces that should handle "everything else".
// Set DEFAULT_SLAVE_IDX to -1 to disable this feature (unmapped returns zero).
//
// Port Interface:
// All slave ports use concatenated vectors: {slave[N-1], slave[N-2], ..., slave[0]}
// For a 3-slave bus with 32-bit addresses:
//   slave_addr[95:64] = slave 2 address
//   slave_addr[63:32] = slave 1 address
//   slave_addr[31:0]  = slave 0 address

/* verilator lint_off UNUSEDSIGNAL */
module bus #(
    // Number of slave peripherals connected to this bus (max 8)
    parameter int NUM_SLAVES = 3,
    // Address and data width parameters
    parameter int ADDR_WIDTH = 32,
    parameter int DATA_WIDTH = 32,
    // Default slave index for catch-all routing (-1 to disable)
    // When enabled, this slave receives all unmatched addresses
    parameter int DEFAULT_SLAVE_IDX = -1
) (
    // Clock and reset (unused but kept for potential future registered logic)
    input  logic                              clk,
    input  logic                              rst_n,
    
    // Master interface (from CPU)
    input  logic [ADDR_WIDTH-1:0]             master_addr,
    input  logic [DATA_WIDTH-1:0]             master_wdata,
    output logic [DATA_WIDTH-1:0]             master_rdata,
    input  logic                              master_we,
    input  logic [1:0]                        master_size,
    input  logic                              master_req,
    output logic                              master_ready,
    
    // Slave configuration - concatenated vectors for base addresses and sizes
    // Format: {slave[N-1], slave[N-2], ..., slave[0]}
    // Note: For the default slave (if enabled), base_addr and addr_size are ignored
    input  logic [NUM_SLAVES*ADDR_WIDTH-1:0]  slave_base_addr,
    input  logic [NUM_SLAVES*ADDR_WIDTH-1:0]  slave_addr_size,
    
    // Slave interfaces - concatenated vectors
    // Format: {slave[N-1], slave[N-2], ..., slave[0]}
    output logic [NUM_SLAVES*ADDR_WIDTH-1:0]  slave_addr,
    output logic [NUM_SLAVES*DATA_WIDTH-1:0]  slave_wdata,
    input  logic [NUM_SLAVES*DATA_WIDTH-1:0]  slave_rdata,
    output logic [NUM_SLAVES-1:0]             slave_we,
    output logic [NUM_SLAVES*2-1:0]           slave_size,
    output logic [NUM_SLAVES-1:0]             slave_req,
    input  logic [NUM_SLAVES-1:0]             slave_ready
);
/* verilator lint_on UNUSEDSIGNAL */

    // ============================================================
    // Address Decoder
    // ============================================================
    // Generates select signals for each slave based on address ranges.
    // Priority is given to lower-indexed slaves if ranges overlap.
    // If DEFAULT_SLAVE_IDX is valid, that slave handles unmatched addresses.
    
    logic [NUM_SLAVES-1:0] slave_sel;
    logic                  unmapped_addr;
    
    // Helper signals for address extraction (Yosys compatible)
    logic [ADDR_WIDTH-1:0] base_addr_0, base_addr_1, base_addr_2;
    logic [ADDR_WIDTH-1:0] addr_size_0, addr_size_1, addr_size_2;
    logic [DATA_WIDTH-1:0] rdata_0, rdata_1, rdata_2;
    
    // Extract base addresses from concatenated vector
    assign base_addr_0 = slave_base_addr[ADDR_WIDTH-1:0];
    assign base_addr_1 = slave_base_addr[2*ADDR_WIDTH-1:ADDR_WIDTH];
    assign base_addr_2 = slave_base_addr[3*ADDR_WIDTH-1:2*ADDR_WIDTH];
    
    // Extract address sizes from concatenated vector
    assign addr_size_0 = slave_addr_size[ADDR_WIDTH-1:0];
    assign addr_size_1 = slave_addr_size[2*ADDR_WIDTH-1:ADDR_WIDTH];
    assign addr_size_2 = slave_addr_size[3*ADDR_WIDTH-1:2*ADDR_WIDTH];
    
    // Extract slave read data from concatenated vector
    assign rdata_0 = slave_rdata[DATA_WIDTH-1:0];
    assign rdata_1 = slave_rdata[2*DATA_WIDTH-1:DATA_WIDTH];
    assign rdata_2 = slave_rdata[3*DATA_WIDTH-1:2*DATA_WIDTH];
    
    always_comb begin
        slave_sel = '0;
        unmapped_addr = 1'b1;  // Assume unmapped until proven otherwise
        
        // Check slave 0 (skip if it's the default slave)
        if (!(DEFAULT_SLAVE_IDX == 0)) begin
            if ((master_addr >= base_addr_0) && (master_addr < (base_addr_0 + addr_size_0))) begin
                if (unmapped_addr) begin
                    slave_sel[0] = 1'b1;
                    unmapped_addr = 1'b0;
                end
            end
        end
        
        // Check slave 1 (skip if it's the default slave)
        if (!(DEFAULT_SLAVE_IDX == 1)) begin
            if ((master_addr >= base_addr_1) && (master_addr < (base_addr_1 + addr_size_1))) begin
                if (unmapped_addr) begin
                    slave_sel[1] = 1'b1;
                    unmapped_addr = 1'b0;
                end
            end
        end
        
        // Check slave 2 (skip if it's the default slave)
        if (!(DEFAULT_SLAVE_IDX == 2)) begin
            if ((master_addr >= base_addr_2) && (master_addr < (base_addr_2 + addr_size_2))) begin
                if (unmapped_addr) begin
                    slave_sel[2] = 1'b1;
                    unmapped_addr = 1'b0;
                end
            end
        end
        
        // If still unmapped and default slave is configured, select it
        if (unmapped_addr && DEFAULT_SLAVE_IDX >= 0 && DEFAULT_SLAVE_IDX < NUM_SLAVES) begin
            /* verilator lint_off SELRANGE */
            slave_sel[DEFAULT_SLAVE_IDX] = 1'b1;
            /* verilator lint_on SELRANGE */
            unmapped_addr = 1'b0;
        end
    end
    
    // ============================================================
    // Request Routing to Slaves
    // ============================================================
    // Route master signals to all slaves, but only assert req/we for selected slave
    // Address and data are broadcast to all slaves via concatenation
    
    assign slave_addr = {master_addr, master_addr, master_addr};
    assign slave_wdata = {master_wdata, master_wdata, master_wdata};
    assign slave_size = {master_size, master_size, master_size};
    
    // Request and write enable are only asserted for selected slave
    assign slave_req[0] = master_req && slave_sel[0];
    assign slave_req[1] = master_req && slave_sel[1];
    assign slave_req[2] = master_req && slave_sel[2];
    assign slave_we[0]  = master_we  && slave_sel[0];
    assign slave_we[1]  = master_we  && slave_sel[1];
    assign slave_we[2]  = master_we  && slave_sel[2];
    
    // ============================================================
    // Response Multiplexer
    // ============================================================
    // Select response data and ready signal from the active slave
    
    always_comb begin
        // Default: unmapped address - return zero and assert ready
        master_rdata = {DATA_WIDTH{1'b0}};
        master_ready = 1'b1;
        
        if (slave_sel[0]) begin
            master_rdata = rdata_0;
            master_ready = slave_ready[0];
        end else if (slave_sel[1]) begin
            master_rdata = rdata_1;
            master_ready = slave_ready[1];
        end else if (slave_sel[2]) begin
            master_rdata = rdata_2;
            master_ready = slave_ready[2];
        end
        
        // Handle unmapped addresses explicitly
        if (unmapped_addr) begin
            master_rdata = {DATA_WIDTH{1'b0}};
            master_ready = 1'b1;
        end
    end

endmodule

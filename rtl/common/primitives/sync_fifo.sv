`default_nettype none
// Synchronous FIFO Module
// Generic FIFO with configurable data width and depth
// Uses sync_dpram storage with ready/valid handshakes on both sides
//
// Parameters:
//   WIDTH - Data width in bits (default: 8)
//   DEPTH - Number of entries (must be power of 2, >= 2, default: 8)
//
// Interface:
//   clk      - System clock
//   rst_n    - Synchronous active-low reset
//   wr_valid - Write data is valid this cycle
//   wr_ready - FIFO can accept write data this cycle
//   wdata    - Data to write when wr_valid && wr_ready
//   rd_valid - Read data is valid this cycle (authoritative read-side handshake)
//   rd_ready - Consumer can accept read data this cycle
//   rdata    - Current head word when rd_valid is asserted
//   count    - Number of entries currently in FIFO

module sync_fifo #(
    parameter int WIDTH = 8,
    parameter int DEPTH = 8
) (
    input wire logic             clk,
    input wire logic             rst_n,
    
    // Write interface
    input wire logic             wr_valid,
    output logic             wr_ready,
    input wire logic [WIDTH-1:0] wdata,
    
    // Read interface
    output logic             rd_valid,
    input wire logic             rd_ready,
    output logic [WIDTH-1:0] rdata,
    
    // Occupancy output
    output logic [$clog2(DEPTH):0] count
);

    // Pointer width localparam for readability
    localparam int PTR_WIDTH = $clog2(DEPTH);
    localparam int CNT_WIDTH = PTR_WIDTH + 1;

    // Parameter validation (simulation only)
    initial begin
        // Validate DEPTH is power of 2
        if ((DEPTH & (DEPTH - 1)) != 0 || DEPTH < 2) begin
            $fatal(1, "sync_fifo: DEPTH must be power of 2 and >= 2, got %0d", DEPTH);
        end
    end

    // Pointers
    logic [PTR_WIDTH-1:0] wr_ptr;
    logic [PTR_WIDTH-1:0] rd_ptr;

    // Output staging register keeps rdata aligned with rd_valid, while the load
    // pipeline tracks the two-cycle RAM refill needed before the next head word
    // can be staged from sync_dpram's registered read output.
    logic [WIDTH-1:0] out_data;
    logic             out_valid;
    logic             load_pending_stage1;
    logic             load_pending_stage2;
    logic [WIDTH-1:0] ram_rdata;

    // Internal handshake signals
    logic wr_fire;
    logic rd_fire;
    logic direct_write;
    logic ram_write;
    logic start_load;

    assign rd_valid = out_valid;
    assign rdata    = out_data;
    assign wr_ready = (count < DEPTH[CNT_WIDTH-1:0]) || rd_fire;

    assign wr_fire = wr_valid && wr_ready;
    assign rd_fire = rd_valid && rd_ready;

    // Bypass writes directly into the output register when the queue would otherwise
    // be empty after accounting for a same-cycle read of the current head word.
    // That occurs either when the FIFO is currently empty, or when a same-cycle read
    // is consuming the only staged output word (count == CNT_WIDTH'(1)).
    assign direct_write = wr_fire && (
        ((count == '0) && !rd_fire) ||
        ((count == CNT_WIDTH'(1)) && rd_fire)
    );

    assign ram_write  = wr_fire && !direct_write;
    assign start_load = rd_fire && (count > CNT_WIDTH'(1));
    
    sync_dpram #(
        .DATA_WIDTH(WIDTH),
        .ADDR_WIDTH(PTR_WIDTH)
    ) u_mem (
        .wclk(clk),
        .rclk(clk),
        .we(ram_write),
        .waddr(wr_ptr),
        .wdata(wdata),
        .raddr(rd_ptr),
        .rdata(ram_rdata)
    );
    
    // FIFO management logic
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            wr_ptr <= '0;
            rd_ptr <= '0;
            out_valid <= 1'b0;
            load_pending_stage1 <= 1'b0;
            load_pending_stage2 <= 1'b0;
            count  <= '0;
        end else begin
            if (ram_write) begin
                wr_ptr <= wr_ptr + 1'b1;
            end

            if (start_load) begin
                rd_ptr <= rd_ptr + 1'b1;
            end

            // Single-statement occupancy accounting for concurrent write/read handshakes.
            // rd_fire can only occur when out_valid/rd_valid is high, so count is
            // guaranteed to be non-zero before the decrement term is applied.
            count <= count + CNT_WIDTH'(wr_fire) - CNT_WIDTH'(rd_fire);

            // The load pipeline and rd_fire are mutually exclusive because the load
            // pipeline only exists while out_valid is deasserted, which in turn
            // forces rd_valid low. This capture step must run before the rd_fire
            // handling below so a newly staged head word is ready as soon as the
            // two-cycle RAM refill completes.
            if (load_pending_stage2) begin
                out_data <= ram_rdata;
                out_valid <= 1'b1;
            end

            load_pending_stage2 <= load_pending_stage1;
            load_pending_stage1 <= 1'b0;

            if (rd_fire) begin
                if (direct_write) begin
                    out_data <= wdata;
                    out_valid <= 1'b1;
                    load_pending_stage1 <= 1'b0;
                    load_pending_stage2 <= 1'b0;
                end else if (start_load) begin
                    out_valid <= 1'b0;
                    load_pending_stage1 <= 1'b1;
                    load_pending_stage2 <= 1'b0;
                end else begin
                    out_valid <= 1'b0;
                    load_pending_stage1 <= 1'b0;
                    load_pending_stage2 <= 1'b0;
                end
            end else if (direct_write) begin
                out_data <= wdata;
                out_valid <= 1'b1;
                load_pending_stage1 <= 1'b0;
                load_pending_stage2 <= 1'b0;
            end
        end
    end

endmodule
`default_nettype wire

// Asynchronous FIFO Module
// Generic FIFO with configurable data width and depth for clock domain crossing
// Uses Gray-coded pointers synchronized with ff_sync and sync_dpram for storage
//
// Parameters:
//   WIDTH       - Data width in bits (default: 8)
//   DEPTH       - Number of entries (must be power of 2, >= 2, default: 8)
//   SYNC_STAGES - Number of FF synchronizer stages for CDC pointers (default: 2)

module async_fifo #(
    parameter int WIDTH = 8,
    parameter int DEPTH = 8,
    parameter int SYNC_STAGES = 2
) (
    input  logic             wr_clk,
    input  logic             rd_clk,
    input  logic             rst_n,

    // Write interface (wr_clk domain)
    input  logic             wr_valid,
    output logic             wr_ready,
    input  logic [WIDTH-1:0] wdata,

    // Read interface (rd_clk domain)
    output logic             rd_valid,
    input  logic             rd_ready,
    output logic [WIDTH-1:0] rdata,

    // Status outputs
    output logic [$clog2(DEPTH):0] count // wr_clk domain view
);

    localparam int ADDR_WIDTH = $clog2(DEPTH);
    localparam int PTR_WIDTH  = ADDR_WIDTH + 1;

    logic [PTR_WIDTH-1:0] wr_ptr_bin, wr_ptr_bin_next;
    logic [PTR_WIDTH-1:0] wr_ptr_gray, wr_ptr_gray_next;
    logic [PTR_WIDTH-1:0] rd_ptr_bin, rd_ptr_bin_next;
    logic [PTR_WIDTH-1:0] rd_ptr_gray, rd_ptr_gray_next;

    logic [PTR_WIDTH-1:0] wr_ptr_bin_sync_rd;
    logic [PTR_WIDTH-1:0] wr_ptr_gray_sync_rd;
    logic [PTR_WIDTH-1:0] rd_ptr_gray_sync_wr;
    logic [PTR_WIDTH-1:0] rd_ptr_bin_sync_wr;
    logic [WIDTH-1:0]     out_data;
    logic                 out_valid;
    logic                 load_pending;
    logic [WIDTH-1:0]     ram_rdata;
    logic                 full;

    logic wr_do_write;
    logic rd_fire;
    logic full_next;
    logic start_load;
    logic [PTR_WIDTH-1:0] rd_items_available;
    logic [ADDR_WIDTH-1:0] ram_raddr;

    function automatic logic [PTR_WIDTH-1:0] bin_to_gray(input logic [PTR_WIDTH-1:0] bin);
        bin_to_gray = bin ^ (bin >> 1);
    endfunction

    function automatic logic [PTR_WIDTH-1:0] gray_to_bin(input logic [PTR_WIDTH-1:0] gray);
        integer i;
        gray_to_bin[PTR_WIDTH-1] = gray[PTR_WIDTH-1];
        for (i = PTR_WIDTH - 2; i >= 0; i = i - 1) begin
            gray_to_bin[i] = gray_to_bin[i+1] ^ gray[i];
        end
    endfunction

    function automatic logic [PTR_WIDTH-1:0] full_compare_gray(input logic [PTR_WIDTH-1:0] gray);
        full_compare_gray = gray;
        full_compare_gray[PTR_WIDTH-1 -: 2] = ~gray[PTR_WIDTH-1 -: 2];
    endfunction

    // Parameter validation (simulation only)
    initial begin
        if ((DEPTH & (DEPTH - 1)) != 0 || DEPTH < 2) begin
            $fatal(1, "async_fifo: DEPTH must be power of 2 and >= 2, got %0d", DEPTH);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "async_fifo: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    assign wr_ready    = !full;
    assign rd_valid    = out_valid;
    assign rdata       = out_data;

    assign wr_do_write = wr_valid && wr_ready;
    assign rd_fire     = rd_valid && rd_ready;

    assign wr_ptr_bin_next  = wr_ptr_bin + PTR_WIDTH'(wr_do_write);
    assign wr_ptr_gray_next = bin_to_gray(wr_ptr_bin_next);
    assign rd_ptr_bin_next  = rd_ptr_bin + PTR_WIDTH'(rd_fire);
    assign rd_ptr_gray_next = bin_to_gray(rd_ptr_bin_next);

    assign wr_ptr_bin_sync_rd = gray_to_bin(wr_ptr_gray_sync_rd);
    assign rd_ptr_bin_sync_wr = gray_to_bin(rd_ptr_gray_sync_wr);
    assign count = wr_ptr_bin - rd_ptr_bin_sync_wr;

    assign rd_items_available = wr_ptr_bin_sync_rd - rd_ptr_bin;
    assign full_next  = (wr_ptr_gray_next == full_compare_gray(rd_ptr_gray_sync_wr));
    // Start a BRAM fetch either when the output staging register is empty and data is
    // available, or when the current staged word is being consumed and more words remain.
    assign start_load = (!load_pending) && (
        (!out_valid && (rd_items_available != '0)) ||
        (rd_fire && (rd_items_available > PTR_WIDTH'(1)))
    );
    // While a staged head word is valid, continuously point the RAM read address at the
    // next unread entry so a following rd_fire can immediately launch the refill read.
    assign ram_raddr = out_valid ? (rd_ptr_bin[ADDR_WIDTH-1:0] + ADDR_WIDTH'(1))
                                 : rd_ptr_bin[ADDR_WIDTH-1:0];

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(PTR_WIDTH)
    ) u_wr_ptr_sync (
        .clk(rd_clk),
        .rst_n(rst_n),
        .din(wr_ptr_gray),
        .dout(wr_ptr_gray_sync_rd)
    );

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(PTR_WIDTH)
    ) u_rd_ptr_sync (
        .clk(wr_clk),
        .rst_n(rst_n),
        .din(rd_ptr_gray),
        .dout(rd_ptr_gray_sync_wr)
    );

    sync_dpram #(
        .DATA_WIDTH(WIDTH),
        .ADDR_WIDTH(ADDR_WIDTH)
    ) u_mem (
        .wclk(wr_clk),
        .rclk(rd_clk),
        .we(wr_do_write),
        .waddr(wr_ptr_bin[ADDR_WIDTH-1:0]),
        .wdata(wdata),
        .raddr(ram_raddr),
        .rdata(ram_rdata)
    );

    always_ff @(posedge wr_clk) begin
        if (!rst_n) begin
            wr_ptr_bin  <= '0;
            wr_ptr_gray <= '0;
            full        <= 1'b0;
        end else begin
            wr_ptr_bin  <= wr_ptr_bin_next;
            wr_ptr_gray <= wr_ptr_gray_next;
            full        <= full_next;
        end
    end

    always_ff @(posedge rd_clk) begin
        if (!rst_n) begin
            rd_ptr_bin  <= '0;
            rd_ptr_gray <= '0;
            out_data    <= '0;
            out_valid   <= 1'b0;
            load_pending <= 1'b0;
        end else begin
            // Read-side staging pipeline:
            //   idle    (out_valid=0, load_pending=0) -> loading : start_load launches a BRAM read
            //   loading (load_pending=1)              -> valid   : ram_rdata is captured into out_data
            //   valid   (out_valid=1, load_pending=0) -> loading : rd_fire consumes the head word while more data remains
            if (load_pending) begin
                out_data <= ram_rdata;
                out_valid <= 1'b1;
                load_pending <= 1'b0;
            end

            rd_ptr_bin  <= rd_ptr_bin_next;
            rd_ptr_gray <= rd_ptr_gray_next;

            if (rd_fire) begin
                out_valid <= 1'b0;
                load_pending <= start_load;
            end else if (start_load) begin
                load_pending <= 1'b1;
            end
        end
    end

endmodule

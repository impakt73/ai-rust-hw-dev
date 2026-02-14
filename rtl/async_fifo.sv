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
    input  logic             wr_en,
    input  logic [WIDTH-1:0] wdata,

    // Read interface (rd_clk domain)
    input  logic             rd_en,
    output logic [WIDTH-1:0] rdata,

    // Status outputs
    output logic             full,   // wr_clk domain
    output logic             empty,  // rd_clk domain
    output logic [$clog2(DEPTH):0] count // wr_clk domain view
);

    localparam int ADDR_WIDTH = $clog2(DEPTH);
    localparam int PTR_WIDTH  = ADDR_WIDTH + 1;

    logic [PTR_WIDTH-1:0] wr_ptr_bin, wr_ptr_bin_next;
    logic [PTR_WIDTH-1:0] wr_ptr_gray, wr_ptr_gray_next;
    logic [PTR_WIDTH-1:0] rd_ptr_bin, rd_ptr_bin_next;
    logic [PTR_WIDTH-1:0] rd_ptr_gray, rd_ptr_gray_next;

    logic [PTR_WIDTH-1:0] wr_ptr_gray_sync_rd;
    logic [PTR_WIDTH-1:0] rd_ptr_gray_sync_wr;
    logic [PTR_WIDTH-1:0] rd_ptr_bin_sync_wr;

    logic wr_do_write;
    logic rd_do_read;
    logic full_next;
    logic empty_next;

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

    assign wr_do_write = wr_en && !full;
    assign rd_do_read  = rd_en && !empty;

    assign wr_ptr_bin_next  = wr_ptr_bin + PTR_WIDTH'(wr_do_write);
    assign wr_ptr_gray_next = bin_to_gray(wr_ptr_bin_next);
    assign rd_ptr_bin_next  = rd_ptr_bin + PTR_WIDTH'(rd_do_read);
    assign rd_ptr_gray_next = bin_to_gray(rd_ptr_bin_next);

    assign rd_ptr_bin_sync_wr = gray_to_bin(rd_ptr_gray_sync_wr);
    assign count = wr_ptr_bin - rd_ptr_bin_sync_wr;

    assign full_next  = (wr_ptr_gray_next == full_compare_gray(rd_ptr_gray_sync_wr));
    assign empty_next = (rd_ptr_gray_next == wr_ptr_gray_sync_rd);

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
        .raddr(rd_ptr_bin[ADDR_WIDTH-1:0]),
        .rdata(rdata)
    );

    always_ff @(posedge wr_clk or negedge rst_n) begin
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

    always_ff @(posedge rd_clk or negedge rst_n) begin
        if (!rst_n) begin
            rd_ptr_bin  <= '0;
            rd_ptr_gray <= '0;
            empty       <= 1'b1;
        end else begin
            rd_ptr_bin  <= rd_ptr_bin_next;
            rd_ptr_gray <= rd_ptr_gray_next;
            empty       <= empty_next;
        end
    end

endmodule

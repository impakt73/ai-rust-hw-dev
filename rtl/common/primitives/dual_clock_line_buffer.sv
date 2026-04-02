`default_nettype none
// Dual-clock line buffer
// Stores one raster line per bank in a double-buffered sync_dpram so the write
// and read domains can operate on opposite halves concurrently.
//
// Notes:
// - The storage depth of each bank rounds up to the next power of two above
//   LINE_PIXELS so sync_dpram can infer BRAM-friendly storage.
// - SYNC_STAGES must be at least 2 for safe bank handoff across clock domains.
// - wr_ready indicates the current write bank is safe to use.
// - rd_ready indicates the current read bank contains a completed line.
// - rd_valid/rd_eol are aligned to sync_dpram's two-cycle read latency.

module dual_clock_line_buffer #(
    parameter int PIXEL_WIDTH = 8,
    parameter int LINE_PIXELS = 640,
    parameter int SYNC_STAGES = 2
) (
    input  wire logic                   wr_clk,
    input  wire logic                   rd_clk,
    input  wire logic                   rst,
    input  wire logic                   wr_sof,
    input  wire logic                   rd_sof,

    input  wire logic                   wr_en,
    input  wire logic [PIXEL_WIDTH-1:0] wdata,
    output logic                        wr_ready,
    output logic                        wr_eol,
    output logic                        wr_bank,

    input  wire logic                   rd_en,
    output logic                        rd_ready,
    output logic                        rd_valid,
    output logic [PIXEL_WIDTH-1:0]      rdata,
    output logic                        rd_eol,
    output logic                        rd_bank
);

    function automatic int ceil_pow2(input int value);
        int pow2;
        begin
            pow2 = 1;
            while (pow2 < value) begin
                pow2 = pow2 << 1;
            end
            ceil_pow2 = pow2;
        end
    endfunction

    localparam int BANK_DEPTH      = ceil_pow2(LINE_PIXELS);
    localparam int BANK_ADDR_WIDTH = (BANK_DEPTH <= 2) ? 1 : $clog2(BANK_DEPTH);
    localparam int TOTAL_ADDR_WIDTH = BANK_ADDR_WIDTH + 1;
    localparam logic [BANK_ADDR_WIDTH-1:0] LAST_ADDR = BANK_ADDR_WIDTH'(LINE_PIXELS - 1);

    localparam int LINE_PTR_WIDTH = 2;

    logic                        wr_rst;
    logic                        rd_rst;
    logic                        wr_fire;
    logic                        rd_fire;
    logic                        wr_last;
    logic                        rd_last;
    logic                        rd_pending_stage1;
    logic                        rd_pending_stage2;
    logic                        rd_eol_stage1;
    logic                        rd_eol_stage2;
    logic                        full;
    logic                        empty;
    logic [BANK_ADDR_WIDTH-1:0]  wr_addr;
    logic [BANK_ADDR_WIDTH-1:0]  rd_addr;
    logic [PIXEL_WIDTH-1:0]      ram_rdata;
    logic [TOTAL_ADDR_WIDTH-1:0] ram_waddr;
    logic [TOTAL_ADDR_WIDTH-1:0] ram_raddr;
    logic [LINE_PTR_WIDTH-1:0]   wr_line_ptr_bin;
    logic [LINE_PTR_WIDTH-1:0]   wr_line_ptr_gray;
    logic [LINE_PTR_WIDTH-1:0]   rd_line_ptr_bin;
    logic [LINE_PTR_WIDTH-1:0]   rd_line_ptr_gray;
    logic [LINE_PTR_WIDTH-1:0]   wr_line_ptr_gray_sync_rd;
    logic [LINE_PTR_WIDTH-1:0]   rd_line_ptr_gray_sync_wr;

    function automatic logic [LINE_PTR_WIDTH-1:0] bin_to_gray(
        input logic [LINE_PTR_WIDTH-1:0] bin
    );
        bin_to_gray = bin ^ (bin >> 1);
    endfunction

    function automatic logic [LINE_PTR_WIDTH-1:0] full_compare_gray(
        input logic [LINE_PTR_WIDTH-1:0] gray
    );
        full_compare_gray = gray;
        full_compare_gray[LINE_PTR_WIDTH-1 -: 2] = ~gray[LINE_PTR_WIDTH-1 -: 2];
    endfunction

    initial begin
        if (LINE_PIXELS < 2) begin
            $fatal(1, "dual_clock_line_buffer: LINE_PIXELS must be >= 2, got %0d", LINE_PIXELS);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "dual_clock_line_buffer: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    assign wr_rst = rst || wr_sof;
    assign rd_rst = rst || rd_sof;

    assign wr_bank = wr_line_ptr_bin[0];
    assign rd_bank = rd_line_ptr_bin[0];

    assign full = (wr_line_ptr_gray == full_compare_gray(rd_line_ptr_gray_sync_wr));
    assign empty = (wr_line_ptr_gray_sync_rd == rd_line_ptr_gray);

    assign wr_ready = !wr_rst && ((wr_addr != '0) || !full);
    assign rd_ready = !rd_rst && ((rd_addr != '0) || !empty);

    assign wr_fire = wr_en && wr_ready && !wr_rst;
    assign rd_fire = rd_en && rd_ready && !rd_rst;

    assign wr_last = (wr_addr == LAST_ADDR);
    assign rd_last = (rd_addr == LAST_ADDR);

    assign ram_waddr = {wr_bank, wr_addr};
    assign ram_raddr = {rd_bank, rd_addr};

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(LINE_PTR_WIDTH)
    ) u_rd_line_ptr_sync (
        .clk(wr_clk),
        .rst(wr_rst),
        .din(rd_line_ptr_gray),
        .dout(rd_line_ptr_gray_sync_wr)
    );

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(LINE_PTR_WIDTH)
    ) u_wr_line_ptr_sync (
        .clk(rd_clk),
        .rst(rd_rst),
        .din(wr_line_ptr_gray),
        .dout(wr_line_ptr_gray_sync_rd)
    );

    sync_dpram #(
        .DATA_WIDTH(PIXEL_WIDTH),
        .ADDR_WIDTH(TOTAL_ADDR_WIDTH)
    ) u_mem (
        .wclk(wr_clk),
        .rclk(rd_clk),
        .we(wr_fire),
        .waddr(ram_waddr),
        .wdata(wdata),
        .raddr(ram_raddr),
        .rdata(ram_rdata)
    );

    always_ff @(posedge wr_clk) begin
        if (wr_rst) begin
            wr_line_ptr_bin <= '0;
            wr_line_ptr_gray <= '0;
            wr_addr <= '0;
            wr_eol <= 1'b0;
        end else if (wr_fire) begin
            wr_eol <= wr_last;
            if (wr_last) begin
                wr_line_ptr_bin <= wr_line_ptr_bin + LINE_PTR_WIDTH'(1);
                wr_line_ptr_gray <= bin_to_gray(wr_line_ptr_bin + LINE_PTR_WIDTH'(1));
                wr_addr <= '0;
            end else begin
                wr_addr <= wr_addr + 1'b1;
            end
        end else begin
            wr_eol <= 1'b0;
        end
    end

    always_ff @(posedge rd_clk) begin
        if (rd_rst) begin
            rd_line_ptr_bin <= '0;
            rd_line_ptr_gray <= '0;
            rd_addr <= '0;
            rd_valid <= 1'b0;
            rd_eol <= 1'b0;
            rd_pending_stage1 <= 1'b0;
            rd_pending_stage2 <= 1'b0;
            rd_eol_stage1 <= 1'b0;
            rd_eol_stage2 <= 1'b0;
        end else begin
            rd_valid <= 1'b0;
            rd_eol <= 1'b0;

            if (rd_pending_stage2) begin
                rdata <= ram_rdata;
                rd_valid <= 1'b1;
                rd_eol <= rd_eol_stage2;
            end

            rd_pending_stage2 <= rd_pending_stage1;
            rd_pending_stage1 <= rd_fire;
            rd_eol_stage2 <= rd_eol_stage1;
            rd_eol_stage1 <= rd_fire && rd_last;

            if (rd_fire) begin
                if (rd_last) begin
                    rd_line_ptr_bin <= rd_line_ptr_bin + LINE_PTR_WIDTH'(1);
                    rd_line_ptr_gray <= bin_to_gray(rd_line_ptr_bin + LINE_PTR_WIDTH'(1));
                    rd_addr <= '0;
                end else begin
                    rd_addr <= rd_addr + 1'b1;
                end
            end
        end
    end

endmodule
`default_nettype wire

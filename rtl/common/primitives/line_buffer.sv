`default_nettype none
// Line Buffer - Double-Buffered Pixel Clock Domain Crossing
// Generic double-buffered line buffer for crossing pixel data between a fast
// write clock domain and a slower read clock domain.  Uses ping-pong (double)
// buffering inside a single dual-port RAM whose address MSB selects the active
// bank.  The write side stores a complete scan line, then swaps banks.  The
// read side drains a complete line from the opposite bank before swapping.
//
// The read-side staging register follows the same 2-stage load-pending pipeline
// used by async_fifo, which hides the sync_dpram 2-cycle read latency behind
// a continuously pre-fetched output register.
//
// Parameters:
//   PIXEL_WIDTH    - Width of a single pixel in bits (default: 8)
//   MAX_LINE_WIDTH - Maximum pixels per line, must be power of 2 (default: 1024)
//   SYNC_STAGES    - Number of ff_sync synchronizer stages for CDC (default: 3)
//
// Write-domain interface (wr_clk):
//   wr_clk   - Write clock (typically the high-speed pixel clock)
//   wr_rst   - Synchronous active-high reset
//   wr_data  - Pixel data input [PIXEL_WIDTH-1:0]
//   wr_valid - Pixel data is valid this cycle
//   wr_ready - Module can accept a pixel (output, deasserted when stalled)
//   wr_eol   - End of line, asserted WITH the last valid pixel of a line
//   wr_sof   - Start of frame, resets all internal pointers to zero
//
// Read-domain interface (rd_clk):
//   rd_clk   - Read clock (typically the lower-speed consumer clock)
//   rd_rst   - Synchronous active-high reset
//   rd_data  - Pixel data output [PIXEL_WIDTH-1:0]
//   rd_valid - Pixel data is valid (output)
//   rd_ready - Consumer can accept a pixel (input)
//   rd_eol   - Last pixel of line (output, asserted with last valid pixel)
//   rd_sof   - First pixel of a new frame (output, asserted for first pixel after SOF)

module line_buffer #(
    parameter int PIXEL_WIDTH    = 8,
    parameter int MAX_LINE_WIDTH = 1024,
    parameter int SYNC_STAGES    = 3
) (
    // Write clock domain
    input  wire logic                    wr_clk,
    input  wire logic                    wr_rst,
    input  wire logic [PIXEL_WIDTH-1:0]  wr_data,
    input  wire logic                    wr_valid,
    output logic                         wr_ready,
    input  wire logic                    wr_eol,
    input  wire logic                    wr_sof,

    // Read clock domain
    input  wire logic                    rd_clk,
    input  wire logic                    rd_rst,
    output logic [PIXEL_WIDTH-1:0]       rd_data,
    output logic                         rd_valid,
    input  wire logic                    rd_ready,
    output logic                         rd_eol,
    output logic                         rd_sof
);

    // ----------------------------------------------------------------
    // Local parameters
    // ----------------------------------------------------------------
    localparam int LINE_ADDR_WIDTH  = $clog2(MAX_LINE_WIDTH);
    localparam int DPRAM_ADDR_WIDTH = LINE_ADDR_WIDTH + 1; // MSB = bank select

    // ----------------------------------------------------------------
    // Parameter validation (simulation only)
    // ----------------------------------------------------------------
    initial begin
        if ((MAX_LINE_WIDTH & (MAX_LINE_WIDTH - 1)) != 0 || MAX_LINE_WIDTH < 2) begin
            $fatal(1, "line_buffer: MAX_LINE_WIDTH must be power of 2 and >= 2, got %0d",
                   MAX_LINE_WIDTH);
        end
        if (PIXEL_WIDTH < 1) begin
            $fatal(1, "line_buffer: PIXEL_WIDTH must be >= 1, got %0d", PIXEL_WIDTH);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "line_buffer: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    // ================================================================
    //  Write-domain signals
    // ================================================================
    logic                        wr_bank;        // Active write bank (0 or 1)
    logic [LINE_ADDR_WIDTH-1:0]  wr_addr;        // Pixel index within current line
    logic                        wr_active;      // Writer is accepting pixels
    logic                        wr_sof_toggle;  // Toggles on each SOF event
    logic                        wr_sof_stall;   // Writer stalled waiting for SOF ack
    logic                        wr_sof_prev;    // Previous wr_sof for edge detection

    // Per-buffer line length (index of last pixel written)
    logic [LINE_ADDR_WIDTH-1:0]  wr_line_len [0:1];

    // rd_bank synchronized into write domain
    logic                        rd_bank_synced_wr;

    // Combinational write-side helpers
    logic                        wr_do_write;
    logic                        wr_next_buf_free;
    logic                        wr_sof_edge;    // Rising edge of wr_sof

    // ================================================================
    //  Read-domain signals
    // ================================================================
    logic                        rd_bank;        // Active read bank (0 or 1)
    logic [LINE_ADDR_WIDTH-1:0]  rd_addr;        // Pixel index within current line

    // Output staging register (async_fifo pattern)
    logic                        rd_out_valid;
    logic [PIXEL_WIDTH-1:0]      rd_out_data;

    // 2-stage load-pending pipeline (matches sync_dpram 2-cycle read latency)
    logic                        rd_load_s1;
    logic                        rd_load_s2;
    logic                        rd_load_pending;

    // SOF tracking in read domain
    logic                        rd_sof_pending;
    logic                        rd_sof_toggle_prev;

    // CDC outputs in read domain
    logic                        wr_bank_synced_rd;
    logic                        wr_sof_toggle_synced;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_0;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_1;

    // SOF ack from read domain → write domain
    logic                        rd_sof_ack_synced_wr;

    // Gray-coded line length intermediates for CDC-safe transfer
    logic [LINE_ADDR_WIDTH-1:0]  wr_line_len_0_gray;
    logic [LINE_ADDR_WIDTH-1:0]  wr_line_len_1_gray;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_0_gray;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_1_gray;

    // Stability filter: only accept synchronized Gray values after they settle
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_0_gray_prev;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_0_gray_stable;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_1_gray_prev;
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len_1_gray_stable;

    // Combinational read-side helpers
    logic [LINE_ADDR_WIDTH-1:0]  rd_line_len;
    logic                        rd_has_line;
    logic                        rd_is_last;
    logic                        rd_fire;
    logic                        rd_start_load;
    logic                        rd_sof_edge;

    // DPRAM interface signals
    logic [DPRAM_ADDR_WIDTH-1:0] ram_waddr;
    logic [DPRAM_ADDR_WIDTH-1:0] ram_raddr;
    logic [PIXEL_WIDTH-1:0]      ram_rdata;

    // ================================================================
    //  Combinational logic
    // ================================================================

    // -- Write side --
    assign wr_ready        = wr_active;
    assign wr_do_write     = wr_valid && wr_ready;
    assign wr_next_buf_free = (wr_bank == rd_bank_synced_wr);
    assign wr_sof_edge     = wr_sof && !wr_sof_prev;

    // -- Read side --
    assign rd_valid        = rd_out_valid;
    assign rd_data         = rd_out_data;
    assign rd_fire         = rd_valid && rd_ready;
    assign rd_load_pending = rd_load_s1 || rd_load_s2;

    assign rd_line_len     = rd_bank ? rd_line_len_1 : rd_line_len_0;
    assign rd_has_line     = (rd_bank != wr_bank_synced_rd);
    assign rd_is_last      = (rd_addr == rd_line_len);

    assign rd_eol          = rd_out_valid && rd_is_last;
    assign rd_sof          = rd_out_valid && rd_sof_pending;

    // Start a DPRAM fetch when no load is in flight and either:
    //   (a) the output is empty and a complete line is available, or
    //   (b) the consumer just took a pixel and more pixels remain in the line.
    assign rd_start_load = (!rd_load_pending) && (
        (!rd_out_valid && rd_has_line) ||
        (rd_fire && !rd_is_last)
    );

    // SOF edge detection: wr_sof_toggle crossed into read domain vs. previous
    assign rd_sof_edge = (wr_sof_toggle_synced != rd_sof_toggle_prev);

    // -- DPRAM addresses --
    assign ram_waddr = {wr_bank, wr_addr};

    // While the staging register holds a valid pixel, continuously present the
    // next address to the DPRAM so a subsequent rd_fire can launch its refill
    // read without an extra cycle of address setup.
    assign ram_raddr = {rd_bank, rd_out_valid ? (rd_addr + LINE_ADDR_WIDTH'(1))
                                              : rd_addr};

    // ================================================================
    //  Gray-code helpers for CDC-safe multi-bit transfer
    // ================================================================

    function automatic logic [LINE_ADDR_WIDTH-1:0] bin2gray(
        input logic [LINE_ADDR_WIDTH-1:0] bin
    );
        bin2gray = (bin >> 1) ^ bin;
    endfunction

    function automatic logic [LINE_ADDR_WIDTH-1:0] gray2bin(
        input logic [LINE_ADDR_WIDTH-1:0] gray
    );
        logic [LINE_ADDR_WIDTH-1:0] bin;
        bin[LINE_ADDR_WIDTH-1] = gray[LINE_ADDR_WIDTH-1];
        for (int i = LINE_ADDR_WIDTH-2; i >= 0; i--) begin
            bin[i] = bin[i+1] ^ gray[i];
        end
        gray2bin = bin;
    endfunction

    // Encode line lengths in Gray code (write domain)
    assign wr_line_len_0_gray = bin2gray(wr_line_len[0]);
    assign wr_line_len_1_gray = bin2gray(wr_line_len[1]);

    // ================================================================
    //  CDC synchronisers (ff_sync instances)
    // ================================================================

    // wr_bank → read domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (1)
    ) u_wr_bank_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_bank),
        .dout (wr_bank_synced_rd)
    );

    // rd_bank → write domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (1)
    ) u_rd_bank_sync (
        .clk  (wr_clk),
        .rst  (wr_rst),
        .din  (rd_bank),
        .dout (rd_bank_synced_wr)
    );

    // Line length for buffer 0 → read domain (Gray-coded CDC)
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (LINE_ADDR_WIDTH)
    ) u_line_len_0_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_line_len_0_gray),
        .dout (rd_line_len_0_gray)
    );

    // Line length for buffer 1 → read domain (Gray-coded CDC)
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (LINE_ADDR_WIDTH)
    ) u_line_len_1_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_line_len_1_gray),
        .dout (rd_line_len_1_gray)
    );

    // Decode Gray-coded lengths back to binary in read domain.
    // To avoid decoding transient, potentially invalid Gray words caused by
    // non-adjacent multi-bit changes at EOL, require the synchronized Gray
    // value to be stable for at least one full rd_clk cycle before accepting it.
    always_ff @(posedge rd_clk) begin
        if (rd_rst) begin
            rd_line_len_0_gray_prev   <= '0;
            rd_line_len_0_gray_stable <= '0;
            rd_line_len_1_gray_prev   <= '0;
            rd_line_len_1_gray_stable <= '0;
        end else begin
            if (rd_line_len_0_gray == rd_line_len_0_gray_prev) begin
                rd_line_len_0_gray_stable <= rd_line_len_0_gray;
            end
            rd_line_len_0_gray_prev <= rd_line_len_0_gray;

            if (rd_line_len_1_gray == rd_line_len_1_gray_prev) begin
                rd_line_len_1_gray_stable <= rd_line_len_1_gray;
            end
            rd_line_len_1_gray_prev <= rd_line_len_1_gray;
        end
    end

    assign rd_line_len_0 = gray2bin(rd_line_len_0_gray_stable);
    assign rd_line_len_1 = gray2bin(rd_line_len_1_gray_stable);

    // wr_sof_toggle → read domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (1)
    ) u_sof_toggle_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_sof_toggle),
        .dout (wr_sof_toggle_synced)
    );

    // SOF ack: rd_sof_toggle_prev → write domain (confirms reader processed SOF)
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH (1)
    ) u_sof_ack_sync (
        .clk  (wr_clk),
        .rst  (wr_rst),
        .din  (rd_sof_toggle_prev),
        .dout (rd_sof_ack_synced_wr)
    );

    // ================================================================
    //  Dual-port RAM (single instance, MSB selects bank)
    // ================================================================
    sync_dpram #(
        .DATA_WIDTH(PIXEL_WIDTH),
        .ADDR_WIDTH(DPRAM_ADDR_WIDTH)
    ) u_dpram (
        .wclk  (wr_clk),
        .rclk  (rd_clk),
        .we    (wr_do_write),
        .waddr (ram_waddr),
        .wdata (wr_data),
        .raddr (ram_raddr),
        .rdata (ram_rdata)
    );

    // ================================================================
    //  Write-side sequential logic  (wr_clk domain)
    // ================================================================
    always_ff @(posedge wr_clk) begin
        if (wr_rst) begin
            wr_bank       <= 1'b0;
            wr_addr       <= '0;
            wr_active     <= 1'b1;
            wr_sof_toggle <= 1'b0;
            wr_sof_stall  <= 1'b0;
            wr_sof_prev   <= 1'b0;
            // NOTE: wr_line_len[] is datapath payload — intentionally not reset.
            // Safe because a complete write (with EOL) always stores a valid length
            // before the read side can access it via the bank-swap handshake.
        end else begin
            wr_sof_prev <= wr_sof;

            // SOF rising edge resets write address, marks a new frame, and stalls
            // until the reader acknowledges to avoid same-bank read/write collisions.
            // Edge-detected to prevent multi-toggle if wr_sof is held >1 cycle.
            if (wr_sof_edge) begin
                wr_bank       <= 1'b0;
                wr_addr       <= '0;
                wr_active     <= 1'b0;
                wr_sof_toggle <= ~wr_sof_toggle;
                wr_sof_stall  <= 1'b1;
            end else if (wr_do_write) begin
                if (wr_eol) begin
                    // Record the index of the last pixel as the line length.
                    wr_line_len[wr_bank] <= wr_addr;

                    // Attempt to swap to the next buffer.
                    if (wr_next_buf_free) begin
                        wr_bank <= ~wr_bank;
                        wr_addr <= '0;
                    end else begin
                        // Next buffer is still being read — stall.
                        wr_active <= 1'b0;
                    end
                end else begin
                    wr_addr <= wr_addr + LINE_ADDR_WIDTH'(1);
`ifndef SYNTHESIS
                    if (wr_addr == LINE_ADDR_WIDTH'(MAX_LINE_WIDTH - 1)) begin
                        $fatal(1,
                               "line_buffer: wr_addr overflow — %0d pixels without EOL (MAX_LINE_WIDTH=%0d)",
                               MAX_LINE_WIDTH, MAX_LINE_WIDTH);
                    end
`endif
                end
            end else if (!wr_active) begin
                if (wr_sof_stall) begin
                    // Stalled after SOF: wait for reader to acknowledge SOF.
                    if (rd_sof_ack_synced_wr == wr_sof_toggle) begin
                        wr_active    <= 1'b1;
                        wr_sof_stall <= 1'b0;
                    end
                end else begin
                    // Stalled after EOL: poll until the next buffer is released.
                    if (wr_next_buf_free) begin
                        wr_bank   <= ~wr_bank;
                        wr_addr   <= '0;
                        wr_active <= 1'b1;
                    end
                end
            end
        end
    end

    // ================================================================
    //  Read-side sequential logic  (rd_clk domain)
    // ================================================================
    always_ff @(posedge rd_clk) begin
        if (rd_rst) begin
            rd_bank            <= 1'b0;
            rd_addr            <= '0;
            rd_out_valid       <= 1'b0;
            rd_load_s1         <= 1'b0;
            rd_load_s2         <= 1'b0;
            rd_sof_pending     <= 1'b0;
            rd_sof_toggle_prev <= 1'b0;
            // NOTE: rd_out_data is datapath payload — intentionally not reset.
            // Safely ignored when rd_out_valid is low after reset.
        end else begin
            // ---------------------------------------------------------
            // SOF edge: hard-reset the entire read side for the new frame.
            // ---------------------------------------------------------
            if (rd_sof_edge) begin
                rd_bank            <= 1'b0;
                rd_addr            <= '0;
                rd_out_valid       <= 1'b0;
                rd_load_s1         <= 1'b0;
                rd_load_s2         <= 1'b0;
                rd_sof_pending     <= 1'b1;
                rd_sof_toggle_prev <= wr_sof_toggle_synced;
            end else begin
                // -------------------------------------------------
                // Read-side staging pipeline (mirrors async_fifo):
                //   idle     (out_valid=0, pending=0) → loading
                //   loading  (pending=1)              → valid
                //   valid    (out_valid=1, pending=0)  → loading (on rd_fire)
                // -------------------------------------------------

                // Stage 2 completes: capture DPRAM output into staging register.
                if (rd_load_s2) begin
                    rd_out_data  <= ram_rdata;
                    rd_out_valid <= 1'b1;
                end

                // Shift the load pipeline forward.
                rd_load_s2 <= rd_load_s1;
                rd_load_s1 <= 1'b0;

                if (rd_fire) begin
                    // Consumer accepted the staged pixel.
                    rd_out_valid <= 1'b0;
                    rd_load_s2  <= 1'b0;
                    rd_load_s1  <= rd_start_load;

                    // Clear SOF flag once the first pixel of the frame is consumed.
                    if (rd_sof_pending) begin
                        rd_sof_pending <= 1'b0;
                    end

                    if (rd_is_last) begin
                        // Last pixel of line — release this bank, move to next.
                        rd_bank <= ~rd_bank;
                        rd_addr <= '0;
                    end else begin
                        // More pixels remain — advance the address.
                        rd_addr <= rd_addr + LINE_ADDR_WIDTH'(1);
                    end
                end else if (rd_start_load) begin
                    // No rd_fire this cycle but a load can start (initial or idle).
                    rd_load_s1 <= 1'b1;
                end
            end
        end
    end

endmodule
`default_nettype wire

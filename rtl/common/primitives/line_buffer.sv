`default_nettype none
// Line Buffer Module
// Double-buffered line buffer for transferring pixel data from a high-speed
// write clock domain to a low-speed read clock domain.
//
// Architecture:
//   A single sync_dpram is divided into two halves by the MSB of the address,
//   creating two line buffers.  The writer fills one half while the reader
//   drains the other.  The buffer-select bit (pointer MSB) of each side is
//   synchronized into the opposite clock domain with ff_sync so that each
//   side can determine whether it may proceed:
//
//     Writer ready:  wr_buf_sel == rd_buf_sel_sync   (reader is not using
//                    the writer's current buffer)
//     Line available: wr_buf_sel_sync != rd_buf_sel  (writer has completed
//                     a buffer the reader has not yet consumed)
//
// Parameters:
//   PIXEL_WIDTH    - Width of a single pixel in bits (default: 24)
//   MAX_LINE_WIDTH - Maximum number of pixels per line (default: 1024,
//                    must be >= 2)
//   SYNC_STAGES    - Number of ff_sync stages for CDC (default: 3)
//
// Write Interface (wr_clk domain):
//   wr_clk             - Write clock (high-speed domain)
//   wr_rst              - Synchronous active-high reset
//   wr_start_of_frame   - Resets all write-side pointers (1-cycle pulse)
//   wr_end_of_line      - Assert WITH the last wr_valid pixel of a line
//   wr_valid            - Pixel data is valid
//   wr_data             - Pixel data input
//   wr_ready            - Module can accept pixel data (output)
//
// Read Interface (rd_clk domain):
//   rd_clk             - Read clock (low-speed domain)
//   rd_rst              - Synchronous active-high reset
//   rd_start_of_frame   - Resets all read-side pointers (1-cycle pulse,
//                         directly from rd_clk domain or sync'd externally)
//   rd_line_valid       - A complete line is available for reading (output)
//   rd_ready            - Consumer requests / acknowledges a pixel (input)
//   rd_valid            - Pixel data is valid on rd_data (output)
//   rd_data             - Pixel data output
//   rd_end_of_line      - Asserted with rd_valid for the last pixel (output)

module line_buffer #(
    parameter int PIXEL_WIDTH    = 24,
    parameter int MAX_LINE_WIDTH = 1024,
    parameter int SYNC_STAGES    = 3
) (
    // Write interface (high-speed clock domain)
    input wire logic                   wr_clk,
    input wire logic                   wr_rst,
    input wire logic                   wr_start_of_frame,
    input wire logic                   wr_end_of_line,
    input wire logic                   wr_valid,
    input wire logic [PIXEL_WIDTH-1:0] wr_data,
    output logic                       wr_ready,

    // Read interface (low-speed clock domain)
    input wire logic                   rd_clk,
    input wire logic                   rd_rst,
    input wire logic                   rd_start_of_frame,
    output logic                       rd_line_valid,
    input wire logic                   rd_ready,
    output logic                       rd_valid,
    output logic [PIXEL_WIDTH-1:0]     rd_data,
    output logic                       rd_end_of_line
);

    // =====================================================================
    // Local parameters
    // =====================================================================
    localparam int PIX_IDX_WIDTH = $clog2(MAX_LINE_WIDTH);
    // DPRAM address: {buffer_select (1 bit), pixel_index (PIX_IDX_WIDTH bits)}
    localparam int ADDR_WIDTH = 1 + PIX_IDX_WIDTH;

    // Parameter validation (simulation only)
    initial begin
        if (MAX_LINE_WIDTH < 2) begin
            $fatal(1, "line_buffer: MAX_LINE_WIDTH must be >= 2, got %0d",
                   MAX_LINE_WIDTH);
        end
    end

    // =====================================================================
    // Write-domain signals
    // =====================================================================
    logic                     wr_buf_sel;          // Current write buffer (0 or 1)
    logic [PIX_IDX_WIDTH-1:0] wr_pixel_idx;        // Pixel position within current line
    logic                     rd_buf_sel_sync_wr;  // Read buf-sel synchronized to wr_clk
    // Per-buffer line length, set when writer completes a line.
    // Consumed by the read side after the synchronized handshake guarantees
    // the value is stable (quasi-static CDC data with synchronized strobe).
    logic [PIX_IDX_WIDTH-1:0] wr_line_length [0:1];

    // =====================================================================
    // Read-domain signals
    // =====================================================================
    logic                     rd_buf_sel;          // Current read buffer (0 or 1)
    logic [PIX_IDX_WIDTH-1:0] rd_pixel_idx;        // Pixel position within current line
    logic                     wr_buf_sel_sync_rd;  // Write buf-sel synchronized to rd_clk
    logic [PIX_IDX_WIDTH-1:0] rd_line_len;         // Latched line length for current read

    // SOF toggle for CDC from wr_clk → rd_clk
    logic                     wr_sof_toggle;
    logic                     rd_sof_toggle_sync;
    logic                     rd_sof_toggle_prev;
    logic                     rd_sof_edge;

    // Read-side FSM
    //   RD_IDLE    : Waiting for a completed line to become available.
    //   RD_FETCH_1 : First DPRAM pipeline cycle (address captured by DPRAM).
    //   RD_FETCH_2 : Second DPRAM pipeline cycle (data propagating to output).
    //   RD_DATA    : Pixel data valid on rd_data, waiting for rd_ready.
    typedef enum logic [1:0] {
        RD_IDLE,
        RD_FETCH_1,
        RD_FETCH_2,
        RD_DATA
    } rd_state_t;
    rd_state_t rd_state;

    // =====================================================================
    // CDC synchronizers
    // =====================================================================

    // Write buffer-select → read domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_wr_buf_sel_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_buf_sel),
        .dout (wr_buf_sel_sync_rd)
    );

    // Read buffer-select → write domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_rd_buf_sel_sync (
        .clk  (wr_clk),
        .rst  (wr_rst),
        .din  (rd_buf_sel),
        .dout (rd_buf_sel_sync_wr)
    );

    // Start-of-frame toggle → read domain
    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_sof_sync (
        .clk  (rd_clk),
        .rst  (rd_rst),
        .din  (wr_sof_toggle),
        .dout (rd_sof_toggle_sync)
    );

    // =====================================================================
    // Dual-port RAM (double-buffered pixel storage)
    // =====================================================================
    // Address = {buffer_select, pixel_index}
    logic                   ram_we;
    logic [ADDR_WIDTH-1:0]  ram_waddr;
    logic [PIXEL_WIDTH-1:0] ram_wdata;
    logic [ADDR_WIDTH-1:0]  ram_raddr;
    logic [PIXEL_WIDTH-1:0] ram_rdata;

    sync_dpram #(
        .DATA_WIDTH(PIXEL_WIDTH),
        .ADDR_WIDTH(ADDR_WIDTH)
    ) u_dpram (
        .wclk  (wr_clk),
        .rclk  (rd_clk),
        .we    (ram_we),
        .waddr (ram_waddr),
        .wdata (ram_wdata),
        .raddr (ram_raddr),
        .rdata (ram_rdata)
    );

    // =====================================================================
    // Write-domain logic
    // =====================================================================

    // The writer may proceed when its current buffer is not the one the
    // reader is draining.  When both buffer-select bits are equal the reader
    // has finished (or has not yet started) with the writer's target buffer.
    logic wr_can_write;
    assign wr_can_write = (wr_buf_sel == rd_buf_sel_sync_wr);

    assign wr_ready  = wr_can_write;
    assign ram_we    = wr_valid && wr_ready;
    assign ram_waddr = {wr_buf_sel, wr_pixel_idx};
    assign ram_wdata = wr_data;

    always_ff @(posedge wr_clk) begin
        if (wr_rst) begin
            wr_buf_sel        <= 1'b0;
            wr_pixel_idx      <= '0;
            wr_sof_toggle     <= 1'b0;
            wr_line_length[0] <= '0;
            wr_line_length[1] <= '0;
        end else if (wr_start_of_frame) begin
            wr_buf_sel    <= 1'b0;
            wr_pixel_idx  <= '0;
            wr_sof_toggle <= ~wr_sof_toggle;
        end else if (wr_valid && wr_ready) begin
            if (wr_end_of_line) begin
                // Record line length (index + 1 = number of pixels written)
                wr_line_length[wr_buf_sel] <= wr_pixel_idx + PIX_IDX_WIDTH'(1);
                // Switch to other buffer and reset pixel index
                wr_buf_sel   <= ~wr_buf_sel;
                wr_pixel_idx <= '0;
            end else begin
                wr_pixel_idx <= wr_pixel_idx + PIX_IDX_WIDTH'(1);
            end
        end
    end

    // =====================================================================
    // Read-domain logic
    // =====================================================================

    // A completed line is available when the synchronized write buffer-select
    // differs from the read buffer-select (the writer has moved on).
    logic rd_line_avail;
    assign rd_line_avail = (wr_buf_sel_sync_rd != rd_buf_sel);

    // SOF edge detection
    assign rd_sof_edge = (rd_sof_toggle_sync != rd_sof_toggle_prev);

    // Expose line-available status (only meaningful when idle)
    assign rd_line_valid = rd_line_avail && (rd_state == RD_IDLE);

    // DPRAM read address
    assign ram_raddr = {rd_buf_sel, rd_pixel_idx};

    // Last-pixel flag for the current read position
    logic rd_is_last_pixel;
    assign rd_is_last_pixel = (rd_pixel_idx == rd_line_len - PIX_IDX_WIDTH'(1));

    // Outputs
    assign rd_valid       = (rd_state == RD_DATA);
    assign rd_data        = ram_rdata;
    assign rd_end_of_line = rd_valid && rd_is_last_pixel;

    always_ff @(posedge rd_clk) begin
        if (rd_rst || rd_sof_edge || rd_start_of_frame) begin
            rd_buf_sel         <= 1'b0;
            rd_pixel_idx       <= '0;
            rd_state           <= RD_IDLE;
            rd_line_len        <= '0;
            rd_sof_toggle_prev <= rd_sof_toggle_sync;
        end else begin
            rd_sof_toggle_prev <= rd_sof_toggle_sync;

            case (rd_state)
                RD_IDLE: begin
                    rd_pixel_idx <= '0;
                    if (rd_line_avail && rd_ready) begin
                        // Latch line length.  Safe to sample here because
                        // the synchronized wr_buf_sel guarantees the value
                        // was written at least SYNC_STAGES rd_clk cycles ago.
                        rd_line_len <= wr_line_length[rd_buf_sel];
                        rd_state    <= RD_FETCH_1;
                    end
                end

                RD_FETCH_1: begin
                    // DPRAM pipeline stage 1: address is presented on
                    // ram_raddr; the DPRAM captures mem[raddr] into
                    // its internal pipeline register at this posedge.
                    rd_state <= RD_FETCH_2;
                end

                RD_FETCH_2: begin
                    // DPRAM pipeline stage 2: rdata now holds the pixel
                    // for the current rd_pixel_idx.
                    rd_state <= RD_DATA;
                end

                RD_DATA: begin
                    if (rd_ready) begin
                        if (rd_is_last_pixel) begin
                            // Line complete – release the buffer
                            rd_buf_sel   <= ~rd_buf_sel;
                            rd_pixel_idx <= '0;
                            rd_state     <= RD_IDLE;
                        end else begin
                            // Advance to next pixel and re-enter the
                            // two-cycle fetch pipeline.
                            rd_pixel_idx <= rd_pixel_idx + PIX_IDX_WIDTH'(1);
                            rd_state     <= RD_FETCH_1;
                        end
                    end
                end

                default: begin
                    rd_state <= RD_IDLE;
                end
            endcase
        end
    end

endmodule
`default_nettype wire

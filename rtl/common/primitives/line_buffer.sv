`default_nettype none
// Generic double-buffered line buffer for crossing pixel lines between clock domains.
// Pixels are written into one half of a dual-port RAM while the other half is read out.
// The MSB of the write/read line pointers selects the active buffer and is synchronized
// into the opposite clock domain so each side can determine when a line is available or
// when a buffer has been released.

module line_buffer #(
    parameter int PIXEL_WIDTH     = 24,
    parameter int MAX_LINE_PIXELS = 1024,
    parameter int SYNC_STAGES     = 3
) (
    input  wire logic                   wr_clk,
    input  wire logic                   rd_clk,
    input  wire logic                   rst,
    input  wire logic                   start_of_frame,

    // Write-side pixel input (wr_clk domain)
    input  wire logic                   wr_valid,
    output logic                        wr_ready,
    input  wire logic [PIXEL_WIDTH-1:0] wr_pixel,
    input  wire logic                   wr_eol,

    // Read-side pixel output (rd_clk domain)
    output logic                        rd_valid,
    input  wire logic                   rd_ready,
    output logic [PIXEL_WIDTH-1:0]      rd_pixel,
    output logic                        rd_eol
);

    localparam int LINE_ADDR_WIDTH = (MAX_LINE_PIXELS <= 1) ? 1 : $clog2(MAX_LINE_PIXELS);
    localparam int RAM_ADDR_WIDTH  = LINE_ADDR_WIDTH + 1;
    localparam int RAM_DATA_WIDTH  = PIXEL_WIDTH + 1;

    // wr_clk domain signals
    logic                    wr_line_ptr_msb;
    logic [LINE_ADDR_WIDTH-1:0] wr_addr;
    logic                    wr_blocked;
    logic                    rd_line_ptr_msb_sync_wr;
    logic                    wr_fire;
    logic                    wr_line_ptr_msb_base;
    logic [LINE_ADDR_WIDTH-1:0] wr_addr_base;
    logic                    wr_blocked_base;
    logic                    wr_addr_has_space;
    logic                    wr_sof_toggle;
    logic                    wr_wait_for_rd_reset;

    // rd_clk domain signals
    logic                    rd_line_ptr_msb;
    logic [LINE_ADDR_WIDTH-1:0] rd_addr;
    logic                    wr_line_ptr_msb_sync_rd;
    logic                    rd_wait_for_wr_reset;
    logic                    rd_line_active;
    logic                    rd_out_valid;
    logic                    rd_out_eol;
    logic [PIXEL_WIDTH-1:0]  rd_out_pixel;
    logic                    rd_load_pending_stage1;
    logic                    rd_load_pending_stage2;
    logic                    rd_line_available;
    logic                    rd_start_load;
    logic                    rd_fire;
    logic                    rd_sof_toggle_sync;
    logic                    rd_sof_toggle_sync_d;
    logic                    rd_frame_reset;

    logic [RAM_DATA_WIDTH-1:0] ram_rdata;

    initial begin
        if (PIXEL_WIDTH <= 0) begin
            $fatal(1, "line_buffer: PIXEL_WIDTH must be > 0, got %0d", PIXEL_WIDTH);
        end
        if (MAX_LINE_PIXELS <= 0) begin
            $fatal(1, "line_buffer: MAX_LINE_PIXELS must be > 0, got %0d", MAX_LINE_PIXELS);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "line_buffer: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    assign wr_line_ptr_msb_base = start_of_frame ? 1'b0 : wr_line_ptr_msb;
    assign wr_addr_base         = start_of_frame ? '0 : wr_addr;
    assign wr_blocked_base      = start_of_frame ? 1'b0 : wr_blocked;

    generate
        if (MAX_LINE_PIXELS == 1) begin : gen_single_pixel_lines
            assign wr_addr_has_space = wr_eol;
        end else begin : gen_multi_pixel_lines
            assign wr_addr_has_space = (wr_addr_base < LINE_ADDR_WIDTH'(MAX_LINE_PIXELS - 1)) || wr_eol;
        end
    endgenerate

    assign wr_ready = !wr_blocked_base && wr_addr_has_space;
    assign wr_fire  = wr_valid && wr_ready;

    assign rd_valid = rd_out_valid;
    assign rd_pixel = rd_out_pixel;
    assign rd_eol   = rd_out_eol;
    assign rd_fire  = rd_valid && rd_ready;

    assign rd_frame_reset = rd_sof_toggle_sync ^ rd_sof_toggle_sync_d;
    assign rd_line_available = !rd_wait_for_wr_reset && (wr_line_ptr_msb_sync_rd != rd_line_ptr_msb);
    assign rd_start_load = !rd_load_pending_stage1
        && !rd_load_pending_stage2
        && !rd_out_valid
        && (rd_line_active || rd_line_available);

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_rd_line_ptr_sync (
        .clk(wr_clk),
        .rst(rst),
        .din(rd_line_ptr_msb),
        .dout(rd_line_ptr_msb_sync_wr)
    );

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_wr_line_ptr_sync (
        .clk(rd_clk),
        .rst(rst),
        .din(wr_line_ptr_msb),
        .dout(wr_line_ptr_msb_sync_rd)
    );

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .WIDTH(1)
    ) u_sof_sync (
        .clk(rd_clk),
        .rst(rst),
        .din(wr_sof_toggle),
        .dout(rd_sof_toggle_sync)
    );

    sync_dpram #(
        .DATA_WIDTH(RAM_DATA_WIDTH),
        .ADDR_WIDTH(RAM_ADDR_WIDTH)
    ) u_line_ram (
        .wclk(wr_clk),
        .rclk(rd_clk),
        .we(wr_fire),
        .waddr({wr_line_ptr_msb_base, wr_addr_base}),
        .wdata({wr_eol, wr_pixel}),
        .raddr({rd_line_ptr_msb, rd_addr}),
        .rdata(ram_rdata)
    );

    always_ff @(posedge wr_clk) begin
        if (rst) begin
            wr_line_ptr_msb <= 1'b0;
            wr_addr         <= '0;
            wr_blocked      <= 1'b0;
            wr_sof_toggle   <= 1'b0;
            wr_wait_for_rd_reset <= 1'b1;
        end else begin
            if (start_of_frame) begin
                wr_line_ptr_msb <= 1'b0;
                wr_addr         <= '0;
                wr_blocked      <= 1'b0;
                wr_sof_toggle   <= ~wr_sof_toggle;
                wr_wait_for_rd_reset <= 1'b1;
            end else begin
                if (wr_wait_for_rd_reset && (rd_line_ptr_msb_sync_wr == 1'b0)) begin
                    wr_wait_for_rd_reset <= 1'b0;
                end
            end

            if (wr_blocked && !wr_wait_for_rd_reset && (rd_line_ptr_msb_sync_wr != wr_line_ptr_msb)) begin
                wr_blocked <= 1'b0;
            end
            if (wr_fire) begin
                if (wr_eol) begin
                    wr_line_ptr_msb <= ~wr_line_ptr_msb_base;
                    wr_addr         <= '0;
                    wr_blocked      <= ((~wr_line_ptr_msb_base) == (wr_wait_for_rd_reset ? 1'b0 : rd_line_ptr_msb_sync_wr));
                end else begin
                    wr_addr <= wr_addr_base + LINE_ADDR_WIDTH'(1);
                end
            end
        end
    end

    always_ff @(posedge rd_clk) begin
        if (rst || rd_frame_reset) begin
            rd_sof_toggle_sync_d <= 1'b0;
            rd_line_ptr_msb      <= 1'b0;
            rd_addr              <= '0;
            rd_wait_for_wr_reset <= 1'b1;
            rd_line_active       <= 1'b0;
            rd_out_valid         <= 1'b0;
            rd_out_eol           <= 1'b0;
            rd_load_pending_stage1 <= 1'b0;
            rd_load_pending_stage2 <= 1'b0;
        end else begin
            rd_sof_toggle_sync_d <= rd_sof_toggle_sync;

            if (rd_wait_for_wr_reset && (wr_line_ptr_msb_sync_rd == 1'b0)) begin
                rd_wait_for_wr_reset <= 1'b0;
            end

            if (rd_load_pending_stage2) begin
                rd_out_pixel <= ram_rdata[PIXEL_WIDTH-1:0];
                rd_out_eol   <= ram_rdata[RAM_DATA_WIDTH-1];
                rd_out_valid <= 1'b1;
                rd_addr      <= rd_addr + LINE_ADDR_WIDTH'(1);
            end

            rd_load_pending_stage2 <= rd_load_pending_stage1;
            rd_load_pending_stage1 <= rd_start_load;

            if (!rd_line_active && rd_line_available && !rd_out_valid) begin
                rd_line_active <= 1'b1;
            end

            if (rd_fire) begin
                rd_out_valid <= 1'b0;
                if (rd_out_eol) begin
                    rd_line_active <= 1'b0;
                    rd_line_ptr_msb <= ~rd_line_ptr_msb;
                    rd_addr <= '0;
                end
            end
        end
    end

endmodule
`default_nettype wire

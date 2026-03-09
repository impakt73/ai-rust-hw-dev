// Video Sync Generator
// Generates registered video timing outputs for raster-style interfaces such as
// VGA or DVI.
//
// Parameters:
//   H_ACTIVE           - Horizontal active pixel count
//   H_FRONT_PORCH      - Horizontal front porch pixel count
//   H_SYNC_WIDTH       - Horizontal sync pulse width in pixels
//   H_BACK_PORCH       - Horizontal back porch pixel count
//   V_ACTIVE           - Vertical active line count
//   V_FRONT_PORCH      - Vertical front porch line count
//   V_SYNC_WIDTH       - Vertical sync pulse width in lines
//   V_BACK_PORCH       - Vertical back porch line count
//   HSYNC_ACTIVE_HIGH  - 1 for active-high hsync, 0 for active-low hsync
//   VSYNC_ACTIVE_HIGH  - 1 for active-high vsync, 0 for active-low vsync
//
// Interface:
//   clk          - Pixel clock
//   rst_n        - Synchronous active-low reset
//   hsync        - Registered horizontal sync output
//   vsync        - Registered vertical sync output
//   active_video - Registered active-region qualifier
//   line_start   - Registered one-cycle pulse at the first pixel of each line
//   frame_start  - Registered one-cycle pulse at the first pixel of each frame
//   active_x     - Registered X coordinate within the active region, else 0
//   active_y     - Registered Y coordinate within the active region, else 0

module video_sync #(
    parameter int unsigned H_ACTIVE = 640,
    parameter int unsigned H_FRONT_PORCH = 16,
    parameter int unsigned H_SYNC_WIDTH = 96,
    parameter int unsigned H_BACK_PORCH = 48,
    parameter int unsigned V_ACTIVE = 480,
    parameter int unsigned V_FRONT_PORCH = 10,
    parameter int unsigned V_SYNC_WIDTH = 2,
    parameter int unsigned V_BACK_PORCH = 33,
    parameter bit HSYNC_ACTIVE_HIGH = 1'b0,
    parameter bit VSYNC_ACTIVE_HIGH = 1'b0
) (
    input  logic clk,
    input  logic rst_n,
    output logic hsync,
    output logic vsync,
    output logic active_video,
    output logic line_start,
    output logic frame_start,
    output logic [((H_ACTIVE <= 1) ? 1 : $clog2(H_ACTIVE)) - 1:0] active_x,
    output logic [((V_ACTIVE <= 1) ? 1 : $clog2(V_ACTIVE)) - 1:0] active_y
);

    localparam int unsigned H_TOTAL = H_ACTIVE + H_FRONT_PORCH + H_SYNC_WIDTH + H_BACK_PORCH;
    localparam int unsigned V_TOTAL = V_ACTIVE + V_FRONT_PORCH + V_SYNC_WIDTH + V_BACK_PORCH;
    localparam int unsigned H_COUNTER_WIDTH = (H_TOTAL <= 1) ? 1 : $clog2(H_TOTAL);
    localparam int unsigned V_COUNTER_WIDTH = (V_TOTAL <= 1) ? 1 : $clog2(V_TOTAL);
    localparam int unsigned ACTIVE_X_WIDTH = (H_ACTIVE <= 1) ? 1 : $clog2(H_ACTIVE);
    localparam int unsigned ACTIVE_Y_WIDTH = (V_ACTIVE <= 1) ? 1 : $clog2(V_ACTIVE);
    localparam logic [H_COUNTER_WIDTH-1:0] H_LAST = H_COUNTER_WIDTH'(H_TOTAL - 1);
    localparam logic [V_COUNTER_WIDTH-1:0] V_LAST = V_COUNTER_WIDTH'(V_TOTAL - 1);
    localparam logic [H_COUNTER_WIDTH-1:0] H_ACTIVE_END = H_COUNTER_WIDTH'(H_ACTIVE);
    localparam logic [V_COUNTER_WIDTH-1:0] V_ACTIVE_END = V_COUNTER_WIDTH'(V_ACTIVE);
    localparam logic [H_COUNTER_WIDTH-1:0] H_SYNC_START =
        H_COUNTER_WIDTH'(H_ACTIVE + H_FRONT_PORCH);
    localparam logic [H_COUNTER_WIDTH-1:0] H_SYNC_SPAN = H_COUNTER_WIDTH'(H_SYNC_WIDTH);
    localparam logic [V_COUNTER_WIDTH-1:0] V_SYNC_START =
        V_COUNTER_WIDTH'(V_ACTIVE + V_FRONT_PORCH);
    localparam logic [V_COUNTER_WIDTH-1:0] V_SYNC_SPAN = V_COUNTER_WIDTH'(V_SYNC_WIDTH);

    logic [H_COUNTER_WIDTH-1:0] h_counter;
    logic [V_COUNTER_WIDTH-1:0] v_counter;
    logic [H_COUNTER_WIDTH-1:0] h_counter_next;
    logic [V_COUNTER_WIDTH-1:0] v_counter_next;
    logic hsync_next;
    logic vsync_next;
    logic active_video_next;
    logic line_start_next;
    logic frame_start_next;
    logic [ACTIVE_X_WIDTH-1:0] active_x_next;
    logic [ACTIVE_Y_WIDTH-1:0] active_y_next;
    logic [H_COUNTER_WIDTH-1:0] h_sync_offset;
    logic [V_COUNTER_WIDTH-1:0] v_sync_offset;
    logic h_in_active_region;
    logic v_in_active_region;
    logic h_in_sync_region;
    logic v_in_sync_region;

    // Parameter validation (simulation only)
    initial begin
        if (H_ACTIVE == 0) begin
            $fatal(1, "video_sync: H_ACTIVE must be > 0");
        end
        if (V_ACTIVE == 0) begin
            $fatal(1, "video_sync: V_ACTIVE must be > 0");
        end
        if (H_SYNC_WIDTH == 0) begin
            $fatal(1, "video_sync: H_SYNC_WIDTH must be > 0");
        end
        if (V_SYNC_WIDTH == 0) begin
            $fatal(1, "video_sync: V_SYNC_WIDTH must be > 0");
        end
        if (H_TOTAL <= H_ACTIVE) begin
            $fatal(1, "video_sync: horizontal blanking interval must be > 0");
        end
        if (V_TOTAL <= V_ACTIVE) begin
            $fatal(1, "video_sync: vertical blanking interval must be > 0");
        end
    end

    always_comb begin
        if (h_counter == H_LAST) begin
            h_counter_next = '0;
            if (v_counter == V_LAST) begin
                v_counter_next = '0;
            end else begin
                v_counter_next = v_counter + 1'b1;
            end
        end else begin
            h_counter_next = h_counter + 1'b1;
            v_counter_next = v_counter;
        end

        h_in_active_region = (h_counter_next < H_ACTIVE_END);
        v_in_active_region = (v_counter_next < V_ACTIVE_END);
        h_sync_offset = h_counter_next - H_SYNC_START;
        v_sync_offset = v_counter_next - V_SYNC_START;
        h_in_sync_region =
            (h_counter_next >= H_SYNC_START) &&
            (h_sync_offset < H_SYNC_SPAN);
        v_in_sync_region =
            (v_counter_next >= V_SYNC_START) &&
            (v_sync_offset < V_SYNC_SPAN);

        active_video_next = h_in_active_region && v_in_active_region;
        line_start_next = (h_counter_next == '0);
        frame_start_next = (h_counter_next == '0) && (v_counter_next == '0);
        hsync_next = h_in_sync_region ? HSYNC_ACTIVE_HIGH : ~HSYNC_ACTIVE_HIGH;
        vsync_next = v_in_sync_region ? VSYNC_ACTIVE_HIGH : ~VSYNC_ACTIVE_HIGH;
        active_x_next = active_video_next ? ACTIVE_X_WIDTH'(h_counter_next) : '0;
        active_y_next = active_video_next ? ACTIVE_Y_WIDTH'(v_counter_next) : '0;
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            h_counter <= H_LAST;
            v_counter <= V_LAST;
            hsync <= ~HSYNC_ACTIVE_HIGH;
            vsync <= ~VSYNC_ACTIVE_HIGH;
            active_video <= 1'b0;
            line_start <= 1'b0;
            frame_start <= 1'b0;
            active_x <= '0;
            active_y <= '0;
        end else begin
            h_counter <= h_counter_next;
            v_counter <= v_counter_next;
            hsync <= hsync_next;
            vsync <= vsync_next;
            active_video <= active_video_next;
            line_start <= line_start_next;
            frame_start <= frame_start_next;
            active_x <= active_x_next;
            active_y <= active_y_next;
        end
    end

endmodule

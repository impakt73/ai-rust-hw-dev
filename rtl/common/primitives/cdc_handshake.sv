`default_nettype none
// Clock-Domain-Crossing Handshake
// Transfers a single multi-bit payload across clock domains using a request/ack
// toggle handshake. The source payload is captured into a holding register on the
// source-side handshake, and the destination presents the transferred word on
// registered outputs.
//
// Parameters:
//   WIDTH       - Payload width in bits (default: 8)
//   SYNC_STAGES - Number of FF synchronizer stages for CDC toggles (default: 3)
//
// Interface:
//   src_clk   - Source clock domain
//   dst_clk   - Destination clock domain
//   src_rst   - Synchronous active-high reset in the source clock domain
//   dst_rst   - Synchronous active-high reset in the destination clock domain
//   src_valid - Source presents a new payload this cycle
//   src_ready - Module can accept a new payload this cycle
//   src_data  - Source payload sampled when src_valid && src_ready
//   dst_valid - Destination payload is valid this cycle
//   dst_ready - Destination consumes the payload this cycle
//   dst_data  - Registered destination payload

module cdc_handshake #(
    parameter int WIDTH = 8,
    parameter int SYNC_STAGES = 3
) (
    // Source interface (src_clk domain)
    input wire logic             src_clk,
    input wire logic             dst_clk,
    input wire logic             src_rst,
    input wire logic             dst_rst,
    input wire logic             src_valid,
    output logic                 src_ready,
    input wire logic [WIDTH-1:0] src_data,

    // Destination interface (dst_clk domain)
    output logic                 dst_valid,
    input wire logic             dst_ready,
    output logic [WIDTH-1:0]     dst_data
);

    logic                 src_req_toggle;
    logic                 src_pending;
    logic [WIDTH-1:0]     src_data_hold;
    logic                 src_ack_toggle_sync;
    logic                 dst_ack_toggle;
    logic                 dst_req_toggle_sync;
    logic                 dst_req_toggle_prev;
    logic                 src_fire;
    logic                 dst_fire;
    logic                 dst_req_seen;

    // Parameter validation (simulation only)
    initial begin
        if (WIDTH < 1) begin
            $fatal(1, "cdc_handshake: WIDTH must be >= 1, got %0d", WIDTH);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "cdc_handshake: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    assign src_fire = src_valid && src_ready;
    assign dst_fire = dst_valid && dst_ready;
    assign dst_req_seen = (dst_req_toggle_sync != dst_req_toggle_prev);

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .RESET_VALUE(1'b0)
    ) u_ack_sync (
        .clk(src_clk),
        .rst(src_rst),
        .din(dst_ack_toggle),
        .dout(src_ack_toggle_sync)
    );

    ff_sync #(
        .STAGES(SYNC_STAGES),
        .RESET_VALUE(1'b0)
    ) u_req_sync (
        .clk(dst_clk),
        .rst(dst_rst),
        .din(src_req_toggle),
        .dout(dst_req_toggle_sync)
    );

    always_ff @(posedge src_clk) begin
        if (src_rst) begin
            src_req_toggle <= 1'b0;
            src_pending    <= 1'b0;
            src_ready      <= 1'b1;
        end else begin
            if (src_fire) begin
                // Hold the payload stable until the destination consumes it and the
                // synchronized acknowledge returns. This register intentionally is not
                // reset because src_pending guarantees it is ignored while invalid.
                src_data_hold  <= src_data;
                src_req_toggle <= ~src_req_toggle;
                src_pending    <= 1'b1;
                src_ready      <= 1'b0;
            end else if (src_pending && (src_ack_toggle_sync == src_req_toggle)) begin
                src_pending <= 1'b0;
                src_ready   <= 1'b1;
            end
        end
    end

    always_ff @(posedge dst_clk) begin
        if (dst_rst) begin
            dst_valid           <= 1'b0;
            dst_ack_toggle      <= 1'b0;
            dst_req_toggle_prev <= 1'b0;
        end else begin
            dst_req_toggle_prev <= dst_req_toggle_sync;

            if (dst_req_seen) begin
                // The request toggle is synchronized into dst_clk before capture, so
                // src_data_hold has already been stable for multiple dst_clk cycles
                // when the destination samples it here. dst_data intentionally is not
                // reset because dst_valid marks when it contains meaningful data.
                dst_data  <= src_data_hold;
                dst_valid <= 1'b1;
            end else if (dst_fire) begin
                dst_valid <= 1'b0;
            end

            if (dst_fire) begin
                dst_ack_toggle <= ~dst_ack_toggle;
            end
        end
    end

endmodule
`default_nettype wire

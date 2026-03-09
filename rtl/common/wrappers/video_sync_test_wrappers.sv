module video_sync_wrapper (
    input  logic clk,
    input  logic rst_n,
    output logic hsync,
    output logic vsync,
    output logic active_video,
    output logic line_start,
    output logic frame_start,
    output logic [1:0] active_x,
    output logic [1:0] active_y
);

    video_sync #(
        .H_ACTIVE(4),
        .H_FRONT_PORCH(1),
        .H_SYNC_WIDTH(2),
        .H_BACK_PORCH(1),
        .V_ACTIVE(3),
        .V_FRONT_PORCH(1),
        .V_SYNC_WIDTH(1),
        .V_BACK_PORCH(1),
        .HSYNC_ACTIVE_HIGH(1'b0),
        .VSYNC_ACTIVE_HIGH(1'b0)
    ) u_video_sync (
        .clk(clk),
        .rst_n(rst_n),
        .hsync(hsync),
        .vsync(vsync),
        .active_video(active_video),
        .line_start(line_start),
        .frame_start(frame_start),
        .active_x(active_x),
        .active_y(active_y)
    );

endmodule

module video_sync_minimal_wrapper (
    input  logic clk,
    input  logic rst_n,
    output logic hsync,
    output logic vsync,
    output logic active_video,
    output logic line_start,
    output logic frame_start,
    output logic active_x,
    output logic active_y
);

    video_sync #(
        .H_ACTIVE(1),
        .H_FRONT_PORCH(0),
        .H_SYNC_WIDTH(1),
        .H_BACK_PORCH(0),
        .V_ACTIVE(1),
        .V_FRONT_PORCH(0),
        .V_SYNC_WIDTH(1),
        .V_BACK_PORCH(0),
        .HSYNC_ACTIVE_HIGH(1'b1),
        .VSYNC_ACTIVE_HIGH(1'b1)
    ) u_video_sync (
        .clk(clk),
        .rst_n(rst_n),
        .hsync(hsync),
        .vsync(vsync),
        .active_video(active_video),
        .line_start(line_start),
        .frame_start(frame_start),
        .active_x(active_x),
        .active_y(active_y)
    );

endmodule

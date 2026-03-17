module core_bridge_cmd (
    input  wire        clk,
    output wire        reset_n,
    input  wire        bridge_endian_little,
    input  wire [31:0] bridge_addr,
    input  wire        bridge_rd,
    output wire [31:0] bridge_rd_data,
    input  wire        bridge_wr,
    input  wire [31:0] bridge_wr_data,
    input  wire        status_boot_done,
    input  wire        status_setup_done,
    input  wire        status_running,
    output wire        dataslot_requestread,
    output wire [15:0] dataslot_requestread_id,
    input  wire        dataslot_requestread_ack,
    input  wire        dataslot_requestread_ok,
    output wire        dataslot_requestwrite,
    output wire [15:0] dataslot_requestwrite_id,
    output wire [31:0] dataslot_requestwrite_size,
    input  wire        dataslot_requestwrite_ack,
    input  wire        dataslot_requestwrite_ok,
    output wire        dataslot_update,
    output wire [15:0] dataslot_update_id,
    output wire [31:0] dataslot_update_size,
    output wire        dataslot_allcomplete,
    output wire [31:0] rtc_epoch_seconds,
    output wire [31:0] rtc_date_bcd,
    output wire [31:0] rtc_time_bcd,
    output wire        rtc_valid,
    input  wire        savestate_supported,
    input  wire [31:0] savestate_addr,
    input  wire [31:0] savestate_size,
    input  wire [31:0] savestate_maxloadsize,
    output wire        osnotify_inmenu,
    output wire        savestate_start,
    input  wire        savestate_start_ack,
    input  wire        savestate_start_busy,
    input  wire        savestate_start_ok,
    input  wire        savestate_start_err,
    output wire        savestate_load,
    input  wire        savestate_load_ack,
    input  wire        savestate_load_busy,
    input  wire        savestate_load_ok,
    input  wire        savestate_load_err,
    input  wire        target_dataslot_read,
    input  wire        target_dataslot_write,
    input  wire        target_dataslot_getfile,
    input  wire        target_dataslot_openfile,
    output wire        target_dataslot_ack,
    output wire        target_dataslot_done,
    output wire [2:0]  target_dataslot_err,
    input  wire [15:0] target_dataslot_id,
    input  wire [31:0] target_dataslot_slotoffset,
    input  wire [31:0] target_dataslot_bridgeaddr,
    input  wire [31:0] target_dataslot_length,
    input  wire [31:0] target_buffer_param_struct,
    input  wire [31:0] target_buffer_resp_struct,
    input  wire [9:0]  datatable_addr,
    input  wire        datatable_wren,
    input  wire [31:0] datatable_data,
    output wire [31:0] datatable_q
);
    assign reset_n = 1'b1;
    assign bridge_rd_data = 32'h0;
    assign dataslot_requestread = 1'b0;
    assign dataslot_requestread_id = 16'h0;
    assign dataslot_requestwrite = 1'b0;
    assign dataslot_requestwrite_id = 16'h0;
    assign dataslot_requestwrite_size = 32'h0;
    assign dataslot_update = 1'b0;
    assign dataslot_update_id = 16'h0;
    assign dataslot_update_size = 32'h0;
    assign dataslot_allcomplete = 1'b0;
    assign rtc_epoch_seconds = 32'h0;
    assign rtc_date_bcd = 32'h0;
    assign rtc_time_bcd = 32'h0;
    assign rtc_valid = 1'b0;
    assign osnotify_inmenu = 1'b0;
    assign savestate_start = 1'b0;
    assign savestate_load = 1'b0;
    assign target_dataslot_ack = 1'b0;
    assign target_dataslot_done = 1'b0;
    assign target_dataslot_err = 3'b000;
    assign datatable_q = 32'h0;
endmodule

module io_pad_controller (
    input  wire        clk,
    input  wire        reset_n,
    inout  wire        pad_1wire,
    output wire [31:0] cont1_key,
    output wire [31:0] cont2_key,
    output wire [31:0] cont3_key,
    output wire [31:0] cont4_key,
    output wire [31:0] cont1_joy,
    output wire [31:0] cont2_joy,
    output wire [31:0] cont3_joy,
    output wire [31:0] cont4_joy,
    output wire [15:0] cont1_trig,
    output wire [15:0] cont2_trig,
    output wire [15:0] cont3_trig,
    output wire [15:0] cont4_trig,
    output wire        rx_timed_out
);
    assign pad_1wire = 1'bz;
    assign cont1_key = 32'h0;
    assign cont2_key = 32'h0;
    assign cont3_key = 32'h0;
    assign cont4_key = 32'h0;
    assign cont1_joy = 32'h0;
    assign cont2_joy = 32'h0;
    assign cont3_joy = 32'h0;
    assign cont4_joy = 32'h0;
    assign cont1_trig = 16'h0;
    assign cont2_trig = 16'h0;
    assign cont3_trig = 16'h0;
    assign cont4_trig = 16'h0;
    assign rx_timed_out = 1'b0;
endmodule

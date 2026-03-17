module synch_2 #(parameter WIDTH = 1) (
    input  wire [WIDTH-1:0] i,
    output reg  [WIDTH-1:0] o,
    input  wire             rst,
    input  wire             clk,
    output wire             rise,
    output wire             fall
);
    reg [WIDTH-1:0] stage_1;
    reg [WIDTH-1:0] stage_2;

    assign rise = (WIDTH == 1) ? (o & ~stage_2) : 1'b0;
    assign fall = (WIDTH == 1) ? (~o & stage_2) : 1'b0;

    always @(posedge clk) begin
        if (rst) begin
            stage_2 <= '0;
            o <= '0;
            stage_1 <= '0;
        end else begin
            stage_2 <= o;
            o <= stage_1;
            stage_1 <= i;
        end
    end
endmodule

module synch_3 #(parameter WIDTH = 1) (
    input  wire [WIDTH-1:0] i,
    output reg  [WIDTH-1:0] o,
    input  wire             rst,
    input  wire             clk,
    output wire             rise,
    output wire             fall
);
    reg [WIDTH-1:0] stage_1;
    reg [WIDTH-1:0] stage_2;
    reg [WIDTH-1:0] stage_3;

    assign rise = (WIDTH == 1) ? (o & ~stage_3) : 1'b0;
    assign fall = (WIDTH == 1) ? (~o & stage_3) : 1'b0;

    always @(posedge clk) begin
        if (rst) begin
            stage_3 <= '0;
            o <= '0;
            stage_2 <= '0;
            stage_1 <= '0;
        end else begin
            stage_3 <= o;
            o <= stage_2;
            stage_2 <= stage_1;
            stage_1 <= i;
        end
    end
endmodule

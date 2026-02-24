// Single-entry skid buffer with optional bypass
// - Default (BYPASS_ENABLE=0): fully registered output path
// - Optional (BYPASS_ENABLE=1): combinational pass-through when buffer is empty
module skid_buffer #(
    parameter int WIDTH = 8,
    parameter int BYPASS_ENABLE = 0
) (
    input  logic             clk,
    input  logic             rst_n,
    input  logic             in_valid,
    input  logic [WIDTH-1:0] in_data,
    output logic             in_ready,
    output logic             out_valid,
    output logic [WIDTH-1:0] out_data,
    input  logic             out_ready
);

    logic             valid_q;
    logic [WIDTH-1:0] data_q;
    localparam logic BYPASS_EN = (BYPASS_ENABLE != 0);

    // Parameter validation (simulation only)
    initial begin
        if (WIDTH < 1) begin
            $fatal(1, "skid_buffer: WIDTH must be >= 1, got %0d", WIDTH);
        end
        if (BYPASS_ENABLE != 0 && BYPASS_ENABLE != 1) begin
            $fatal(1, "skid_buffer: BYPASS_ENABLE must be 0 or 1, got %0d", BYPASS_ENABLE);
        end
    end

    always_comb begin
        if (BYPASS_EN && !valid_q) begin
            in_ready  = 1'b1;
            out_valid = in_valid;
            out_data  = in_data;
        end else begin
            in_ready  = !valid_q || out_ready;
            out_valid = valid_q;
            out_data  = data_q;
        end
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            valid_q <= 1'b0;
            data_q  <= '0;
        end else if (BYPASS_EN) begin
            if (!valid_q) begin
                if (in_valid && !out_ready) begin
                    valid_q <= 1'b1;
                    data_q  <= in_data;
                end else begin
                    valid_q <= 1'b0;
                end
            end else if (out_ready) begin
                if (in_valid) begin
                    valid_q <= 1'b1;
                    data_q  <= in_data;
                end else begin
                    valid_q <= 1'b0;
                end
            end
        end else if (!valid_q || out_ready) begin
            valid_q <= in_valid;
            if (in_valid) begin
                data_q <= in_data;
            end
        end
    end

endmodule

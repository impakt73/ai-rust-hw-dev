// Full skid buffer (2-entry) with registered output-side and input-side handshake signals.
// Breaks combinational paths by registering out_valid/out_data and in_ready.
module skid_buffer #(
    parameter int WIDTH = 8
) (
    input  logic             clk,
    input  logic             rst_n,

    // Input side
    input  logic             in_valid,
    input  logic [WIDTH-1:0] in_data,
    output logic             in_ready,

    // Output side
    output logic             out_valid,
    output logic [WIDTH-1:0] out_data,
    input  logic             out_ready
);

    logic [WIDTH-1:0] out_data_current;
    logic [WIDTH-1:0] out_data_next;
    logic             out_valid_current;
    logic             out_valid_next;

    logic [WIDTH-1:0] skid_data_current;
    logic [WIDTH-1:0] skid_data_next;
    logic             skid_valid_current;
    logic             skid_valid_next;

    logic             in_ready_current;
    logic             in_ready_next;

    logic             out_pop_current;
    logic             in_push_current;

    always_comb begin
        out_data_next   = out_data_current;
        out_valid_next  = out_valid_current;
        skid_data_next  = skid_data_current;
        skid_valid_next = skid_valid_current;

        out_pop_current = out_valid_current && out_ready;
        in_push_current = in_valid && in_ready_current;

        // Pop from output slot first.
        if (out_pop_current) begin
            if (skid_valid_current) begin
                out_data_next   = skid_data_current;
                out_valid_next  = 1'b1;
                skid_valid_next = 1'b0;
            end else begin
                out_valid_next = 1'b0;
            end
        end

        // Push new input data.
        if (in_push_current) begin
            if (!out_valid_next) begin
                out_data_next  = in_data;
                out_valid_next = 1'b1;
            end else begin
                skid_data_next  = in_data;
                skid_valid_next = 1'b1;
            end
        end

        in_ready_next = !(out_valid_next && skid_valid_next);
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            out_data_current  <= '0;
            out_valid_current <= 1'b0;
            skid_data_current <= '0;
            skid_valid_current <= 1'b0;
            in_ready_current  <= 1'b1;
        end else begin
            out_data_current  <= out_data_next;
            out_valid_current <= out_valid_next;
            skid_data_current <= skid_data_next;
            skid_valid_current <= skid_valid_next;
            in_ready_current  <= in_ready_next;
        end
    end

    assign out_data  = out_data_current;
    assign out_valid = out_valid_current;
    assign in_ready  = in_ready_current;

endmodule

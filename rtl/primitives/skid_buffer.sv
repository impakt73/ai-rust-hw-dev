// Two-entry skid buffer with registered output data/valid signals
//
// Parameters:
//   WIDTH  - Data width in bits (default: 8)
//   BYPASS - When 1, allows refill of output register in the same cycle it is consumed
//
// Interface:
//   s_valid/s_data/s_ready - Source (input) ready/valid channel
//   m_valid/m_data/m_ready - Sink (output) ready/valid channel

module skid_buffer #(
    parameter int WIDTH = 8,
    parameter int BYPASS = 0
) (
    input  logic             clk,
    input  logic             rst_n,

    input  logic             s_valid,
    input  logic [WIDTH-1:0] s_data,
    output logic             s_ready,

    output logic             m_valid,
    output logic [WIDTH-1:0] m_data,
    input  logic             m_ready
);

    logic             out_valid_q, out_valid_d;
    logic [WIDTH-1:0] out_data_q, out_data_d;
    logic             skid_valid_q, skid_valid_d;
    logic [WIDTH-1:0] skid_data_q, skid_data_d;
    logic             pop_now;
    logic             push_now;
    logic             bypass_taken;

    initial begin
        if ((BYPASS != 0) && (BYPASS != 1)) begin
            $fatal(1, "skid_buffer: BYPASS must be 0 or 1, got %0d", BYPASS);
        end
    end

    assign pop_now  = out_valid_q && m_ready;
    assign s_ready  = !skid_valid_q || pop_now;
    assign push_now = s_valid && s_ready;

    always_comb begin
        out_valid_d   = out_valid_q;
        out_data_d    = out_data_q;
        skid_valid_d  = skid_valid_q;
        skid_data_d   = skid_data_q;
        bypass_taken  = 1'b0;

        if (pop_now) begin
            if (skid_valid_q) begin
                out_valid_d  = 1'b1;
                out_data_d   = skid_data_q;
                skid_valid_d = 1'b0;
            end else if ((BYPASS != 0) && s_valid) begin
                out_valid_d  = 1'b1;
                out_data_d   = s_data;
                bypass_taken = 1'b1;
            end else begin
                out_valid_d = 1'b0;
            end
        end

        if (!out_valid_d && skid_valid_d) begin
            out_valid_d  = 1'b1;
            out_data_d   = skid_data_d;
            skid_valid_d = 1'b0;
        end

        if (push_now && !bypass_taken) begin
            if (!out_valid_d) begin
                if ((BYPASS == 0) && pop_now && !skid_valid_q) begin
                    skid_valid_d = 1'b1;
                    skid_data_d  = s_data;
                end else begin
                    out_valid_d = 1'b1;
                    out_data_d  = s_data;
                end
            end else if (!skid_valid_d) begin
                skid_valid_d = 1'b1;
                skid_data_d  = s_data;
            end
        end
    end

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            out_valid_q  <= 1'b0;
            out_data_q   <= '0;
            skid_valid_q <= 1'b0;
            skid_data_q  <= '0;
        end else begin
            out_valid_q  <= out_valid_d;
            out_data_q   <= out_data_d;
            skid_valid_q <= skid_valid_d;
            skid_data_q  <= skid_data_d;
        end
    end

    assign m_valid = out_valid_q;
    assign m_data  = out_data_q;

endmodule

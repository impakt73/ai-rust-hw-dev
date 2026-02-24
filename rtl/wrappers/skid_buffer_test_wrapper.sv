module skid_buffer_test_wrapper (
    input  logic       clk,
    input  logic       rst_n,
    input  logic       s_valid,
    input  logic [7:0] s_data,
    output logic       s_ready,
    output logic       m_valid,
    output logic [7:0] m_data,
    input  logic       m_ready
);

    skid_buffer #(
        .WIDTH(8),
        .BYPASS(0)
    ) u_skid_buffer (
        .clk(clk),
        .rst_n(rst_n),
        .s_valid(s_valid),
        .s_data(s_data),
        .s_ready(s_ready),
        .m_valid(m_valid),
        .m_data(m_data),
        .m_ready(m_ready)
    );

endmodule

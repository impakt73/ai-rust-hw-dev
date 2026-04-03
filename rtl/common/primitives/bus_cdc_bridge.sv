`default_nettype none
module bus_cdc_bridge #(
    parameter int ADDR_WIDTH = 32,
    parameter int DATA_WIDTH = 32,
    parameter int SIZE_WIDTH = 2,
    parameter int SYNC_STAGES = 3
) (
    input wire logic                  sys_clk,
    input wire logic                  periph_clk,
    input wire logic                  rst,

    input wire logic [ADDR_WIDTH-1:0] sys_mem_a_addr,
    input wire logic [DATA_WIDTH-1:0] sys_mem_a_wdata,
    input wire logic                  sys_mem_a_we,
    input wire logic [SIZE_WIDTH-1:0] sys_mem_a_size,
    input wire logic                  sys_mem_a_valid,
    output logic                      sys_mem_a_ready,

    output logic [DATA_WIDTH-1:0]     sys_mem_d_rdata,
    output logic                      sys_mem_d_valid,
    input wire logic                  sys_mem_d_ready,

    output logic [ADDR_WIDTH-1:0]     periph_mem_a_addr,
    output logic [DATA_WIDTH-1:0]     periph_mem_a_wdata,
    output logic                      periph_mem_a_we,
    output logic [SIZE_WIDTH-1:0]     periph_mem_a_size,
    output logic                      periph_mem_a_valid,
    input wire logic                  periph_mem_a_ready,

    input wire logic [DATA_WIDTH-1:0] periph_mem_d_rdata,
    input wire logic                  periph_mem_d_valid,
    output logic                      periph_mem_d_ready
);

    localparam int A_CHANNEL_WIDTH = ADDR_WIDTH + DATA_WIDTH + 1 + SIZE_WIDTH;

    logic [A_CHANNEL_WIDTH-1:0] sys_a_src_payload;
    logic                       sys_a_src_valid;
    logic                       sys_a_src_next_valid;
    logic                       sys_a_enqueue;
    logic                       sys_a_dequeue;

    logic [DATA_WIDTH-1:0]      periph_d_src_payload;
    logic                       periph_d_src_valid;
    logic                       periph_d_src_next_valid;
    logic                       periph_d_enqueue;
    logic                       periph_d_dequeue;

    logic                       a_cdc_src_ready;
    logic                       a_cdc_dst_valid;
    logic                       a_cdc_dst_ready;
    logic [A_CHANNEL_WIDTH-1:0] a_cdc_dst_payload;

    logic                       d_cdc_src_ready;
    logic                       d_cdc_dst_valid;
    logic                       d_cdc_dst_ready;
    logic [DATA_WIDTH-1:0]      d_cdc_dst_payload;

    initial begin
        if (ADDR_WIDTH < 1) begin
            $fatal(1, "bus_cdc_bridge: ADDR_WIDTH must be >= 1, got %0d", ADDR_WIDTH);
        end
        if (DATA_WIDTH < 1) begin
            $fatal(1, "bus_cdc_bridge: DATA_WIDTH must be >= 1, got %0d", DATA_WIDTH);
        end
        if (SIZE_WIDTH < 1) begin
            $fatal(1, "bus_cdc_bridge: SIZE_WIDTH must be >= 1, got %0d", SIZE_WIDTH);
        end
        if (SYNC_STAGES < 2) begin
            $fatal(1, "bus_cdc_bridge: SYNC_STAGES must be >= 2, got %0d", SYNC_STAGES);
        end
    end

    assign sys_a_enqueue = sys_mem_a_valid && sys_mem_a_ready;
    assign sys_a_dequeue = sys_a_src_valid && a_cdc_src_ready;

    always_comb begin
        sys_a_src_next_valid = sys_a_src_valid;

        if (sys_a_dequeue) begin
            sys_a_src_next_valid = 1'b0;
        end

        if (sys_a_enqueue) begin
            sys_a_src_next_valid = 1'b1;
        end
    end

    always_ff @(posedge sys_clk) begin
        if (rst) begin
            sys_a_src_valid <= 1'b0;
            sys_mem_a_ready <= 1'b1;
            sys_mem_d_valid <= 1'b0;
        end else begin
            if (sys_a_enqueue) begin
                sys_a_src_payload <= {sys_mem_a_addr, sys_mem_a_wdata, sys_mem_a_we, sys_mem_a_size};
            end

            sys_a_src_valid <= sys_a_src_next_valid;
            sys_mem_a_ready <= !sys_a_src_next_valid;

            if (d_cdc_dst_valid && d_cdc_dst_ready) begin
                sys_mem_d_rdata <= d_cdc_dst_payload;
                sys_mem_d_valid <= 1'b1;
            end else if (sys_mem_d_valid && sys_mem_d_ready) begin
                sys_mem_d_valid <= 1'b0;
            end
        end
    end

    assign a_cdc_dst_ready = !periph_mem_a_valid || periph_mem_a_ready;
    assign d_cdc_dst_ready = !sys_mem_d_valid || sys_mem_d_ready;

    always_ff @(posedge periph_clk) begin
        if (rst) begin
            periph_mem_a_valid <= 1'b0;
            periph_d_src_valid <= 1'b0;
            periph_mem_d_ready <= 1'b1;
        end else begin
            if (a_cdc_dst_valid && a_cdc_dst_ready) begin
                {
                    periph_mem_a_addr,
                    periph_mem_a_wdata,
                    periph_mem_a_we,
                    periph_mem_a_size
                } <= a_cdc_dst_payload;
                periph_mem_a_valid <= 1'b1;
            end else if (periph_mem_a_valid && periph_mem_a_ready) begin
                periph_mem_a_valid <= 1'b0;
            end

            if (periph_d_enqueue) begin
                periph_d_src_payload <= periph_mem_d_rdata;
            end

            periph_d_src_valid <= periph_d_src_next_valid;
            periph_mem_d_ready <= !periph_d_src_next_valid;
        end
    end

    assign periph_d_enqueue = periph_mem_d_valid && periph_mem_d_ready;
    assign periph_d_dequeue = periph_d_src_valid && d_cdc_src_ready;

    always_comb begin
        periph_d_src_next_valid = periph_d_src_valid;

        if (periph_d_dequeue) begin
            periph_d_src_next_valid = 1'b0;
        end

        if (periph_d_enqueue) begin
            periph_d_src_next_valid = 1'b1;
        end
    end

    cdc_handshake #(
        .WIDTH(A_CHANNEL_WIDTH),
        .SYNC_STAGES(SYNC_STAGES)
    ) u_addr_channel_cdc (
        .src_clk(sys_clk),
        .dst_clk(periph_clk),
        .rst(rst),
        .src_valid(sys_a_src_valid),
        .src_ready(a_cdc_src_ready),
        .src_data(sys_a_src_payload),
        .dst_valid(a_cdc_dst_valid),
        .dst_ready(a_cdc_dst_ready),
        .dst_data(a_cdc_dst_payload)
    );

    cdc_handshake #(
        .WIDTH(DATA_WIDTH),
        .SYNC_STAGES(SYNC_STAGES)
    ) u_data_channel_cdc (
        .src_clk(periph_clk),
        .dst_clk(sys_clk),
        .rst(rst),
        .src_valid(periph_d_src_valid),
        .src_ready(d_cdc_src_ready),
        .src_data(periph_d_src_payload),
        .dst_valid(d_cdc_dst_valid),
        .dst_ready(d_cdc_dst_ready),
        .dst_data(d_cdc_dst_payload)
    );

endmodule
`default_nettype wire

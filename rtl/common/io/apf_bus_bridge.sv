`default_nettype none

module apf_bus_bridge (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [31:0] bridge_addr,
    input  wire logic        bridge_rd,
    output logic             bridge_rd_ready,
    input  wire logic        bridge_wr,
    output logic             bridge_wr_ready,
    input  wire logic [31:0] bridge_wr_data,
    output logic [31:0]      bridge_rd_data,
    output logic [31:0]      mem_a_addr,
    output logic [31:0]      mem_a_wdata,
    output logic             mem_a_we,
    output logic [1:0]       mem_a_size,
    output logic             mem_a_valid,
    input  wire logic        mem_a_ready,
    input  wire logic [31:0] mem_d_rdata,
    input  wire logic        mem_d_valid,
    output logic             mem_d_ready
);

    typedef enum logic [1:0] {
        S_IDLE,
        S_WAIT_A,
        S_WAIT_D
    } state_t;

    state_t       state;
    logic [31:0]  req_addr;
    logic [31:0]  req_wdata;
    logic         req_we;

    assign bridge_rd_ready = (state == S_IDLE);
    assign bridge_wr_ready = (state == S_IDLE);
    assign mem_a_addr = req_addr;
    assign mem_a_wdata = req_wdata;
    assign mem_a_we = req_we;
    assign mem_a_size = 2'b10;
    assign mem_a_valid = (state == S_WAIT_A);
    assign mem_d_ready = (state == S_WAIT_D);

    always_ff @(posedge clk) begin
        if (rst) begin
            state <= S_IDLE;
        end else begin
            case (state)
                S_IDLE: begin
                    if (bridge_wr) begin
                        req_addr <= bridge_addr;
                        req_wdata <= bridge_wr_data;
                        req_we <= 1'b1;
                        state <= S_WAIT_A;
                    end else if (bridge_rd) begin
                        req_addr <= bridge_addr;
                        req_wdata <= 32'h0000_0000;
                        req_we <= 1'b0;
                        state <= S_WAIT_A;
                    end
                end

                S_WAIT_A: begin
                    if (mem_a_ready) begin
                        state <= S_WAIT_D;
                    end
                end

                S_WAIT_D: begin
                    if (mem_d_valid) begin
                        if (!req_we) begin
                            bridge_rd_data <= mem_d_rdata;
                        end
                        state <= S_IDLE;
                    end
                end

                default: begin
                    state <= S_IDLE;
                end
            endcase
        end
    end

endmodule

`default_nettype wire

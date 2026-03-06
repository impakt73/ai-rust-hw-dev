module registered_bus #(
    parameter int unsigned NUM_SLAVES = 2,
    localparam int unsigned SLAVE_IDX_W = (NUM_SLAVES <= 1) ? 1 : $clog2(NUM_SLAVES)
) (
    input  logic                                 clk,
    input  logic                                 rst_n,

    // Master A channel (input)
    input  logic [31:0]                          master_mem_a_addr,
    input  logic [31:0]                          master_mem_a_wdata,
    input  logic                                 master_mem_a_we,
    input  logic [1:0]                           master_mem_a_size,
    input  logic                                 master_mem_a_valid,
    output logic                                 master_mem_a_ready,

    // Master D channel (output)
    output logic [31:0]                          master_mem_d_rdata,
    output logic                                 master_mem_d_valid,
    input  logic                                 master_mem_d_ready,

    // Slave address map.
    // Decode matches top nibble (addr[31:28]) against slave_base_addr[i][31:28].
    // slave_addr_size is used as an enable: zero disables a slave entry.
    input  logic [NUM_SLAVES-1:0][31:0]          slave_base_addr,
    input  logic [NUM_SLAVES-1:0][31:0]          slave_addr_size,

    // Slave A channels (output)
    output logic [NUM_SLAVES-1:0][31:0]          slave_mem_a_addr,
    output logic [NUM_SLAVES-1:0][31:0]          slave_mem_a_wdata,
    output logic [NUM_SLAVES-1:0]                slave_mem_a_we,
    output logic [NUM_SLAVES-1:0][1:0]           slave_mem_a_size,
    output logic [NUM_SLAVES-1:0]                slave_mem_a_valid,
    input  logic [NUM_SLAVES-1:0]                slave_mem_a_ready,

    // Slave D channels (input/output)
    input  logic [NUM_SLAVES-1:0][31:0]          slave_mem_d_rdata,
    input  logic [NUM_SLAVES-1:0]                slave_mem_d_valid,
    output logic [NUM_SLAVES-1:0]                slave_mem_d_ready
);

    typedef enum logic [1:0] {
        S_IDLE      = 2'd0,
        S_SLAVE_REQ = 2'd1,
        S_SLAVE_RESP = 2'd2,
        S_MASTER_RESP = 2'd3
    } state_t;

    state_t state;

    logic [31:0] pending_req_addr;
    logic [31:0] pending_req_wdata;
    logic        pending_req_we;
    logic [1:0]  pending_req_size;

    logic [31:0] pending_resp_rdata;
    logic        pending_resp_valid;

    logic [SLAVE_IDX_W-1:0] selected_slave_idx;
    logic                   selected_slave_valid;

    logic [SLAVE_IDX_W-1:0] req_slave_idx;
    logic                   req_slave_valid;
    logic                   req_slave_a_ready;
    logic                   selected_slave_d_valid;
    logic [31:0]            selected_slave_d_rdata;

    logic                   a_handshake;
    logic                   d_handshake;

    logic [SLAVE_IDX_W-1:0] decoded_slave_idx;
    logic                   decoded_slave_valid;

    integer i;

    always_comb begin
        decoded_slave_idx = '0;
        decoded_slave_valid = 1'b0;

        for (i = 0; i < NUM_SLAVES; i++) begin
            if (!decoded_slave_valid
                && (slave_addr_size[i] != 32'h0)
                && (pending_req_addr[31:28] == slave_base_addr[i][31:28])) begin
                decoded_slave_idx = SLAVE_IDX_W'(i);
                decoded_slave_valid = 1'b1;
            end
        end
    end

    always_comb begin
        req_slave_idx = selected_slave_idx;
        req_slave_valid = selected_slave_valid;
        req_slave_a_ready = 1'b0;
        selected_slave_d_valid = 1'b0;
        selected_slave_d_rdata = 32'h0;

        if (state == S_SLAVE_REQ) begin
            req_slave_idx = decoded_slave_idx;
            req_slave_valid = decoded_slave_valid;
        end

        if (req_slave_valid) begin
            req_slave_a_ready = slave_mem_a_ready[req_slave_idx];
        end

        if (selected_slave_valid) begin
            selected_slave_d_valid = slave_mem_d_valid[selected_slave_idx];
            selected_slave_d_rdata = slave_mem_d_rdata[selected_slave_idx];
        end
    end

    assign a_handshake = master_mem_a_valid && master_mem_a_ready;
    assign d_handshake = master_mem_d_valid && master_mem_d_ready;

    assign master_mem_a_ready = (state == S_IDLE) && !pending_resp_valid;
    assign master_mem_d_valid = pending_resp_valid;
    assign master_mem_d_rdata = pending_resp_rdata;

    always_comb begin
        slave_mem_a_addr  = '{default: pending_req_addr};
        slave_mem_a_wdata = '{default: pending_req_wdata};
        slave_mem_a_we    = '0;
        slave_mem_a_size  = '{default: pending_req_size};
        slave_mem_a_valid = '0;
        slave_mem_d_ready = '0;

        if ((state == S_SLAVE_REQ) && req_slave_valid) begin
            slave_mem_a_valid[req_slave_idx] = 1'b1;
            slave_mem_a_we[req_slave_idx] = pending_req_we;
        end

        if ((state == S_SLAVE_RESP) && selected_slave_valid) begin
            slave_mem_d_ready[selected_slave_idx] = !pending_resp_valid;
        end
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            state <= S_IDLE;
            pending_req_addr <= 32'h0;
            pending_req_wdata <= 32'h0;
            pending_req_we <= 1'b0;
            pending_req_size <= 2'b00;
            pending_resp_rdata <= 32'h0;
            pending_resp_valid <= 1'b0;
            selected_slave_idx <= '0;
            selected_slave_valid <= 1'b0;
        end else begin
            case (state)
                S_IDLE: begin
                    if (a_handshake) begin
                        pending_req_addr <= master_mem_a_addr;
                        pending_req_wdata <= master_mem_a_wdata;
                        pending_req_we <= master_mem_a_we;
                        pending_req_size <= master_mem_a_size;
                        state <= S_SLAVE_REQ;
                    end
                end

                S_SLAVE_REQ: begin
                    selected_slave_idx <= decoded_slave_idx;
                    selected_slave_valid <= decoded_slave_valid;

                    if (!decoded_slave_valid) begin
                        pending_resp_rdata <= 32'h0;
                        pending_resp_valid <= 1'b1;
                        state <= S_MASTER_RESP;
                    end else if (req_slave_a_ready) begin
                        state <= S_SLAVE_RESP;
                    end
                end

                S_SLAVE_RESP: begin
                    if (selected_slave_d_valid
                        && selected_slave_valid
                        && slave_mem_d_ready[selected_slave_idx]
                        && !pending_resp_valid) begin
                        pending_resp_rdata <= selected_slave_d_rdata;
                        pending_resp_valid <= 1'b1;
                        state <= S_MASTER_RESP;
                    end
                end

                S_MASTER_RESP: begin
                    if (d_handshake) begin
                        pending_resp_valid <= 1'b0;
                        selected_slave_valid <= 1'b0;
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

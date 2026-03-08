module registered_bus #(
    parameter int unsigned NUM_MASTERS = 1,
    parameter int unsigned NUM_SLAVES = 2,
    localparam int unsigned MASTER_IDX_W = (NUM_MASTERS <= 1) ? 1 : $clog2(NUM_MASTERS),
    localparam int unsigned SLAVE_IDX_W = (NUM_SLAVES <= 1) ? 1 : $clog2(NUM_SLAVES)
) (
    input  logic                                 clk,
    input  logic                                 rst_n,

    // Master A channels (input)
    input  logic [NUM_MASTERS*32-1:0]            master_mem_a_addr,
    input  logic [NUM_MASTERS*32-1:0]            master_mem_a_wdata,
    input  logic [NUM_MASTERS-1:0]               master_mem_a_we,
    input  logic [NUM_MASTERS*2-1:0]             master_mem_a_size,
    input  logic [NUM_MASTERS-1:0]               master_mem_a_valid,
    output logic [NUM_MASTERS-1:0]               master_mem_a_ready,

    // Master D channels (output/input)
    output logic [NUM_MASTERS*32-1:0]            master_mem_d_rdata,
    output logic [NUM_MASTERS-1:0]               master_mem_d_valid,
    input  logic [NUM_MASTERS-1:0]               master_mem_d_ready,

    // Slave address map.
    // Decode matches top nibble (addr[31:28]) against slave_base_addr[i][31:28].
    // slave_addr_size is used as an enable: zero disables a slave entry.
    input  logic [NUM_SLAVES*32-1:0]             slave_base_addr,
    input  logic [NUM_SLAVES*32-1:0]             slave_addr_size,

    // Slave A channels (output/input)
    output logic [NUM_SLAVES*32-1:0]             slave_mem_a_addr,
    output logic [NUM_SLAVES*32-1:0]             slave_mem_a_wdata,
    output logic [NUM_SLAVES-1:0]                slave_mem_a_we,
    output logic [NUM_SLAVES*2-1:0]              slave_mem_a_size,
    output logic [NUM_SLAVES-1:0]                slave_mem_a_valid,
    input  logic [NUM_SLAVES-1:0]                slave_mem_a_ready,

    // Slave D channels (input/output)
    input  logic [NUM_SLAVES*32-1:0]             slave_mem_d_rdata,
    input  logic [NUM_SLAVES-1:0]                slave_mem_d_valid,
    output logic [NUM_SLAVES-1:0]                slave_mem_d_ready
);

    logic [31:0]                                 master_mem_a_addr_int [NUM_MASTERS];
    logic [31:0]                                 master_mem_a_wdata_int [NUM_MASTERS];
    logic [1:0]                                  master_mem_a_size_int [NUM_MASTERS];
    logic [31:0]                                 master_mem_d_rdata_int [NUM_MASTERS];

    logic [31:0]                                 slave_base_addr_int [NUM_SLAVES];
    logic [31:0]                                 slave_addr_size_int [NUM_SLAVES];
    logic [31:0]                                 slave_mem_a_addr_int [NUM_SLAVES];
    logic [31:0]                                 slave_mem_a_wdata_int [NUM_SLAVES];
    logic [1:0]                                  slave_mem_a_size_int [NUM_SLAVES];
    logic [31:0]                                 slave_mem_d_rdata_int [NUM_SLAVES];

    for (genvar master_idx = 0; master_idx < NUM_MASTERS; master_idx++) begin : gen_master_bus_vectors
        assign master_mem_a_addr_int[master_idx] = master_mem_a_addr[(master_idx*32) +: 32];
        assign master_mem_a_wdata_int[master_idx] = master_mem_a_wdata[(master_idx*32) +: 32];
        assign master_mem_a_size_int[master_idx] = master_mem_a_size[(master_idx*2) +: 2];
        assign master_mem_d_rdata[(master_idx*32) +: 32] = master_mem_d_rdata_int[master_idx];
    end

    for (genvar slave_idx = 0; slave_idx < NUM_SLAVES; slave_idx++) begin : gen_slave_bus_vectors
        assign slave_base_addr_int[slave_idx] = slave_base_addr[(slave_idx*32) +: 32];
        assign slave_addr_size_int[slave_idx] = slave_addr_size[(slave_idx*32) +: 32];
        assign slave_mem_a_addr[(slave_idx*32) +: 32] = slave_mem_a_addr_int[slave_idx];
        assign slave_mem_a_wdata[(slave_idx*32) +: 32] = slave_mem_a_wdata_int[slave_idx];
        assign slave_mem_a_size[(slave_idx*2) +: 2] = slave_mem_a_size_int[slave_idx];
        assign slave_mem_d_rdata_int[slave_idx] = slave_mem_d_rdata[(slave_idx*32) +: 32];
    end

    logic                                        pending_req_valid;
    logic [MASTER_IDX_W-1:0]                     pending_req_master_idx;
    logic [31:0]                                 pending_req_addr;
    logic [31:0]                                 pending_req_wdata;
    logic                                        pending_req_we;
    logic [1:0]                                  pending_req_size;

    logic                                        pending_resp_valid;
    logic [MASTER_IDX_W-1:0]                     pending_resp_master_idx;
    logic [31:0]                                 pending_resp_rdata;

    logic [NUM_SLAVES-1:0]                       slave_response_pending;
    logic [NUM_SLAVES*MASTER_IDX_W-1:0]         slave_response_master_idx;

    logic [MASTER_IDX_W-1:0]                     selected_master_idx;
    logic                                        selected_master_valid;
    logic [SLAVE_IDX_W-1:0]                      decoded_slave_idx;
    logic                                        decoded_slave_valid;
    logic [SLAVE_IDX_W-1:0]                      selected_resp_slave_idx;
    logic                                        selected_resp_slave_valid;

    logic                                        master_req_accept;
    logic                                        slave_req_accept;
    logic                                        unmapped_req_accept;
    logic                                        slave_resp_accept;
    logic                                        master_resp_accept;

    always_comb begin
        selected_master_idx = '0;
        selected_master_valid = 1'b0;

        // Fixed-priority arbitration: lowest index master wins each cycle.
        for (int unsigned master_idx = 0; master_idx < NUM_MASTERS; master_idx++) begin
            if (!selected_master_valid && master_mem_a_valid[master_idx]) begin
                selected_master_idx = MASTER_IDX_W'(master_idx);
                selected_master_valid = 1'b1;
            end
        end
    end

    always_comb begin
        decoded_slave_idx = '0;
        decoded_slave_valid = 1'b0;

        for (int unsigned slave_idx = 0; slave_idx < NUM_SLAVES; slave_idx++) begin
            if (!decoded_slave_valid
                && (slave_addr_size_int[slave_idx] != 32'h0)
                && (pending_req_addr[31:28] == slave_base_addr_int[slave_idx][31:28])) begin
                decoded_slave_idx = SLAVE_IDX_W'(slave_idx);
                decoded_slave_valid = 1'b1;
            end
        end
    end

    always_comb begin
        selected_resp_slave_idx = '0;
        selected_resp_slave_valid = 1'b0;

        // Fixed-priority arbitration: lowest index slave response wins each cycle.
        for (int unsigned slave_idx = 0; slave_idx < NUM_SLAVES; slave_idx++) begin
            if (!selected_resp_slave_valid
                && slave_response_pending[slave_idx]
                && slave_mem_d_valid[slave_idx]) begin
                selected_resp_slave_idx = SLAVE_IDX_W'(slave_idx);
                selected_resp_slave_valid = 1'b1;
            end
        end
    end

    always_comb begin
        master_mem_a_ready = '0;
        master_mem_d_valid = '0;
        slave_mem_a_we = '0;
        slave_mem_a_valid = '0;
        slave_mem_d_ready = '0;

        for (int unsigned master_idx = 0; master_idx < NUM_MASTERS; master_idx++) begin
            master_mem_d_rdata_int[master_idx] = 32'h0;
        end

        for (int unsigned slave_idx = 0; slave_idx < NUM_SLAVES; slave_idx++) begin
            slave_mem_a_addr_int[slave_idx] = 32'h0;
            slave_mem_a_wdata_int[slave_idx] = 32'h0;
            slave_mem_a_size_int[slave_idx] = 2'b00;
        end

        if (!pending_req_valid && selected_master_valid) begin
            master_mem_a_ready[selected_master_idx] = 1'b1;
        end

        if (pending_req_valid && decoded_slave_valid && !slave_response_pending[decoded_slave_idx]) begin
            slave_mem_a_addr_int[decoded_slave_idx] = pending_req_addr;
            slave_mem_a_wdata_int[decoded_slave_idx] = pending_req_wdata;
            slave_mem_a_we[decoded_slave_idx] = pending_req_we;
            slave_mem_a_size_int[decoded_slave_idx] = pending_req_size;
            slave_mem_a_valid[decoded_slave_idx] = 1'b1;
        end

        if (!pending_resp_valid && selected_resp_slave_valid) begin
            slave_mem_d_ready[selected_resp_slave_idx] = 1'b1;
        end

        if (pending_resp_valid) begin
            master_mem_d_rdata_int[pending_resp_master_idx] = pending_resp_rdata;
            master_mem_d_valid[pending_resp_master_idx] = 1'b1;
        end
    end

    assign master_req_accept = !pending_req_valid && selected_master_valid;
    assign slave_req_accept = pending_req_valid
                              && decoded_slave_valid
                              && !slave_response_pending[decoded_slave_idx]
                              && slave_mem_a_ready[decoded_slave_idx];
    assign unmapped_req_accept = pending_req_valid && !decoded_slave_valid && !pending_resp_valid;
    assign slave_resp_accept = !pending_resp_valid && !unmapped_req_accept && selected_resp_slave_valid;
    assign master_resp_accept = pending_resp_valid && master_mem_d_ready[pending_resp_master_idx];

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            pending_req_valid <= 1'b0;
            pending_req_master_idx <= '0;
            pending_req_addr <= 32'h0;
            pending_req_wdata <= 32'h0;
            pending_req_we <= 1'b0;
            pending_req_size <= 2'b00;

            pending_resp_valid <= 1'b0;
            pending_resp_master_idx <= '0;
            pending_resp_rdata <= 32'h0;

            slave_response_pending <= '0;
            slave_response_master_idx <= '0;
        end else begin
            if (master_req_accept) begin
                pending_req_valid <= 1'b1;
                pending_req_master_idx <= selected_master_idx;
                pending_req_addr <= master_mem_a_addr_int[selected_master_idx];
                pending_req_wdata <= master_mem_a_wdata_int[selected_master_idx];
                pending_req_we <= master_mem_a_we[selected_master_idx];
                pending_req_size <= master_mem_a_size_int[selected_master_idx];
            end

            if (slave_req_accept) begin
                pending_req_valid <= 1'b0;
                slave_response_pending[decoded_slave_idx] <= 1'b1;
                slave_response_master_idx[(decoded_slave_idx*MASTER_IDX_W) +: MASTER_IDX_W] <= pending_req_master_idx;
            end else if (unmapped_req_accept) begin
                pending_req_valid <= 1'b0;
                pending_resp_valid <= 1'b1;
                pending_resp_master_idx <= pending_req_master_idx;
                pending_resp_rdata <= 32'h0;
            end

            if (slave_resp_accept) begin
                pending_resp_valid <= 1'b1;
                pending_resp_master_idx <=
                    slave_response_master_idx[(selected_resp_slave_idx*MASTER_IDX_W) +: MASTER_IDX_W];
                pending_resp_rdata <= slave_mem_d_rdata_int[selected_resp_slave_idx];
                slave_response_pending[selected_resp_slave_idx] <= 1'b0;
            end else if (master_resp_accept) begin
                pending_resp_valid <= 1'b0;
            end
        end
    end
endmodule

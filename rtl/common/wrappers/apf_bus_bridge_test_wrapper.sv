`default_nettype none

module apf_bus_bridge_test_wrapper (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [31:0] bridge_addr,
    input  wire logic        bridge_rd,
    input  wire logic        bridge_wr,
    input  wire logic [31:0] bridge_wr_data,
    output logic [31:0]      bridge_rd_data
);

    logic [31:0] master_mem_a_addr;
    logic [31:0] master_mem_a_wdata;
    logic        master_mem_a_we;
    logic [1:0]  master_mem_a_size;
    logic        master_mem_a_valid;
    logic        master_mem_a_ready;
    logic [31:0] master_mem_d_rdata;
    logic        master_mem_d_valid;
    logic        master_mem_d_ready;

    logic [31:0] slave_mem_a_addr;
    logic [31:0] slave_mem_a_wdata;
    logic        slave_mem_a_we;
    logic [1:0]  slave_mem_a_size;
    logic        slave_mem_a_valid;
    logic        slave_mem_a_ready;
    logic [31:0] slave_mem_d_rdata;
    logic        slave_mem_d_valid;
    logic        slave_mem_d_ready;

    apf_bus_bridge u_apf_bus_bridge (
        .clk(clk),
        .rst(rst),
        .bridge_addr(bridge_addr),
        .bridge_rd(bridge_rd),
        .bridge_wr(bridge_wr),
        .bridge_wr_data(bridge_wr_data),
        .bridge_rd_data(bridge_rd_data),
        .mem_a_addr(master_mem_a_addr),
        .mem_a_wdata(master_mem_a_wdata),
        .mem_a_we(master_mem_a_we),
        .mem_a_size(master_mem_a_size),
        .mem_a_valid(master_mem_a_valid),
        .mem_a_ready(master_mem_a_ready),
        .mem_d_rdata(master_mem_d_rdata),
        .mem_d_valid(master_mem_d_valid),
        .mem_d_ready(master_mem_d_ready)
    );

    registered_bus #(
        .NUM_MASTERS(1),
        .NUM_SLAVES(1)
    ) u_registered_bus (
        .clk(clk),
        .rst(rst),
        .master_mem_a_addr(master_mem_a_addr),
        .master_mem_a_wdata(master_mem_a_wdata),
        .master_mem_a_we(master_mem_a_we),
        .master_mem_a_size(master_mem_a_size),
        .master_mem_a_valid(master_mem_a_valid),
        .master_mem_a_ready(master_mem_a_ready),
        .master_mem_d_rdata(master_mem_d_rdata),
        .master_mem_d_valid(master_mem_d_valid),
        .master_mem_d_ready(master_mem_d_ready),
        .slave_base_addr(32'h7000_0000),
        .slave_addr_size(32'h0000_3000),
        .slave_mem_a_addr(slave_mem_a_addr),
        .slave_mem_a_wdata(slave_mem_a_wdata),
        .slave_mem_a_we(slave_mem_a_we),
        .slave_mem_a_size(slave_mem_a_size),
        .slave_mem_a_valid(slave_mem_a_valid),
        .slave_mem_a_ready(slave_mem_a_ready),
        .slave_mem_d_rdata(slave_mem_d_rdata),
        .slave_mem_d_valid(slave_mem_d_valid),
        .slave_mem_d_ready(slave_mem_d_ready)
    );

    sram_peripheral u_sram_peripheral (
        .clk(clk),
        .rst(rst),
        .mem_a_addr(slave_mem_a_addr),
        .mem_a_wdata(slave_mem_a_wdata),
        .mem_a_we(slave_mem_a_we),
        .mem_a_size(slave_mem_a_size),
        .mem_a_valid(slave_mem_a_valid),
        .mem_a_ready(slave_mem_a_ready),
        .mem_d_rdata(slave_mem_d_rdata),
        .mem_d_valid(slave_mem_d_valid),
        .mem_d_ready(slave_mem_d_ready)
    );

endmodule

`default_nettype wire

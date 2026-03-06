module registered_bus_wrapper (
    input  logic        clk,
    input  logic        rst_n,

    // Master 0 A channel
    input  logic [31:0] master0_mem_a_addr,
    input  logic [31:0] master0_mem_a_wdata,
    input  logic        master0_mem_a_we,
    input  logic [1:0]  master0_mem_a_size,
    input  logic        master0_mem_a_valid,
    output logic        master0_mem_a_ready,

    // Master 0 D channel
    output logic [31:0] master0_mem_d_rdata,
    output logic        master0_mem_d_valid,
    input  logic        master0_mem_d_ready,

    // Master 1 A channel
    input  logic [31:0] master1_mem_a_addr,
    input  logic [31:0] master1_mem_a_wdata,
    input  logic        master1_mem_a_we,
    input  logic [1:0]  master1_mem_a_size,
    input  logic        master1_mem_a_valid,
    output logic        master1_mem_a_ready,

    // Master 1 D channel
    output logic [31:0] master1_mem_d_rdata,
    output logic        master1_mem_d_valid,
    input  logic        master1_mem_d_ready,

    // Slave 0 configuration
    input  logic [31:0] slave0_base_addr,
    input  logic [31:0] slave0_addr_size,

    // Slave 0 A channel
    output logic [31:0] slave0_mem_a_addr,
    output logic [31:0] slave0_mem_a_wdata,
    output logic        slave0_mem_a_we,
    output logic [1:0]  slave0_mem_a_size,
    output logic        slave0_mem_a_valid,
    input  logic        slave0_mem_a_ready,

    // Slave 0 D channel
    input  logic [31:0] slave0_mem_d_rdata,
    input  logic        slave0_mem_d_valid,
    output logic        slave0_mem_d_ready,

    // Slave 1 configuration
    input  logic [31:0] slave1_base_addr,
    input  logic [31:0] slave1_addr_size,

    // Slave 1 A channel
    output logic [31:0] slave1_mem_a_addr,
    output logic [31:0] slave1_mem_a_wdata,
    output logic        slave1_mem_a_we,
    output logic [1:0]  slave1_mem_a_size,
    output logic        slave1_mem_a_valid,
    input  logic        slave1_mem_a_ready,

    // Slave 1 D channel
    input  logic [31:0] slave1_mem_d_rdata,
    input  logic        slave1_mem_d_valid,
    output logic        slave1_mem_d_ready
);

    logic [1:0][31:0] master_mem_a_addr;
    logic [1:0][31:0] master_mem_a_wdata;
    logic [1:0]       master_mem_a_we;
    logic [1:0][1:0]  master_mem_a_size;
    logic [1:0]       master_mem_a_valid;
    logic [1:0]       master_mem_a_ready;

    logic [1:0][31:0] master_mem_d_rdata;
    logic [1:0]       master_mem_d_valid;
    logic [1:0]       master_mem_d_ready;

    logic [1:0][31:0] slave_base_addr;
    logic [1:0][31:0] slave_addr_size;

    logic [1:0][31:0] slave_mem_a_addr;
    logic [1:0][31:0] slave_mem_a_wdata;
    logic [1:0]       slave_mem_a_we;
    logic [1:0][1:0]  slave_mem_a_size;
    logic [1:0]       slave_mem_a_valid;
    logic [1:0]       slave_mem_a_ready;

    logic [1:0][31:0] slave_mem_d_rdata;
    logic [1:0]       slave_mem_d_valid;
    logic [1:0]       slave_mem_d_ready;

    assign master_mem_a_addr[0] = master0_mem_a_addr;
    assign master_mem_a_addr[1] = master1_mem_a_addr;
    assign master_mem_a_wdata[0] = master0_mem_a_wdata;
    assign master_mem_a_wdata[1] = master1_mem_a_wdata;
    assign master_mem_a_we[0] = master0_mem_a_we;
    assign master_mem_a_we[1] = master1_mem_a_we;
    assign master_mem_a_size[0] = master0_mem_a_size;
    assign master_mem_a_size[1] = master1_mem_a_size;
    assign master_mem_a_valid[0] = master0_mem_a_valid;
    assign master_mem_a_valid[1] = master1_mem_a_valid;
    assign master0_mem_a_ready = master_mem_a_ready[0];
    assign master1_mem_a_ready = master_mem_a_ready[1];

    assign master0_mem_d_rdata = master_mem_d_rdata[0];
    assign master1_mem_d_rdata = master_mem_d_rdata[1];
    assign master0_mem_d_valid = master_mem_d_valid[0];
    assign master1_mem_d_valid = master_mem_d_valid[1];
    assign master_mem_d_ready[0] = master0_mem_d_ready;
    assign master_mem_d_ready[1] = master1_mem_d_ready;

    assign slave_base_addr[0] = slave0_base_addr;
    assign slave_base_addr[1] = slave1_base_addr;
    assign slave_addr_size[0] = slave0_addr_size;
    assign slave_addr_size[1] = slave1_addr_size;

    assign slave0_mem_a_addr = slave_mem_a_addr[0];
    assign slave1_mem_a_addr = slave_mem_a_addr[1];
    assign slave0_mem_a_wdata = slave_mem_a_wdata[0];
    assign slave1_mem_a_wdata = slave_mem_a_wdata[1];
    assign slave0_mem_a_we = slave_mem_a_we[0];
    assign slave1_mem_a_we = slave_mem_a_we[1];
    assign slave0_mem_a_size = slave_mem_a_size[0];
    assign slave1_mem_a_size = slave_mem_a_size[1];
    assign slave0_mem_a_valid = slave_mem_a_valid[0];
    assign slave1_mem_a_valid = slave_mem_a_valid[1];
    assign slave_mem_a_ready[0] = slave0_mem_a_ready;
    assign slave_mem_a_ready[1] = slave1_mem_a_ready;

    assign slave_mem_d_rdata[0] = slave0_mem_d_rdata;
    assign slave_mem_d_rdata[1] = slave1_mem_d_rdata;
    assign slave_mem_d_valid[0] = slave0_mem_d_valid;
    assign slave_mem_d_valid[1] = slave1_mem_d_valid;
    assign slave0_mem_d_ready = slave_mem_d_ready[0];
    assign slave1_mem_d_ready = slave_mem_d_ready[1];

    registered_bus #(
        .NUM_MASTERS(2),
        .NUM_SLAVES(2)
    ) u_registered_bus (
        .clk(clk),
        .rst_n(rst_n),
        .master_mem_a_addr(master_mem_a_addr),
        .master_mem_a_wdata(master_mem_a_wdata),
        .master_mem_a_we(master_mem_a_we),
        .master_mem_a_size(master_mem_a_size),
        .master_mem_a_valid(master_mem_a_valid),
        .master_mem_a_ready(master_mem_a_ready),
        .master_mem_d_rdata(master_mem_d_rdata),
        .master_mem_d_valid(master_mem_d_valid),
        .master_mem_d_ready(master_mem_d_ready),
        .slave_base_addr(slave_base_addr),
        .slave_addr_size(slave_addr_size),
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

endmodule

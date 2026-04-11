`default_nettype none

module sdram_peripheral_test_wrapper (
    input  wire logic        sys_clk,
    input  wire logic        sdram_clk,
    input  wire logic        rst,
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready,
    output logic [3:0]       burst_rd_count,
    output logic [3:0]       burstwr_strobe_count,
    output logic [7:0]       burstwr_wait_cycle_count,
    output logic             periph_a_ready_dbg
);

    localparam logic [31:0] BASE_ADDR = 32'h1000_0000;
    localparam logic [31:0] ADDR_SIZE = 32'h0000_0100;
    localparam int unsigned BURST_ADDR_WIDTH = 25;
    localparam int unsigned HALFWORD_DEPTH = ADDR_SIZE / 2;
    localparam int unsigned HALFWORD_ADDR_WIDTH = $clog2(HALFWORD_DEPTH);
    localparam logic [BURST_ADDR_WIDTH-1:0] HALFWORD_DEPTH_LIMIT = BURST_ADDR_WIDTH'(HALFWORD_DEPTH);

    logic                              burst_rd;
    logic [BURST_ADDR_WIDTH-1:0]       burst_addr;
    logic [10:0]                       burst_len;
    logic                              burst_32bit;
    logic [31:0]                       burst_data;
    logic                              burst_data_valid;
    logic                              burst_data_done;
    logic                              burstwr;
    logic [BURST_ADDR_WIDTH-1:0]       burstwr_addr;
    logic                              burstwr_ready;
    logic                              burstwr_strobe;
    logic [15:0]                       burstwr_data;
    logic                              burstwr_done;

    logic [15:0]                       halfword_mem[0:HALFWORD_DEPTH-1];
    logic                              read_pending;
    logic [BURST_ADDR_WIDTH-1:0]       read_addr_reg;
    logic [1:0]                        read_words_remaining_reg;
    logic                              burstwr_pending;
    logic [1:0]                        burstwr_wait_cycles_remaining;

    function automatic logic [15:0] read_halfword(
        input logic [BURST_ADDR_WIDTH-1:0] addr
    );
        if (addr < HALFWORD_DEPTH_LIMIT) begin
            read_halfword = halfword_mem[addr[HALFWORD_ADDR_WIDTH-1:0]];
        end else begin
            read_halfword = 16'h0000;
        end
    endfunction

    initial begin
        // This is a test-only memory model. Each Rust test constructs a fresh wrapper
        // instance, so initializing the backing store once at elaboration time keeps the
        // model deterministic while the synchronous reset below still owns all control
        // state visible to the peripheral.
        for (int unsigned idx = 0; idx < HALFWORD_DEPTH; idx++) begin
            halfword_mem[idx] = 16'h0000;
        end
    end

    // Mirror io_sdram's registered ready behavior so ready can remain high for one
    // cycle after the strobe that ends the current write window.
    assign periph_a_ready_dbg = u_sdram_peripheral.periph_mem_a_ready;

    sdram_peripheral #(
        .BASE_ADDR(BASE_ADDR),
        .ADDR_SIZE(ADDR_SIZE),
        .BURST_ADDR_WIDTH(BURST_ADDR_WIDTH),
        .BUS_CDC_SYNC_STAGES(2)
    ) u_sdram_peripheral (
        .sys_clk(sys_clk),
        .sdram_clk(sdram_clk),
        .rst(rst),
        .mem_a_addr(mem_a_addr),
        .mem_a_wdata(mem_a_wdata),
        .mem_a_we(mem_a_we),
        .mem_a_size(mem_a_size),
        .mem_a_valid(mem_a_valid),
        .mem_a_ready(mem_a_ready),
        .mem_d_rdata(mem_d_rdata),
        .mem_d_valid(mem_d_valid),
        .mem_d_ready(mem_d_ready),
        .burst_rd(burst_rd),
        .burst_addr(burst_addr),
        .burst_len(burst_len),
        .burst_32bit(burst_32bit),
        .burst_data(burst_data),
        .burst_data_valid(burst_data_valid),
        .burst_data_done(burst_data_done),
        .burstwr(burstwr),
        .burstwr_addr(burstwr_addr),
        .burstwr_ready(burstwr_ready),
        .burstwr_strobe(burstwr_strobe),
        .burstwr_data(burstwr_data),
        .burstwr_done(burstwr_done)
    );

    always_ff @(posedge sdram_clk) begin
        burst_data_valid <= 1'b0;
        burst_data_done <= 1'b0;

        if (rst) begin
            read_pending <= 1'b0;
            read_addr_reg <= '0;
            read_words_remaining_reg <= '0;
            burstwr_pending <= 1'b0;
            burstwr_wait_cycles_remaining <= '0;
            burstwr_ready <= 1'b0;
            burst_data <= '0;
            burst_rd_count <= '0;
            burstwr_strobe_count <= '0;
            burstwr_wait_cycle_count <= '0;
        end else begin
            burstwr_ready <= burstwr_pending && (burstwr_wait_cycles_remaining == 2'd0);

            if (burst_rd) begin
                read_pending <= 1'b1;
                read_addr_reg <= burst_addr;
                read_words_remaining_reg <= (burst_len == 11'd4) ? 2'd2 : 2'd1;
                burst_rd_count <= burst_rd_count + 1'b1;
            end

            if (read_pending) begin
                burst_data <= {
                    read_halfword(read_addr_reg),
                    read_halfword(read_addr_reg + BURST_ADDR_WIDTH'(1))
                };
                burst_data_valid <= 1'b1;

                if (read_words_remaining_reg == 2'd1) begin
                    burst_data_done <= 1'b1;
                    read_pending <= 1'b0;
                end else begin
                    read_addr_reg <= read_addr_reg + BURST_ADDR_WIDTH'(2);
                    read_words_remaining_reg <= read_words_remaining_reg - 1'b1;
                end
            end

            if (burstwr_strobe) begin
                if (burstwr_addr < HALFWORD_DEPTH_LIMIT) begin
                    halfword_mem[burstwr_addr[HALFWORD_ADDR_WIDTH-1:0]] <= burstwr_data;
                end
                burstwr_pending <= 1'b0;
                burstwr_wait_cycles_remaining <= '0;
                burstwr_strobe_count <= burstwr_strobe_count + 1'b1;
            end

            // Latch a new write request either from the initial burstwr pulse
            // (!burstwr_pending) or from the handoff cycle where the previous halfword
            // strobes and the DUT immediately advances to request the next halfword
            // (burstwr_strobe).
            if (burstwr && (!burstwr_pending || burstwr_strobe)) begin
                burstwr_pending <= 1'b1;
                burstwr_wait_cycles_remaining <= 2'd2;
            end else if (burstwr_pending && !burstwr_strobe && (burstwr_wait_cycles_remaining != 2'd0)) begin
                burstwr_wait_cycles_remaining <= burstwr_wait_cycles_remaining - 1'b1;
                burstwr_wait_cycle_count <= burstwr_wait_cycle_count + 1'b1;
            end
        end
    end

endmodule

`default_nettype wire

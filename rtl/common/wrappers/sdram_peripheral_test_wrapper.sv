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
    output logic [3:0]       word_rd_count,
    output logic [3:0]       word_wr_count,
    output logic [7:0]       word_wait_cycle_count,
    output logic             periph_a_ready_dbg
);

    localparam logic [31:0] BASE_ADDR = 32'h1000_0000;
    localparam logic [31:0] ADDR_SIZE = 32'h0000_0100;
    localparam int unsigned WORD_ADDR_WIDTH = 24;
    localparam int unsigned WORD_DEPTH = ADDR_SIZE / 4;
    localparam logic [WORD_ADDR_WIDTH-1:0] WORD_DEPTH_LIMIT = WORD_ADDR_WIDTH'(WORD_DEPTH);
    localparam logic [WORD_ADDR_WIDTH-1:0] EXTENDED_BUSY_WORD_ADDR = WORD_ADDR_WIDTH'('h10);
    localparam int unsigned WORD_WAIT_COUNTER_WIDTH = 6;

    logic                         word_rd;
    logic                         word_wr;
    logic [WORD_ADDR_WIDTH-1:0]   word_addr;
    logic [31:0]                  word_data;
    logic [31:0]                  word_q;
    logic                         word_busy;

    logic [31:0]                  word_mem[0:WORD_DEPTH-1];
    logic                         pending_is_write;
    logic [WORD_ADDR_WIDTH-1:0]   pending_word_addr;
    logic [31:0]                  pending_word_data;
    logic [WORD_WAIT_COUNTER_WIDTH-1:0] wait_cycles_remaining;

    function automatic logic [WORD_WAIT_COUNTER_WIDTH-1:0] word_wait_cycles_for_request(
        input logic [WORD_ADDR_WIDTH-1:0] addr,
        input logic                       is_write
    );
        begin
            // Reads complete one cycle sooner than writes in this stub so the testbench
            // exercises both the nominal path and longer write-side busy windows.
            if (addr == EXTENDED_BUSY_WORD_ADDR) begin
                word_wait_cycles_for_request = WORD_WAIT_COUNTER_WIDTH'(40);
            end else if (is_write) begin
                word_wait_cycles_for_request = WORD_WAIT_COUNTER_WIDTH'(2);
            end else begin
                word_wait_cycles_for_request = WORD_WAIT_COUNTER_WIDTH'(1);
            end
        end
    endfunction

    function automatic logic [31:0] read_word(
        input logic [WORD_ADDR_WIDTH-1:0] addr
    );
        begin
            if (addr < WORD_DEPTH_LIMIT) begin
                read_word = word_mem[int'(addr)];
            end else begin
                read_word = 32'h0000_0000;
            end
        end
    endfunction

    initial begin
        for (int unsigned idx = 0; idx < WORD_DEPTH; idx++) begin
            word_mem[idx] = 32'h0000_0000;
        end
    end

    assign periph_a_ready_dbg = u_sdram_peripheral.periph_mem_a_ready;

    sdram_peripheral #(
        .BASE_ADDR(BASE_ADDR),
        .ADDR_SIZE(ADDR_SIZE),
        .WORD_ADDR_WIDTH(WORD_ADDR_WIDTH),
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
        .word_rd(word_rd),
        .word_wr(word_wr),
        .word_addr(word_addr),
        .word_data(word_data),
        .word_q(word_q),
        .word_busy(word_busy)
    );

    always_ff @(posedge sdram_clk) begin
        if (rst) begin
            word_busy <= 1'b0;
            word_q <= '0;
            pending_is_write <= 1'b0;
            pending_word_addr <= '0;
            pending_word_data <= '0;
            wait_cycles_remaining <= '0;
            word_rd_count <= '0;
            word_wr_count <= '0;
            word_wait_cycle_count <= '0;
        end else begin
            if (word_busy) begin
                if (wait_cycles_remaining != '0) begin
                    wait_cycles_remaining <= wait_cycles_remaining - 1'b1;
                    word_wait_cycle_count <= word_wait_cycle_count + 1'b1;
                end else begin
                    if (pending_is_write) begin
                        if (pending_word_addr < WORD_DEPTH_LIMIT) begin
                            word_mem[int'(pending_word_addr)] <= pending_word_data;
                        end
                    end else begin
                        word_q <= read_word(pending_word_addr);
                    end
                    word_busy <= 1'b0;
                end
            end else if (word_rd || word_wr) begin
                pending_is_write <= word_wr;
                pending_word_addr <= word_addr;
                pending_word_data <= word_data;
                wait_cycles_remaining <= word_wait_cycles_for_request(word_addr, word_wr);
                word_busy <= 1'b1;

                if (word_rd) begin
                    word_rd_count <= word_rd_count + 1'b1;
                end
                if (word_wr) begin
                    word_wr_count <= word_wr_count + 1'b1;
                end
            end
        end
    end

endmodule

`default_nettype wire

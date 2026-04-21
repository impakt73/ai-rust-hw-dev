`default_nettype none

module sdram_controller_test_harness #(
    parameter int unsigned CONTROLLER_CLK_FREQ_HZ = 133_000_000,
    parameter int unsigned CAS_LATENCY = 3,
    parameter int unsigned EXTRA_READ_LATENCY_CYCLES = 0,
    parameter int unsigned INIT_DELAY_US = 10
) (
    input  wire logic        controller_clk,
    input  wire logic        chip_clk,
    input  wire logic        sample_clk,
    input  wire logic        rst,
    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy,
    output logic [12:0]      loaded_mode_reg,
    output logic [7:0]       read_cmd_count,
    output logic [7:0]       write_cmd_count,
    output logic [7:0]       refresh_cmd_count
);

    localparam logic [2:0] CMD_ACT     = 3'b011;
    localparam logic [2:0] CMD_READ    = 3'b101;
    localparam logic [2:0] CMD_WRITE   = 3'b100;
    localparam logic [2:0] CMD_PRECHG  = 3'b010;
    localparam logic [2:0] CMD_AUTOREF = 3'b001;
    localparam logic [2:0] CMD_LMR     = 3'b000;

    localparam int unsigned BANK_COUNT = 4;
    localparam int unsigned ROW_INDEX_BITS = 4;
    localparam int unsigned COL_INDEX_BITS = 6;
    localparam int unsigned HALFWORD_DEPTH = 1 << (2 + ROW_INDEX_BITS + COL_INDEX_BITS);
    localparam int unsigned MAX_READ_PIPE = 8;

    logic        phy_cke;
    logic        phy_cas;
    logic        phy_ras;
    logic        phy_we;
    logic [1:0]  phy_ba;
    logic [12:0] phy_a;
    tri   [15:0] phy_dq;
    logic [1:0]  phy_dqm;

    logic        model_dq_oe;
    logic [15:0] model_dq_out;

    logic [12:0] active_row [0:BANK_COUNT-1];
    logic        active_row_valid [0:BANK_COUNT-1];
    logic [15:0] halfword_mem [0:HALFWORD_DEPTH-1];
    logic [MAX_READ_PIPE-1:0] read_valid_pipe;
    logic [15:0] read_data_pipe [0:MAX_READ_PIPE-1];

    assign phy_dq = model_dq_oe ? model_dq_out : 16'hZZZZ;

    function automatic int unsigned flat_halfword_index(
        input logic [1:0]  bank,
        input logic [12:0] row,
        input logic [9:0]  col
    );
        begin
            flat_halfword_index = int'({
                bank,
                row[ROW_INDEX_BITS-1:0],
                col[COL_INDEX_BITS-1:0]
            });
        end
    endfunction

    function automatic logic [15:0] read_halfword(
        input logic [1:0]  bank,
        input logic [12:0] row,
        input logic [9:0]  col
    );
        begin
            read_halfword = halfword_mem[flat_halfword_index(bank, row, col)];
        end
    endfunction

    function automatic int unsigned read_pipeline_slot(
        input logic [2:0] cas_latency
    );
        begin
            // The attached SDR SDRAM is only expected to run at CL=2 or CL=3.
            // Keep lower values pinned to slot 0 (the next chip-model beat) so
            // the behavioral model still has a deterministic fallback if a test
            // wrapper is misconfigured.
            if (cas_latency <= 1) begin
                read_pipeline_slot = 0;
            end else begin
                read_pipeline_slot = int'(cas_latency) - 1;
            end
        end
    endfunction

    initial begin
        for (int unsigned idx = 0; idx < HALFWORD_DEPTH; idx++) begin
            halfword_mem[idx] = 16'h0000;
        end
    end

    sdram_controller #(
        .CONTROLLER_CLK_FREQ_HZ(CONTROLLER_CLK_FREQ_HZ),
        .CAS_LATENCY(CAS_LATENCY),
        .EXTRA_READ_LATENCY_CYCLES(EXTRA_READ_LATENCY_CYCLES),
        .INIT_DELAY_US(INIT_DELAY_US)
    ) dut (
        .controller_clk(controller_clk),
        .sample_clk(sample_clk),
        .rst(rst),
        .phy_cke(phy_cke),
        .phy_cas(phy_cas),
        .phy_ras(phy_ras),
        .phy_we(phy_we),
        .phy_ba(phy_ba),
        .phy_a(phy_a),
        .phy_dq(phy_dq),
        .phy_dqm(phy_dqm),
        .word_rd(word_rd),
        .word_wr(word_wr),
        .word_addr(word_addr),
        .word_data(word_data),
        .word_q(word_q),
        .word_busy(word_busy)
    );

    always_ff @(posedge chip_clk) begin
        if (rst) begin
            for (int unsigned bank = 0; bank < BANK_COUNT; bank++) begin
                active_row[bank] <= '0;
                active_row_valid[bank] <= 1'b0;
            end
            for (int unsigned pipe_idx = 0; pipe_idx < MAX_READ_PIPE; pipe_idx++) begin
                read_valid_pipe[pipe_idx] <= 1'b0;
                read_data_pipe[pipe_idx] <= '0;
            end
            loaded_mode_reg <= 13'b000_0_00_011_0_000;
            read_cmd_count <= '0;
            write_cmd_count <= '0;
            refresh_cmd_count <= '0;
            model_dq_oe <= 1'b0;
            model_dq_out <= '0;
        end else begin
            model_dq_oe <= read_valid_pipe[0];
            model_dq_out <= read_data_pipe[0];

            for (int unsigned pipe_idx = 0; pipe_idx < MAX_READ_PIPE - 1; pipe_idx++) begin
                read_valid_pipe[pipe_idx] <= read_valid_pipe[pipe_idx + 1];
                read_data_pipe[pipe_idx] <= read_data_pipe[pipe_idx + 1];
            end
            read_valid_pipe[MAX_READ_PIPE - 1] <= 1'b0;
            read_data_pipe[MAX_READ_PIPE - 1] <= '0;

            unique case ({phy_ras, phy_cas, phy_we})
                CMD_PRECHG: begin
                    if (phy_a[10]) begin
                        for (int unsigned bank = 0; bank < BANK_COUNT; bank++) begin
                            active_row_valid[bank] <= 1'b0;
                        end
                    end else begin
                        active_row_valid[int'(phy_ba)] <= 1'b0;
                    end
                end

                CMD_ACT: begin
                    active_row[int'(phy_ba)] <= phy_a;
                    active_row_valid[int'(phy_ba)] <= 1'b1;
                end

                CMD_READ: begin
                    if (active_row_valid[int'(phy_ba)]) begin
                        read_valid_pipe[read_pipeline_slot(loaded_mode_reg[6:4])] <= 1'b1;
                        read_data_pipe[read_pipeline_slot(loaded_mode_reg[6:4])] <=
                            read_halfword(phy_ba, active_row[int'(phy_ba)], phy_a[9:0]);
                    end
                    read_cmd_count <= read_cmd_count + 1'b1;
                end

                CMD_WRITE: begin
                    if (active_row_valid[int'(phy_ba)]) begin
                        if (!phy_dqm[0]) begin
                            halfword_mem[flat_halfword_index(phy_ba, active_row[int'(phy_ba)], phy_a[9:0])][7:0]
                                <= phy_dq[7:0];
                        end
                        if (!phy_dqm[1]) begin
                            halfword_mem[flat_halfword_index(phy_ba, active_row[int'(phy_ba)], phy_a[9:0])][15:8]
                                <= phy_dq[15:8];
                        end
                    end
                    write_cmd_count <= write_cmd_count + 1'b1;
                end

                CMD_AUTOREF: begin
                    refresh_cmd_count <= refresh_cmd_count + 1'b1;
                end

                CMD_LMR: begin
                    if (phy_ba == 2'b00) begin
                        loaded_mode_reg <= phy_a;
                    end
                end

                default: begin
                end
            endcase
        end
    end

endmodule

/* verilator lint_off MULTITOP */
module sdram_controller_test_wrapper (
    input  wire logic        controller_clk,
    input  wire logic        chip_clk,
    input  wire logic        sample_clk,
    input  wire logic        rst,
    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy,
    output logic [12:0]      loaded_mode_reg,
    output logic [7:0]       read_cmd_count,
    output logic [7:0]       write_cmd_count,
    output logic [7:0]       refresh_cmd_count
);
    sdram_controller_test_harness #(
        .CONTROLLER_CLK_FREQ_HZ(133_000_000),
        .CAS_LATENCY(3),
        .EXTRA_READ_LATENCY_CYCLES(0),
        .INIT_DELAY_US(10)
    ) dut (.*);
endmodule

module sdram_controller_cas2_test_wrapper (
    input  wire logic        controller_clk,
    input  wire logic        chip_clk,
    input  wire logic        sample_clk,
    input  wire logic        rst,
    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy,
    output logic [12:0]      loaded_mode_reg,
    output logic [7:0]       read_cmd_count,
    output logic [7:0]       write_cmd_count,
    output logic [7:0]       refresh_cmd_count
);
    sdram_controller_test_harness #(
        .CONTROLLER_CLK_FREQ_HZ(133_000_000),
        .CAS_LATENCY(2),
        .EXTRA_READ_LATENCY_CYCLES(0),
        .INIT_DELAY_US(10)
    ) dut (.*);
endmodule

module sdram_controller_extra_latency_test_wrapper (
    input  wire logic        controller_clk,
    input  wire logic        chip_clk,
    input  wire logic        sample_clk,
    input  wire logic        rst,
    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy,
    output logic [12:0]      loaded_mode_reg,
    output logic [7:0]       read_cmd_count,
    output logic [7:0]       write_cmd_count,
    output logic [7:0]       refresh_cmd_count
);
    sdram_controller_test_harness #(
        .CONTROLLER_CLK_FREQ_HZ(133_000_000),
        .CAS_LATENCY(3),
        .EXTRA_READ_LATENCY_CYCLES(2),
        .INIT_DELAY_US(10)
    ) dut (.*);
endmodule

module sdram_controller_100mhz_test_wrapper (
    input  wire logic        controller_clk,
    input  wire logic        chip_clk,
    input  wire logic        sample_clk,
    input  wire logic        rst,
    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy,
    output logic [12:0]      loaded_mode_reg,
    output logic [7:0]       read_cmd_count,
    output logic [7:0]       write_cmd_count,
    output logic [7:0]       refresh_cmd_count
);
    sdram_controller_test_harness #(
        .CONTROLLER_CLK_FREQ_HZ(100_000_000),
        .CAS_LATENCY(3),
        .EXTRA_READ_LATENCY_CYCLES(0),
        .INIT_DELAY_US(10)
    ) dut (.*);
endmodule
/* verilator lint_on MULTITOP */

`default_nettype wire

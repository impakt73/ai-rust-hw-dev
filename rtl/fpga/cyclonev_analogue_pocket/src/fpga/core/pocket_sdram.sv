`default_nettype none

module pocket_sdram #(
    parameter int unsigned CLK_FREQ_HZ = 133_000_000,
    parameter int unsigned INIT_DELAY_US = 200
) (
    input  wire logic        controller_clk,
    input  wire logic        sample_clk,
    input  wire logic        reset_n,

    output wire logic        phy_cke,
    output wire logic        phy_cas,
    output wire logic        phy_ras,
    output wire logic        phy_we,
    output wire logic [1:0]  phy_ba,
    output wire logic [12:0] phy_a,
    inout  wire logic [15:0] phy_dq,
    output wire logic [1:0]  phy_dqm,

    input  wire logic        word_rd,
    input  wire logic        word_wr,
    input  wire logic [23:0] word_addr,
    input  wire logic [31:0] word_data,
    output logic [31:0]      word_q,
    output logic             word_busy
);

    typedef enum logic [4:0] {
        ST_RESET,
        ST_INIT_WAIT,
        ST_INIT_CKE,
        ST_INIT_PRECHARGE_WAIT,
        ST_INIT_REFRESH_0,
        ST_INIT_REFRESH_0_WAIT,
        ST_INIT_REFRESH_1,
        ST_INIT_REFRESH_1_WAIT,
        ST_INIT_MR,
        ST_INIT_MR_WAIT,
        ST_INIT_EMR,
        ST_INIT_EMR_WAIT,
        ST_IDLE,
        ST_REFRESH,
        ST_REFRESH_WAIT,
        ST_ACTIVATE,
        ST_ACTIVATE_WAIT,
        ST_READ_CMD_0,
        ST_READ_CMD_1,
        ST_READ_WAIT,
        ST_READ_RECOVERY,
        ST_WRITE_CMD_0,
        ST_WRITE_CMD_1,
        ST_WRITE_RECOVERY
    } state_t;

    localparam logic [2:0] CMD_NOP     = 3'b111;
    localparam logic [2:0] CMD_ACT     = 3'b011;
    localparam logic [2:0] CMD_READ    = 3'b101;
    localparam logic [2:0] CMD_WRITE   = 3'b100;
    localparam logic [2:0] CMD_PRECHG  = 3'b010;
    localparam logic [2:0] CMD_AUTOREF = 3'b001;
    localparam logic [2:0] CMD_LMR     = 3'b000;

    localparam int unsigned CAS_LATENCY = 3;
    localparam int unsigned READ_CAPTURE_PIPE_WIDTH = CAS_LATENCY;

    function automatic int unsigned max_u(input int unsigned a, input int unsigned b);
        if (a > b) begin
            max_u = a;
        end else begin
            max_u = b;
        end
    endfunction

    function automatic int unsigned ns_to_cycles_ceil(input int unsigned ns);
        longint unsigned numerator;
        begin
            numerator = (longint'(CLK_FREQ_HZ) * longint'(ns)) + 1_000_000_000 - 1;
            ns_to_cycles_ceil = int'(numerator / 1_000_000_000);
        end
    endfunction

    function automatic int unsigned ns_to_cycles_floor(input int unsigned ns);
        begin
            ns_to_cycles_floor = int'((longint'(CLK_FREQ_HZ) * longint'(ns)) / 1_000_000_000);
        end
    endfunction

    function automatic int unsigned us_to_cycles_ceil(input int unsigned us);
        longint unsigned numerator;
        begin
            numerator = (longint'(CLK_FREQ_HZ) * longint'(us)) + 1_000_000 - 1;
            us_to_cycles_ceil = int'(numerator / 1_000_000);
        end
    endfunction

    localparam int unsigned TRP_CYCLES = max_u(2, ns_to_cycles_ceil(18));
    localparam int unsigned TRCD_CYCLES = max_u(2, ns_to_cycles_ceil(18));
    localparam int unsigned TRFC_CYCLES = max_u(2, ns_to_cycles_ceil(80));
    localparam int unsigned TMRD_CYCLES = 2;
    localparam int unsigned TWR_CYCLES = max_u(2, ns_to_cycles_ceil(15));
    localparam int unsigned READ_RECOVERY_CYCLES = max_u(2, TRP_CYCLES + 1);
    localparam int unsigned WRITE_RECOVERY_CYCLES = max_u(4, TWR_CYCLES + TRP_CYCLES + 1);
    localparam int unsigned INIT_DELAY_CYCLES = max_u(2, us_to_cycles_ceil(INIT_DELAY_US));
    localparam int unsigned REFRESH_MAX_CYCLES = max_u(2, ns_to_cycles_floor(7_813));
    localparam int unsigned REFRESH_SLIP_CYCLES =
        max_u(
            TRCD_CYCLES + CAS_LATENCY + READ_RECOVERY_CYCLES + 3,
            TRCD_CYCLES + WRITE_RECOVERY_CYCLES + 3
        );
    localparam int unsigned REFRESH_INTERVAL_CYCLES =
        max_u(2, REFRESH_MAX_CYCLES - REFRESH_SLIP_CYCLES);
    localparam int unsigned TIMER_MAX_CYCLES =
        max_u(
            max_u(INIT_DELAY_CYCLES, TRFC_CYCLES),
            max_u(WRITE_RECOVERY_CYCLES, REFRESH_INTERVAL_CYCLES)
        );
    localparam int unsigned TIMER_WIDTH = (TIMER_MAX_CYCLES <= 1) ? 1 : $clog2(TIMER_MAX_CYCLES);

    localparam logic [12:0] MODE_REG = 13'b000_0_00_011_0_000;
    localparam logic [12:0] EXT_MODE_REG = 13'b00000_010_00_000;

    logic rst;
    state_t                  state;
    logic [TIMER_WIDTH-1:0]  wait_counter;
    logic [TIMER_WIDTH-1:0]  refresh_counter;
    logic                    refresh_pending;
    logic                    req_is_write;
    logic [23:0]             req_word_addr;
    logic [31:0]             req_word_data;
    logic                    req_pending;
    logic [9:0]              req_col_halfword;
    logic [9:0]              req_col_halfword_next;
    logic [12:0]             req_row;
    logic [1:0]              req_bank;
    logic [READ_CAPTURE_PIPE_WIDTH-1:0] read_capture_valid_pipe;
    logic [READ_CAPTURE_PIPE_WIDTH-1:0] read_capture_low_pipe;
    logic [READ_CAPTURE_PIPE_WIDTH-1:0] read_capture_last_pipe;

    (* useioff = 1 *) logic [2:0]  phy_cmd_reg;
    (* useioff = 1 *) logic        phy_cke_reg;
    (* useioff = 1 *) logic [1:0]  phy_ba_reg;
    (* useioff = 1 *) logic [12:0] phy_a_reg;
    (* useioff = 1 *) logic [1:0]  phy_dqm_reg;
    (* useioff = 1 *) logic        phy_dq_oe_reg;
    (* useioff = 1 *) logic [15:0] phy_dq_out_reg;
    (* useioff = 1 *) logic [15:0] phy_dq_sampled;

    assign rst = !reset_n;
    assign phy_cke = phy_cke_reg;
    assign {phy_ras, phy_cas, phy_we} = phy_cmd_reg;
    assign phy_ba = phy_ba_reg;
    assign phy_a = phy_a_reg;
    assign phy_dqm = phy_dqm_reg;
    assign phy_dq = phy_dq_oe_reg ? phy_dq_out_reg : 16'hZZZZ;

    always_comb begin
        req_bank = req_word_addr[23:22];
        req_row = req_word_addr[21:9];
        req_col_halfword = {req_word_addr[8:0], 1'b0};
        req_col_halfword_next = req_col_halfword + 10'd1;
    end

    always_ff @(posedge sample_clk) begin
        phy_dq_sampled <= phy_dq;
    end

    always_ff @(posedge controller_clk) begin
        if (rst) begin
            state <= ST_RESET;
            wait_counter <= '0;
            refresh_counter <= '0;
            refresh_pending <= 1'b0;
            req_is_write <= 1'b0;
            req_word_addr <= '0;
            req_word_data <= '0;
            req_pending <= 1'b0;
            read_capture_valid_pipe <= '0;
            read_capture_low_pipe <= '0;
            read_capture_last_pipe <= '0;
            phy_cmd_reg <= CMD_NOP;
            phy_cke_reg <= 1'b0;
            phy_ba_reg <= 2'b00;
            phy_a_reg <= 13'h0000;
            phy_dqm_reg <= 2'b00;
            phy_dq_oe_reg <= 1'b0;
            phy_dq_out_reg <= 16'h0000;
            word_q <= 32'h0000_0000;
            word_busy <= 1'b1;
        end else begin
            phy_cmd_reg <= CMD_NOP;
            phy_dq_oe_reg <= 1'b0;
            phy_dqm_reg <= 2'b00;

            read_capture_valid_pipe <= {
                read_capture_valid_pipe[READ_CAPTURE_PIPE_WIDTH-2:0],
                1'b0
            };
            read_capture_low_pipe <= {
                read_capture_low_pipe[READ_CAPTURE_PIPE_WIDTH-2:0],
                1'b0
            };
            read_capture_last_pipe <= {
                read_capture_last_pipe[READ_CAPTURE_PIPE_WIDTH-2:0],
                1'b0
            };

            if (read_capture_valid_pipe[READ_CAPTURE_PIPE_WIDTH-1]) begin
                if (read_capture_low_pipe[READ_CAPTURE_PIPE_WIDTH-1]) begin
                    word_q[15:0] <= phy_dq_sampled;
                end else begin
                    word_q[31:16] <= phy_dq_sampled;
                end
            end

            if (refresh_counter == TIMER_WIDTH'(REFRESH_INTERVAL_CYCLES - 1)) begin
                refresh_counter <= '0;
                refresh_pending <= 1'b1;
            end else begin
                refresh_counter <= refresh_counter + 1'b1;
            end

            if (!req_pending && (word_rd || word_wr)) begin
                req_is_write <= word_wr;
                req_word_addr <= word_addr;
                req_word_data <= word_data;
                req_pending <= 1'b1;
            end

            case (state)
                ST_RESET: begin
                    phy_cke_reg <= 1'b0;
                    word_busy <= 1'b1;
                    wait_counter <= TIMER_WIDTH'(INIT_DELAY_CYCLES - 1);
                    state <= ST_INIT_WAIT;
                end

                ST_INIT_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        phy_cke_reg <= 1'b1;
                        wait_counter <= '0;
                        state <= ST_INIT_CKE;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_CKE: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        phy_cmd_reg <= CMD_PRECHG;
                        phy_a_reg <= 13'b001_0000_0000_000;
                        wait_counter <= TIMER_WIDTH'(TRP_CYCLES - 1);
                        state <= ST_INIT_PRECHARGE_WAIT;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_PRECHARGE_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        state <= ST_INIT_REFRESH_0;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_REFRESH_0: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_AUTOREF;
                    wait_counter <= TIMER_WIDTH'(TRFC_CYCLES - 1);
                    state <= ST_INIT_REFRESH_0_WAIT;
                end

                ST_INIT_REFRESH_0_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        state <= ST_INIT_REFRESH_1;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_REFRESH_1: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_AUTOREF;
                    wait_counter <= TIMER_WIDTH'(TRFC_CYCLES - 1);
                    state <= ST_INIT_REFRESH_1_WAIT;
                end

                ST_INIT_REFRESH_1_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        state <= ST_INIT_MR;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_MR: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_LMR;
                    phy_ba_reg <= 2'b00;
                    phy_a_reg <= MODE_REG;
                    wait_counter <= TIMER_WIDTH'(TMRD_CYCLES - 1);
                    state <= ST_INIT_MR_WAIT;
                end

                ST_INIT_MR_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        state <= ST_INIT_EMR;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_INIT_EMR: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_LMR;
                    phy_ba_reg <= 2'b10;
                    phy_a_reg <= EXT_MODE_REG;
                    wait_counter <= TIMER_WIDTH'(TMRD_CYCLES - 1);
                    state <= ST_INIT_EMR_WAIT;
                end

                ST_INIT_EMR_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        refresh_pending <= 1'b0;
                        word_busy <= 1'b0;
                        state <= ST_IDLE;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_IDLE: begin
                    word_busy <= req_pending;
                    if (refresh_pending) begin
                        word_busy <= 1'b1;
                        state <= ST_REFRESH;
                    end else if (req_pending) begin
                        req_pending <= 1'b0;
                        word_busy <= 1'b1;
                        state <= ST_ACTIVATE;
                    end
                end

                ST_REFRESH: begin
                    phy_cmd_reg <= CMD_AUTOREF;
                    refresh_pending <= 1'b0;
                    wait_counter <= TIMER_WIDTH'(TRFC_CYCLES - 1);
                    state <= ST_REFRESH_WAIT;
                end

                ST_REFRESH_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        word_busy <= 1'b0;
                        state <= ST_IDLE;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_ACTIVATE: begin
                    phy_cmd_reg <= CMD_ACT;
                    phy_ba_reg <= req_bank;
                    phy_a_reg <= req_row;
                    wait_counter <= TIMER_WIDTH'(TRCD_CYCLES - 1);
                    state <= ST_ACTIVATE_WAIT;
                end

                ST_ACTIVATE_WAIT: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        if (req_is_write) begin
                            state <= ST_WRITE_CMD_0;
                        end else begin
                            state <= ST_READ_CMD_0;
                        end
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_READ_CMD_0: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_READ;
                    phy_ba_reg <= req_bank;
                    phy_a_reg <= {2'b00, 1'b0, req_col_halfword};
                    read_capture_valid_pipe[0] <= 1'b1;
                    read_capture_low_pipe[0] <= 1'b0;
                    read_capture_last_pipe[0] <= 1'b0;
                    state <= ST_READ_CMD_1;
                end

                ST_READ_CMD_1: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_READ;
                    phy_ba_reg <= req_bank;
                    phy_a_reg <= {2'b00, 1'b1, req_col_halfword_next};
                    read_capture_valid_pipe[0] <= 1'b1;
                    read_capture_low_pipe[0] <= 1'b1;
                    read_capture_last_pipe[0] <= 1'b1;
                    state <= ST_READ_WAIT;
                end

                ST_READ_WAIT: begin
                    word_busy <= 1'b1;
                    if (read_capture_valid_pipe[READ_CAPTURE_PIPE_WIDTH-1]
                        && read_capture_last_pipe[READ_CAPTURE_PIPE_WIDTH-1]) begin
                        wait_counter <= TIMER_WIDTH'(READ_RECOVERY_CYCLES - 1);
                        state <= ST_READ_RECOVERY;
                    end
                end

                ST_READ_RECOVERY: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        word_busy <= 1'b0;
                        state <= ST_IDLE;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                ST_WRITE_CMD_0: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_WRITE;
                    phy_ba_reg <= req_bank;
                    phy_a_reg <= {2'b00, 1'b0, req_col_halfword};
                    phy_dq_oe_reg <= 1'b1;
                    phy_dq_out_reg <= req_word_data[31:16];
                    state <= ST_WRITE_CMD_1;
                end

                ST_WRITE_CMD_1: begin
                    word_busy <= 1'b1;
                    phy_cmd_reg <= CMD_WRITE;
                    phy_ba_reg <= req_bank;
                    phy_a_reg <= {2'b00, 1'b1, req_col_halfword_next};
                    phy_dq_oe_reg <= 1'b1;
                    phy_dq_out_reg <= req_word_data[15:0];
                    wait_counter <= TIMER_WIDTH'(WRITE_RECOVERY_CYCLES - 1);
                    state <= ST_WRITE_RECOVERY;
                end

                ST_WRITE_RECOVERY: begin
                    word_busy <= 1'b1;
                    if (wait_counter == '0) begin
                        word_busy <= 1'b0;
                        state <= ST_IDLE;
                    end else begin
                        wait_counter <= wait_counter - 1'b1;
                    end
                end

                default: begin
                    word_busy <= 1'b1;
                    state <= ST_RESET;
                end
            endcase
        end
    end

endmodule

`default_nettype wire

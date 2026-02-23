`timescale 1ns/1ps

module phase_accumulator_tb;
    localparam int unsigned PHASE_WIDTH = 16;
    localparam longint unsigned CLK_FREQ_HZ = 100;
    localparam longint unsigned TICK_FREQ_HZ = 33;
    localparam int unsigned TEST_CYCLES = 600;

    localparam longint unsigned PHASE_MODULUS = 64'd1 << PHASE_WIDTH;
    localparam longint unsigned PHASE_INCREMENT =
        ((TICK_FREQ_HZ * PHASE_MODULUS) + (CLK_FREQ_HZ / 2)) / CLK_FREQ_HZ;
    localparam longint unsigned EXPECTED_TICKS =
        (TEST_CYCLES * PHASE_INCREMENT) >> PHASE_WIDTH;

    logic clk = 1'b0;
    logic rst_n = 1'b0;
    logic tick;

    logic [PHASE_WIDTH-1:0] ref_phase;
    int unsigned tick_count;

    phase_accumulator #(
        .PHASE_WIDTH(PHASE_WIDTH),
        .CLK_FREQ_HZ(CLK_FREQ_HZ),
        .TICK_FREQ_HZ(TICK_FREQ_HZ)
    ) dut (
        .clk(clk),
        .rst_n(rst_n),
        .tick(tick)
    );

    // verilator lint_off BLKSEQ
    always #5 clk = ~clk;
    // verilator lint_on BLKSEQ

    initial begin
        ref_phase = '0;
        tick_count = 0;

        repeat (3) @(posedge clk);
        rst_n = 1'b1;

        for (int i = 0; i < TEST_CYCLES; i++) begin
            logic [PHASE_WIDTH:0] ref_sum;
            @(posedge clk);
            ref_sum = {1'b0, ref_phase} + {1'b0, PHASE_INCREMENT[PHASE_WIDTH-1:0]};

            if (tick !== ref_sum[PHASE_WIDTH]) begin
                $error("tick mismatch at cycle %0d: got=%0b expected=%0b", i, tick, ref_sum[PHASE_WIDTH]);
                $fatal(1);
            end

            ref_phase = ref_sum[PHASE_WIDTH-1:0];
            if (tick) begin
                tick_count++;
            end
        end

        if (longint'(tick_count) != EXPECTED_TICKS) begin
            $error("tick count mismatch: got=%0d expected=%0d", tick_count, EXPECTED_TICKS);
            $fatal(1);
        end

        $display("PASS: phase_accumulator generated %0d ticks over %0d cycles", tick_count, TEST_CYCLES);
        $finish;
    end
endmodule

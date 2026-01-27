// FPGA Top Wrapper (without FPU) for Alchitry Cu v1
// Temporary simplified version for synthesis testing
// Uses counter-based LED blinker instead of full CPU

module fpga_top_simple (
    input  logic       clk,
    input  logic       rst_n_btn,
    output logic [7:0] led
);

    logic rst_n;
    
    // Synchronize reset button (2-FF synchronizer)
    logic rst_n_sync1, rst_n_sync2;
    always_ff @(posedge clk) begin
        rst_n_sync1 <= rst_n_btn;
        rst_n_sync2 <= rst_n_sync1;
    end
    assign rst_n = rst_n_sync2;
    
    // Simple LED counter for testing
    logic [31:0] counter;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            counter <= 32'h0;
            led <= 8'h00;
        end else begin
            counter <= counter + 1;
            // Blink pattern at ~1.5Hz (100MHz / 2^26 ≈ 1.5Hz)
            led <= counter[29:22];
        end
    end

endmodule
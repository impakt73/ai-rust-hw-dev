// Flip-Flop Synchronizer
// Synchronizes asynchronous signals into the local clock domain.
//
// Parameters:
//   STAGES - Number of synchronization stages (default: 3)
//   WIDTH  - Signal width in bits (default: 1)
//
// Interface:
//   clk   - Destination clock domain
//   rst_n - Asynchronous active-low reset
//   din   - Asynchronous input signal
//   dout  - Synchronized output signal

module ff_sync #(
    parameter int STAGES = 3,
    parameter int WIDTH  = 1,
    parameter logic [WIDTH-1:0] RESET_VALUE = '0
) (
    input  logic             clk,
    input  logic             rst_n,
    input  logic [WIDTH-1:0] din,
    output logic [WIDTH-1:0] dout
);

    logic [WIDTH-1:0] sync_regs [0:STAGES-1];

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            for (int i = 0; i < STAGES; i++) begin
                sync_regs[i] <= RESET_VALUE;
            end
        end else begin
            sync_regs[0] <= din;
            for (int i = 1; i < STAGES; i++) begin
                sync_regs[i] <= sync_regs[i-1];
            end
        end
    end

    assign dout = sync_regs[STAGES-1];

endmodule

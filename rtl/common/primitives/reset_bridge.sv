`default_nettype none
// Reset bridge with immediate asynchronous assertion and synchronous release.
//
// Parameters:
//   STAGES - Number of synchronizer stages used to release reset (default: 3)
//
// Interface:
//   clk   - Destination clock domain
//   rst_n - Asynchronous active-low reset input
//   rst   - Active-high reset output with immediate assert and sync release

module reset_bridge #(
    parameter int STAGES = 3
) (
    input wire logic clk,
    input wire logic rst_n,
    output logic     rst
);

    localparam int EFFECTIVE_STAGES = (STAGES >= 1) ? STAGES : 1;

    initial begin
        if (STAGES < 1) begin
            $fatal(1, "reset_bridge: STAGES must be >= 1, got %0d", STAGES);
        end
    end

    // Vivado
    (* IOB = "false" *)
    (* ASYNC_REG = "TRUE" *)
    (* SHREG_EXTRACT = "NO" *)
    // Quartus
    (* useioff = 0 *)
    (* PRESERVE *)
    (* altera_attribute = "-name SYNCHRONIZER_IDENTIFICATION \"FORCED IF ASYNCHRONOUS\"" *)
    logic reset_bridge_sync_regs [0:EFFECTIVE_STAGES-1] = '{default: 1'b1};

    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            for (int i = 0; i < EFFECTIVE_STAGES; i++) begin
                reset_bridge_sync_regs[i] <= 1'b1;
            end
        end else begin
            reset_bridge_sync_regs[0] <= 1'b0;
            for (int i = 1; i < EFFECTIVE_STAGES; i++) begin
                reset_bridge_sync_regs[i] <= reset_bridge_sync_regs[i-1];
            end
        end
    end

    assign rst = reset_bridge_sync_regs[EFFECTIVE_STAGES-1];

endmodule
`default_nettype wire

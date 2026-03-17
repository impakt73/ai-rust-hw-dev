`default_nettype none

module analogue_pocket_repo_top #(
    parameter bit ENABLE_M_EXT = 1'b1,
    parameter bit ENABLE_F_EXT = 1'b0
) (
    input  wire logic       clk,
    input  wire logic       reset_n,
    output logic [7:0]      led_out,
    output logic [7:0]      sys_led_out,
    output logic            halted,
    output logic            instr_complete,
    output logic            rst_out,
    output logic            cpu_booting,
    output logic [31:0]     halted_value
);
    logic rst;
    logic [7:0] host_tx_data_unused;
    logic       host_tx_valid_unused;
    logic       host_rx_ready_unused;
    logic [31:0] debug_rs1_data_unused;
    logic [31:0] debug_rs2_data_unused;
    logic [31:0] debug_rd_data_unused;
    logic [31:0] debug_pc_unused;
    logic [31:0] debug_instruction_unused;
    logic [31:0] debug_current_pc_unused;
    logic [31:0] debug_current_instruction_unused;
    logic [3:0]  debug_fsm_state_unused;

    always_ff @(posedge clk) begin
        if (!reset_n) begin
            rst <= 1'b1;
        end else begin
            rst <= 1'b0;
        end
    end

    top #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT),
        .CLK_FREQ_HZ(74_250_000),
        .RESET_CYCLES(74_250_000)
    ) repo_top_inst (
        .clk(clk),
        .rst(rst),
        .host_tx_data(host_tx_data_unused),
        .host_tx_valid(host_tx_valid_unused),
        .host_tx_ready(1'b1),
        .host_rx_data(8'h00),
        .host_rx_valid(1'b0),
        .host_rx_ready(host_rx_ready_unused),
        .com_err(1'b0),
        .led_out(led_out),
        .sys_led_out(sys_led_out),
        .halted(halted),
        .instr_complete(instr_complete),
        .debug_rs1_data(debug_rs1_data_unused),
        .debug_rs2_data(debug_rs2_data_unused),
        .debug_rd_data(debug_rd_data_unused),
        .debug_pc(debug_pc_unused),
        .debug_instruction(debug_instruction_unused),
        .debug_current_pc(debug_current_pc_unused),
        .debug_current_instruction(debug_current_instruction_unused),
        .debug_fsm_state(debug_fsm_state_unused),
        .rst_out(rst_out),
        .cpu_booting(cpu_booting),
        .halted_value(halted_value)
    );
endmodule

`default_nettype wire

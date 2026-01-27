// Simple FPGA Top Module (without FPU)
// This is a simplified version for FPGA synthesis that excludes the FPU
// due to Yosys 0.33 synthesis limitations

module top_no_fpu (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory interface
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    output logic        imem_req,
    input  logic        imem_ready,
    
    // Data memory interface
    output logic [31:0] dmem_addr,
    output logic [31:0] dmem_wdata,
    input  logic [31:0] dmem_rdata,
    output logic        dmem_we,
    output logic        dmem_re,
    output logic [1:0]  dmem_size,
    output logic        dmem_req,
    input  logic        dmem_ready,
    
    // System control
    output logic        halted,
    output logic        instr_complete,
    
    // Debug outputs (simplified)
    output logic [31:0] debug_pc,
    output logic [3:0]  debug_fsm_state
);

    // Simple counter-based LED blinker
    // This is a placeholder until proper CPU integration
    logic [31:0] counter;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            counter <= 32'h0;
            imem_addr <= boot_addr;
            imem_req <= 1'b0;
            dmem_addr <= 32'h0;
            dmem_wdata <= 32'h0;
            dmem_we <= 1'b0;
            dmem_re <= 1'b0;
            dmem_size <= 2'b00;
            dmem_req <= 1'b0;
            halted <= 1'b0;
            instr_complete <= 1'b0;
            debug_pc <= 32'h0;
            debug_fsm_state <= 4'h0;
        end else begin
            counter <= counter + 1;
            // Simple memory test pattern
            if (counter[20] == 1'b1) begin
                dmem_addr <= 32'h50000000; // LED address
                dmem_wdata <= counter[27:20];
                dmem_we <= 1'b1;
                dmem_req <= 1'b1;
            end else begin
                dmem_we <= 1'b0;
                dmem_req <= 1'b0;
            end
        end
    end

endmodule
// Bus Bridge
// Converts CPU A/D memory channels into the legacy unified memory interface.

module bus_bridge (
    input  logic        clk,
    input  logic        rst_n,
    
    // Address channel input (from CPU)
    input  logic [31:0] mem_a_addr,
    input  logic [31:0] mem_a_wdata,
    input  logic        mem_a_we,
    input  logic [1:0]  mem_a_size,
    input  logic        mem_a_valid,
    output logic        mem_a_ready,
    
    // Data channel output (to CPU)
    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input  logic        mem_d_ready,
    
    // Legacy unified memory interface output
    output logic [31:0] mem_addr,
    output logic [31:0] mem_wdata,
    input  logic [31:0] mem_rdata,
    output logic        mem_we,
    output logic [1:0]  mem_size,
    output logic        mem_req,
    input  logic        mem_ready
);

    logic [31:0] pending_req_addr;
    logic [31:0] pending_req_wdata;
    logic        pending_req_we;
    logic [1:0]  pending_req_size;
    logic        pending_req_valid;
    
    logic [31:0] pending_resp_rdata;
    logic        pending_resp_valid;
    
    logic a_handshake;
    logic d_handshake;
    assign a_handshake = mem_a_valid && mem_a_ready;
    assign d_handshake = mem_d_valid && mem_d_ready;
    
    // Accept one request at a time, and only when there is no pending response.
    assign mem_a_ready = !pending_req_valid && !pending_resp_valid;
    
    // Drive legacy request bus from pending request registers.
    assign mem_addr  = pending_req_addr;
    assign mem_wdata = pending_req_wdata;
    assign mem_we    = pending_req_we;
    assign mem_size  = pending_req_size;
    assign mem_req   = pending_req_valid;
    
    // Drive data channel from pending response registers.
    assign mem_d_rdata = pending_resp_rdata;
    assign mem_d_valid = pending_resp_valid;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            pending_req_addr   <= 32'h0;
            pending_req_wdata  <= 32'h0;
            pending_req_we     <= 1'b0;
            pending_req_size   <= 2'b00;
            pending_req_valid  <= 1'b0;
            pending_resp_rdata <= 32'h0;
            pending_resp_valid <= 1'b0;
        end else begin
            if (a_handshake) begin
                pending_req_addr  <= mem_a_addr;
                pending_req_wdata <= mem_a_wdata;
                pending_req_we    <= mem_a_we;
                pending_req_size  <= mem_a_size;
                pending_req_valid <= 1'b1;
            end
            
            if (pending_req_valid && mem_ready) begin
                pending_req_valid  <= 1'b0;
                pending_resp_rdata <= mem_rdata;
                pending_resp_valid <= 1'b1;
            end
            
            if (d_handshake) begin
                pending_resp_valid <= 1'b0;
            end
        end
    end

endmodule

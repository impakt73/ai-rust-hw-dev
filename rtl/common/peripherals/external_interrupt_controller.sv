`default_nettype none
// External interrupt controller (PLIC-lite)
// - Machine-mode-only target
// - Fixed priority: lowest source ID wins
// - Pending bits latch edge/pulse events until software completion or explicit clear

module external_interrupt_controller #(
    parameter int unsigned NUM_SOURCES = 4
) (
    input  wire logic                   clk,
    input  wire logic                   rst,
    input  wire logic [NUM_SOURCES-1:0] irq_sources,

    // Address channel (A)
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,

    // Data channel (D)
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready,

    // CPU interrupt output
    output logic             meip
);

    localparam int unsigned SOURCE_ID_WIDTH = (NUM_SOURCES <= 1) ? 1 : $clog2(NUM_SOURCES + 1);
    localparam logic [4:0] REG_RAW_STATUS   = 5'h00;
    localparam logic [4:0] REG_PENDING      = 5'h04;
    localparam logic [4:0] REG_ENABLE       = 5'h08;
    localparam logic [4:0] REG_CLAIM        = 5'h0C;
    localparam logic [4:0] REG_COMPLETE     = 5'h10;
    localparam logic [4:0] REG_PENDING_SET  = 5'h14;
    localparam logic [4:0] REG_PENDING_CLR  = 5'h18;
    localparam logic [4:0] REG_SOURCE_COUNT = 5'h1C;

    logic [NUM_SOURCES-1:0] pending_reg;
    logic [NUM_SOURCES-1:0] enable_reg;
    logic [NUM_SOURCES-1:0] pending_next;
    logic [NUM_SOURCES-1:0] enable_next;
    logic [NUM_SOURCES-1:0] enabled_pending;
    logic [NUM_SOURCES-1:0] pending_set_mask;
    logic [NUM_SOURCES-1:0] pending_clear_mask;
    logic [NUM_SOURCES-1:0] completion_clear_mask;
    logic [31:0]            response_data;
    logic                   response_pending;
    logic                   mem_a_handshake;
    logic                   mem_d_handshake;
    logic                   word_access;
    logic [SOURCE_ID_WIDTH-1:0] claim_id;

    function automatic logic [NUM_SOURCES-1:0] decode_source_id(input logic [31:0] source_id);
        logic [NUM_SOURCES-1:0] decoded;
        integer idx;
        begin
            decoded = '0;
            for (idx = 0; idx < NUM_SOURCES; idx = idx + 1) begin
                if (source_id == (idx + 1))
                    decoded[idx] = 1'b1;
            end
            decode_source_id = decoded;
        end
    endfunction

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    assign word_access = (mem_a_size == 2'b10) && (mem_a_addr[1:0] == 2'b00);
    assign mem_a_ready = !response_pending;
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;
    assign enabled_pending = pending_reg & enable_reg;

    always_comb begin
        claim_id = '0;

        for (int unsigned idx = 0; idx < NUM_SOURCES; idx++) begin
            if (enabled_pending[idx] && (claim_id == '0))
                claim_id = SOURCE_ID_WIDTH'(idx + 1);
        end
    end

    always_comb begin
        pending_set_mask = '0;
        pending_clear_mask = '0;
        completion_clear_mask = '0;
        enable_next = enable_reg;

        if (mem_a_handshake && mem_a_we && word_access) begin
            case (mem_a_addr[4:0])
                REG_ENABLE:      enable_next = mem_a_wdata[NUM_SOURCES-1:0];
                REG_COMPLETE:    completion_clear_mask = decode_source_id(mem_a_wdata);
                REG_PENDING_SET: pending_set_mask = mem_a_wdata[NUM_SOURCES-1:0];
                REG_PENDING_CLR: pending_clear_mask = mem_a_wdata[NUM_SOURCES-1:0];
                default: ;
            endcase
        end

        pending_next =
            (pending_reg & ~(pending_clear_mask | completion_clear_mask)) |
            irq_sources |
            pending_set_mask;
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            pending_reg <= '0;
            enable_reg <= '0;
            response_pending <= 1'b0;
            meip <= 1'b0;
        end else begin
            pending_reg <= pending_next;
            enable_reg <= enable_next;
            meip <= |(pending_next & enable_next);

            if (mem_d_handshake)
                response_pending <= 1'b0;

            if (mem_a_handshake) begin
                response_data <= 32'h0;
                response_pending <= 1'b1;

                if (word_access) begin
                    case (mem_a_addr[4:0])
                        REG_RAW_STATUS:
                            if (!mem_a_we)
                                response_data <= {{(32-NUM_SOURCES){1'b0}}, irq_sources};
                        REG_PENDING:
                            if (!mem_a_we)
                                response_data <= {{(32-NUM_SOURCES){1'b0}}, pending_reg};
                        REG_ENABLE:
                            if (mem_a_we)
                                response_data <= 32'h0;
                            else
                                response_data <= {{(32-NUM_SOURCES){1'b0}}, enable_reg};
                        REG_CLAIM:
                            if (!mem_a_we)
                                response_data <= {{(32-SOURCE_ID_WIDTH){1'b0}}, claim_id};
                        REG_COMPLETE:
                            response_data <= 32'h0;
                        REG_PENDING_SET:
                            response_data <= 32'h0;
                        REG_PENDING_CLR:
                            response_data <= 32'h0;
                        REG_SOURCE_COUNT:
                            if (!mem_a_we)
                                response_data <= 32'(NUM_SOURCES);
                        default:
                            response_data <= 32'h0;
                    endcase
                end
            end
        end
    end

endmodule

`default_nettype wire

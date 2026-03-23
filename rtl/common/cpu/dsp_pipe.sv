`default_nettype none

module dsp_pipe (
    input  wire logic        clk,
    input  wire logic        rst,
    input  wire logic [31:0] a,
    input  wire logic [31:0] b,
    input  wire logic [4:0]  alu_op,
    input  wire logic        in_valid,
    output logic [31:0]      out_data,
    output logic             out_valid
);

    // ALU operation encodings kept aligned with rtl/common/cpu/alu.sv so this
    // pipe can be dropped into the existing ALU datapath later.
    localparam logic [4:0] ALU_ADD    = 5'b00000;
    localparam logic [4:0] ALU_SUB    = 5'b00001;
    localparam logic [4:0] ALU_AND    = 5'b00010;
    localparam logic [4:0] ALU_OR     = 5'b00011;
    localparam logic [4:0] ALU_XOR    = 5'b00100;
    localparam logic [4:0] ALU_SLL    = 5'b00101;
    localparam logic [4:0] ALU_SRL    = 5'b00110;
    localparam logic [4:0] ALU_SRA    = 5'b00111;
    localparam logic [4:0] ALU_SLT    = 5'b01000;
    localparam logic [4:0] ALU_SLTU   = 5'b01001;
    localparam logic [4:0] ALU_MUL    = 5'b01010;
    localparam logic [4:0] ALU_MULH   = 5'b01011;
    localparam logic [4:0] ALU_MULHSU = 5'b01100;
    localparam logic [4:0] ALU_MULHU  = 5'b01101;

    logic [31:0] stage1_a_reg;
    logic [31:0] stage1_b_reg;
    logic [4:0]  stage1_op_reg;
    logic        stage1_valid_reg;

    logic [31:0] stage2_result_reg;
    logic        stage2_valid_reg;

    logic [31:0] stage2_result_next;
    logic        stage1_signed_lt;
    logic        stage1_unsigned_lt;
    (* use_dsp = "yes" *) logic signed [63:0] mul_signed_signed;
    (* use_dsp = "yes" *) logic signed [63:0] mul_signed_unsigned;
    (* use_dsp = "yes" *) logic        [63:0] mul_unsigned_unsigned;

    assign stage1_signed_lt = $signed(stage1_a_reg) < $signed(stage1_b_reg);
    assign stage1_unsigned_lt = stage1_a_reg < stage1_b_reg;
    assign mul_signed_signed = $signed({{32{stage1_a_reg[31]}}, stage1_a_reg}) *
                               $signed({{32{stage1_b_reg[31]}}, stage1_b_reg});
    assign mul_signed_unsigned = $signed({{32{stage1_a_reg[31]}}, stage1_a_reg}) *
                                 $signed({32'd0, stage1_b_reg});
    assign mul_unsigned_unsigned = {32'd0, stage1_a_reg} * {32'd0, stage1_b_reg};

    always_comb begin
        stage2_result_next = 32'd0;

        case (stage1_op_reg)
            ALU_ADD:    stage2_result_next = stage1_a_reg + stage1_b_reg;
            ALU_SUB:    stage2_result_next = stage1_a_reg - stage1_b_reg;
            ALU_AND:    stage2_result_next = stage1_a_reg & stage1_b_reg;
            ALU_OR:     stage2_result_next = stage1_a_reg | stage1_b_reg;
            ALU_XOR:    stage2_result_next = stage1_a_reg ^ stage1_b_reg;
            ALU_SLL:    stage2_result_next = stage1_a_reg << stage1_b_reg[4:0];
            ALU_SRL:    stage2_result_next = stage1_a_reg >> stage1_b_reg[4:0];
            ALU_SRA:    stage2_result_next = $signed(stage1_a_reg) >>> stage1_b_reg[4:0];
            ALU_SLT:    stage2_result_next = {31'd0, stage1_signed_lt};
            ALU_SLTU:   stage2_result_next = {31'd0, stage1_unsigned_lt};
            ALU_MUL:    stage2_result_next = mul_signed_signed[31:0];
            ALU_MULH:   stage2_result_next = mul_signed_signed[63:32];
            ALU_MULHSU: stage2_result_next = mul_signed_unsigned[63:32];
            ALU_MULHU:  stage2_result_next = mul_unsigned_unsigned[63:32];
            default:    stage2_result_next = 32'd0;
        endcase
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            stage1_valid_reg <= 1'b0;
            stage2_valid_reg <= 1'b0;
            out_valid        <= 1'b0;
        end else begin
            out_valid <= stage2_valid_reg;
            out_data  <= stage2_result_reg;

            stage2_valid_reg <= stage1_valid_reg;
            stage2_result_reg <= stage2_result_next;

            stage1_valid_reg <= in_valid;
            stage1_a_reg     <= a;
            stage1_b_reg     <= b;
            stage1_op_reg    <= alu_op;
        end
    end

endmodule

`default_nettype wire

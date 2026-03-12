`default_nettype none
// Synthesis Test Harness for Module Resource Analysis
// This module wraps individual RTL modules to prevent optimization
// during synthesis, allowing accurate resource measurement.
//
// The harness is configured via `define statements to select which
// module to synthesize.

module synth_harness (
    input  logic        clk,
    input  logic        rst_n_btn,
    output logic [7:0]  led
);

    // Reset synchronizer
    logic rst_n_sync1, rst_n_sync2;
    logic rst_n;
    
    always_ff @(posedge clk) begin
        rst_n_sync1 <= rst_n_btn;
        rst_n_sync2 <= rst_n_sync1;
    end
    assign rst_n = rst_n_sync2;

    // Input stimulus registers to prevent input optimization
    logic [31:0] stim_reg [0:7];
    logic [4:0]  stim_sel;
    
    // Register the inputs to prevent constant propagation
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            for (int i = 0; i < 8; i++) begin
                stim_reg[i] <= 32'h0;
            end
            stim_sel <= 5'h0;
        end else begin
            // Rotate through values to prevent optimization
            stim_reg[0] <= stim_reg[7] ^ 32'h12345678;
            for (int i = 1; i < 8; i++) begin
                stim_reg[i] <= stim_reg[i-1];
            end
            stim_sel <= stim_sel + 1'b1;
        end
    end
    
    // Output result register
    logic [31:0] result_reg;
    
`ifdef SYNTH_ALU
    // ============================================================
    // ALU Module Test
    // ============================================================
    logic [31:0] alu_out_data;
    logic alu_in_ready, alu_out_valid;
    logic alu_in_valid;
    
    assign alu_in_valid = stim_sel[4];
    
    alu u_alu (
        .clk(clk),
        .rst_n(rst_n),
        .a(stim_reg[0]),
        .b(stim_reg[1]),
        .alu_op(stim_sel[4:0]),
        .in_valid(alu_in_valid),
        .in_ready(alu_in_ready),
        .out_data(alu_out_data),
        .out_valid(alu_out_valid)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= alu_out_data ^ {31'h0, alu_in_ready} ^ {31'h0, alu_out_valid};
    end

`elsif SYNTH_BRANCH_UNIT
    // ============================================================
    // Branch Unit Test
    // ============================================================
    logic take_branch;
    
    branch_unit u_branch (
        .branch(stim_sel[0]),
        .funct3(stim_sel[3:1]),
        .rs1_data(stim_reg[0]),
        .rs2_data(stim_reg[1]),
        .alu_zero(stim_sel[4]),
        .take_branch(take_branch)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= {31'h0, take_branch};
    end

`elsif SYNTH_CSR_FILE
    // ============================================================
    // CSR File Test
    // ============================================================
    logic [31:0] csr_rdata;
    
    csr_file u_csr (
        .clk(clk),
        .rst_n(rst_n),
        .is_csr(stim_sel[0]),
        .instr_complete(stim_sel[1]),
        .funct3(stim_sel[3:1]),
        .rs1(stim_sel[4:0]),
        .csr_addr(stim_reg[0][11:0]),
        .rs1_data(stim_reg[1]),
        .fcsr(stim_reg[2]),
        .csr_rdata(csr_rdata)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= csr_rdata;
    end

`elsif SYNTH_DECODER
    // ============================================================
    // Decoder Test
    // ============================================================
    logic [6:0] opcode;
    logic [4:0] rd, rs1, rs2;
    logic [2:0] funct3;
    logic [6:0] funct7;
    logic [31:0] imm_i, imm_s, imm_b, imm_u, imm_j;
    logic [4:0] alu_op;
    logic alu_src, reg_write, mem_write, mem_read, mem_to_reg;
    logic branch, jump, is_ecall, is_ebreak, is_fence, is_csr, is_auipc;
    logic is_lr, is_sc, is_amo;
    logic [4:0] funct5, fpu_op;
    logic fp_reg_write, fp_to_int, int_to_fp, is_fp_load, is_fp_store;
    logic instruction_valid;
    
    decoder u_decoder (
        .clk(clk),
        .rst_n(rst_n),
        .decode_en(stim_sel[0]),
        .instruction(stim_reg[0]),
        .opcode(opcode),
        .rd(rd),
        .rs1(rs1),
        .rs2(rs2),
        .funct3(funct3),
        .funct7(funct7),
        .imm_i(imm_i),
        .imm_s(imm_s),
        .imm_b(imm_b),
        .imm_u(imm_u),
        .imm_j(imm_j),
        .alu_op(alu_op),
        .alu_src(alu_src),
        .reg_write(reg_write),
        .mem_write(mem_write),
        .mem_read(mem_read),
        .mem_to_reg(mem_to_reg),
        .branch(branch),
        .jump(jump),
        .is_ecall(is_ecall),
        .is_ebreak(is_ebreak),
        .is_fence(is_fence),
        .is_csr(is_csr),
        .is_auipc(is_auipc),
        .is_lr(is_lr),
        .is_sc(is_sc),
        .is_amo(is_amo),
        .funct5(funct5),
        .fpu_op(fpu_op),
        .fp_reg_write(fp_reg_write),
        .fp_to_int(fp_to_int),
        .int_to_fp(int_to_fp),
        .is_fp_load(is_fp_load),
        .is_fp_store(is_fp_store),
        .instruction_valid(instruction_valid)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= imm_i ^ imm_s ^ imm_b ^ imm_u ^ imm_j ^ 
                      {27'h0, alu_op} ^
                      {23'h0, alu_src, reg_write, mem_write, mem_read, mem_to_reg, branch, jump, is_ecall, instruction_valid};
    end

`elsif SYNTH_DECOMPRESS
    // ============================================================
    // Decompressor Test
    // ============================================================
    logic [31:0] insn_32;
    logic is_compressed, is_valid;
    
    decompress u_decompress (
        .insn_16(stim_reg[0][15:0]),
        .insn_32_in(stim_reg[1]),
        .insn_32(insn_32),
        .is_compressed(is_compressed),
        .is_valid(is_valid)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= insn_32 ^ {30'h0, is_compressed, is_valid};
    end

`elsif SYNTH_DIV_UNIT
    // ============================================================
    // Division Unit Test
    // ============================================================
    logic [31:0] div_result;
    logic div_ready;
    
    div_unit #(.WIDTH(32)) u_div (
        .clk(clk),
        .rst_n(rst_n),
        .start(stim_sel[0]),
        .is_signed(stim_sel[1]),
        .rem_sel(stim_sel[2]),
        .dividend(stim_reg[0]),
        .divisor(stim_reg[1]),
        .result(div_result),
        .ready(div_ready)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= div_result ^ {31'h0, div_ready};
    end

`elsif SYNTH_FETCH_BUFFER
    // ============================================================
    // Fetch Buffer Test
    // ============================================================
    logic [31:0] decomp_output;
    logic        decomp_is_valid;
    logic current_insn_compressed;
    logic [31:0] pc_increment;
    
    fetch_buffer u_fetch_buffer (
        .clk(clk),
        .rst_n(rst_n),
        .imem_data(stim_reg[0]),
        .imem_ready(stim_sel[0]),
        .pc(stim_reg[1]),
        .ir_write(stim_sel[1]),
        .pc_write(stim_sel[2]),
        .is_branch(stim_sel[3]),
        .is_writeback(stim_sel[4]),
        .decomp_output(decomp_output),
        .decomp_is_valid(decomp_is_valid),
        .current_insn_compressed(current_insn_compressed),
        .pc_increment(pc_increment)
    );
    
    always_ff @(posedge clk) begin
        // Fold the fetch buffer outputs into one observable register so synthesis
        // keeps the decompressed instruction path, width tracking, and validity bit.
        result_reg <= decomp_output ^ pc_increment ^ {30'h0, current_insn_compressed, decomp_is_valid};
    end

`elsif SYNTH_FP_REGFILE
    // ============================================================
    // FP Register File Test
    // ============================================================
    logic [31:0] fp_rs1_data, fp_rs2_data, fp_rs3_data;
    
    fp_regfile u_fp_regfile (
        .clk(clk),
        .rst_n(rst_n),
        .we(stim_sel[0]),
        .rs1_addr(stim_sel[4:0]),
        .rs2_addr(stim_reg[0][4:0]),
        .rs3_addr(stim_reg[1][4:0]),
        .rd_addr(stim_reg[2][4:0]),
        .rd_data(stim_reg[3]),
        .rs1_data(fp_rs1_data),
        .rs2_data(fp_rs2_data),
        .rs3_data(fp_rs3_data)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= fp_rs1_data ^ fp_rs2_data ^ fp_rs3_data;
    end

`elsif SYNTH_FPU
    // ============================================================
    // FPU Test (Full FPU with all submodules)
    // ============================================================
    logic [31:0] fp_result, int_result;
    logic [4:0] fflags;
    logic fpu_ready;
    
    fpu u_fpu (
        .clk(clk),
        .rst_n(rst_n),
        .fpu_start(stim_sel[0]),
        .fs1(stim_reg[0]),
        .fs2(stim_reg[1]),
        .fs3(stim_reg[2]),
        .int_src(stim_reg[3]),
        .fpu_op(stim_sel[4:0]),
        .rm(stim_reg[4][2:0]),
        .fp_result(fp_result),
        .int_result(int_result),
        .fflags(fflags),
        .fpu_ready(fpu_ready)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= fp_result ^ int_result ^ {27'h0, fflags} ^ {31'h0, fpu_ready};
    end

`elsif SYNTH_FPU_CLASSIFIER
    // ============================================================
    // FPU Classifier Test
    // ============================================================
    logic is_nan, is_snan, is_inf, is_zero, is_subnormal;
    
    fpu_classifier u_fpu_classifier (
        .val(stim_reg[0]),
        .is_nan(is_nan),
        .is_snan(is_snan),
        .is_inf(is_inf),
        .is_zero(is_zero),
        .is_subnormal(is_subnormal)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= {27'h0, is_nan, is_snan, is_inf, is_zero, is_subnormal};
    end

`elsif SYNTH_FPU_COMPARATOR
    // ============================================================
    // FPU Comparator Test
    // ============================================================
    logic less_than;
    
    fpu_comparator u_fpu_comparator (
        .a(stim_reg[0]),
        .b(stim_reg[1]),
        .less_than(less_than)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= {31'h0, less_than};
    end

`elsif SYNTH_FPU_DIV_ASSEMBLE
    // ============================================================
    // FPU Division Assemble Test
    // ============================================================
    logic [31:0] div_result;
    logic [4:0] div_flags;
    
    fpu_div_assemble u_fpu_div_assemble (
        .a(stim_reg[0]),
        .b(stim_reg[1]),
        .quotient_raw({stim_reg[2][15:0], stim_reg[3]}),
        .result(div_result),
        .flags(div_flags)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= div_result ^ {27'h0, div_flags};
    end

`elsif SYNTH_FPU_DIV_SETUP
    // ============================================================
    // FPU Division Setup Test
    // ============================================================
    logic [47:0] dividend, divisor;
    logic needs_div;
    logic [31:0] special_result;
    logic [4:0] setup_flags;
    
    fpu_div_setup u_fpu_div_setup (
        .a(stim_reg[0]),
        .b(stim_reg[1]),
        .dividend(dividend),
        .divisor(divisor),
        .needs_div(needs_div),
        .special_result(special_result),
        .flags(setup_flags)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= dividend[31:0] ^ dividend[47:32] ^ divisor[31:0] ^ divisor[47:32] ^ 
                      special_result ^ {26'h0, needs_div, setup_flags};
    end

`elsif SYNTH_FPU_FLOAT_TO_INT
    // ============================================================
    // FPU Float to Int Test
    // ============================================================
    logic [31:0] fti_result;
    logic fti_invalid;
    
    fpu_float_to_int u_fpu_float_to_int (
        .val(stim_reg[0]),
        .is_signed(stim_sel[0]),
        .result(fti_result),
        .invalid(fti_invalid)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= fti_result ^ {31'h0, fti_invalid};
    end

`elsif SYNTH_FPU_FMA
    // ============================================================
    // FPU FMA Test
    // ============================================================
    logic [31:0] fma_result;
    logic [4:0] fma_flags;
    
    fpu_fma u_fpu_fma (
        .a(stim_reg[0]),
        .b(stim_reg[1]),
        .c(stim_reg[2]),
        .negate_product(stim_sel[0]),
        .negate_addend(stim_sel[1]),
        .result(fma_result),
        .flags(fma_flags)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= fma_result ^ {27'h0, fma_flags};
    end

`elsif SYNTH_FPU_INT_TO_FLOAT
    // ============================================================
    // FPU Int to Float Test
    // ============================================================
    logic [31:0] itf_result;
    
    fpu_int_to_float u_fpu_int_to_float (
        .val(stim_reg[0]),
        .is_signed(stim_sel[0]),
        .result(itf_result)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= itf_result;
    end

`elsif SYNTH_FPU_SQRT
    // ============================================================
    // FPU Sqrt Test
    // ============================================================
    logic [31:0] sqrt_result;
    logic [4:0] sqrt_flags;
    
    fpu_sqrt u_fpu_sqrt (
        .a(stim_reg[0]),
        .result(sqrt_result),
        .flags(sqrt_flags)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= sqrt_result ^ {27'h0, sqrt_flags};
    end

`elsif SYNTH_MEM_INTERFACE
    // ============================================================
    // Memory Interface Test
    // ============================================================
    logic [31:0] dmem_addr, dmem_wdata, formatted_load_data;
    logic dmem_we, dmem_re;
    logic [1:0] dmem_size;
    
    mem_interface u_mem_interface (
        .funct3(stim_reg[0][2:0]),
        .mem_write(stim_sel[0]),
        .mem_read(stim_sel[1]),
        .is_atomic_rmw(stim_sel[2]),
        .is_mem_write_state(stim_sel[3]),
        .is_sc(stim_sel[4]),
        .sc_success(stim_reg[1][0]),
        .is_fp_store(stim_reg[1][1]),
        .alu_result(stim_reg[2]),
        .rs2_data(stim_reg[3]),
        .fs2_data(stim_reg[4]),
        .dmem_rdata(stim_reg[5]),
        .amo_wdata(stim_reg[6]),
        .dmem_addr(dmem_addr),
        .dmem_wdata(dmem_wdata),
        .dmem_we(dmem_we),
        .dmem_re(dmem_re),
        .dmem_size(dmem_size),
        .formatted_load_data(formatted_load_data)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= dmem_addr ^ dmem_wdata ^ formatted_load_data ^ {28'h0, dmem_we, dmem_re, dmem_size};
    end

`elsif SYNTH_REGFILE
    // ============================================================
    // Register File Test
    // ============================================================
    logic [31:0] rs1_data, rs2_data;
    
    regfile u_regfile (
        .clk(clk),
        .we(stim_sel[0]),
        .rs1_addr(stim_sel[4:0]),
        .rs2_addr(stim_reg[0][4:0]),
        .rd_addr(stim_reg[1][4:0]),
        .rd_data(stim_reg[2]),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= rs1_data ^ rs2_data;
    end

`elsif SYNTH_WRITEBACK_MUX
    // ============================================================
    // Writeback Mux Test
    // ============================================================
    logic [31:0] rd_data;
    
    writeback_mux u_writeback_mux (
        .opcode(stim_reg[0][6:0]),
        .jump(stim_sel[0]),
        .is_csr(stim_sel[1]),
        .mem_to_reg(stim_sel[2]),
        .is_lr(stim_sel[3]),
        .is_sc(stim_sel[4]),
        .is_amo(stim_reg[1][0]),
        .sc_success(stim_reg[1][1]),
        .fp_to_int(stim_reg[1][2]),
        .imm_u(stim_reg[3]),
        .alu_result(stim_reg[4]),
        .csr_rdata(stim_reg[5]),
        .formatted_load_data(stim_reg[6]),
        .fpu_result(stim_reg[7]),
        .rd_data(rd_data)
    );
    
    always_ff @(posedge clk) begin
        result_reg <= rd_data;
    end

`else
    // Default: just a counter
    always_ff @(posedge clk) begin
        if (!rst_n)
            result_reg <= 32'h0;
        else
            result_reg <= result_reg + 1'b1;
    end
`endif

    // Output to LEDs (prevents output optimization)
    assign led = result_reg[7:0];

endmodule
`default_nettype wire

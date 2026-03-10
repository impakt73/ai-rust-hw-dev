// CPU Core Module
// Multi-cycle RISC-V RV32IMC processor with variable-latency memory support
// Configurable extension support for resource-constrained FPGA targets
//
// UNIFIED MEMORY INTERFACE: This CPU uses a single memory interface for both
// instruction fetch and data access. The multi-cycle FSM ensures that only one
// type of access is active at a time (S_FETCH for instructions, S_MEM_READ/
// S_MEM_WRITE/S_ATOMIC_RMW for data). This design simplifies the memory
// subsystem while remaining compatible with future arbiter integration if
// pipelining is added.
//
// REGISTER FILE: Uses dual-banked BRAM with 2-cycle read latency.
// The S_REG_READ and S_REG_READ_WAIT states provide time for BRAM reads to
// complete after S_DECODE.

module cpu #(
    parameter bit ENABLE_M_EXT = 1'b1,  // RV32M extension: Multiply/Divide (default: enabled)
    parameter bit ENABLE_F_EXT = 1'b1   // RV32F extension: Floating-Point (default: enabled)
) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        boot,
    input  logic        req_halt,
    input  logic [31:0] boot_addr,
    
    // Memory address channel (A)
    output logic [31:0] mem_a_addr,    // Memory address
    output logic [31:0] mem_a_wdata,   // Write data
    output logic        mem_a_we,      // Write enable
    output logic [1:0]  mem_a_size,    // Operation size: 00=byte, 01=halfword, 10=word
    output logic        mem_a_valid,   // Address channel valid
    input  logic        mem_a_ready,   // Address channel ready
    
    // Memory data channel (D)
    input  logic [31:0] mem_d_rdata,   // Read data / write response payload
    input  logic        mem_d_valid,   // Data channel valid
    output logic        mem_d_ready,   // Data channel ready
    
    // System control signals
    output logic        halted,       // CPU halted (ECALL/EBREAK)
    output logic        instr_complete, // High for 1 cycle when instruction done
    
    // Debug outputs (for tracing register values)
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data,
    
    // Debug outputs for instruction tracing (completed instruction)
    output logic [31:0] debug_pc,         // PC of completed instruction
    output logic [31:0] debug_instruction, // Instruction word of completed instruction
    
    // Debug outputs for current execution state (live signals)
    output logic [31:0] debug_current_pc,         // Current PC (for hung detection)
    output logic [31:0] debug_current_instruction, // Current instruction (for hung detection)
    
    // Debug output for FSM state visibility
    output logic [3:0]  debug_fsm_state,  // Current FSM state (for debugging)
    
    // Boot state indicator
    output logic        is_booting        // High when CPU is in boot state (S_BOOT)
);

    // ============================================================
    // FSM State Definitions
    // ============================================================
    typedef enum logic [3:0] {
        S_BOOT       = 4'b0000,  // After reset, wait for boot signal
        S_FETCH      = 4'b0001,  // Fetch instruction (wait for D-channel response)
        S_DECODE     = 4'b0010,  // Decode instruction, start register file read
        S_REG_READ   = 4'b1100,  // Launch BRAM register file read pipeline
        S_REG_READ_WAIT = 4'b1101,  // Capture BRAM register file read data
        S_EXECUTE    = 4'b0011,  // ALU operation
        S_MEM_ADDR   = 4'b0100,  // Calculate memory address
        S_MEM_READ   = 4'b0101,  // Load from memory (wait for D-channel response)
        S_MEM_WRITE  = 4'b0110,  // Store to memory (wait for D-channel response)
        S_WRITEBACK  = 4'b0111,  // Write result to register
        S_BRANCH     = 4'b1000,  // Branch decision
        S_CSR        = 4'b1001,  // CSR operation
        S_HALT       = 4'b1010,  // ECALL/EBREAK
        S_ATOMIC_RMW = 4'b1011   // Atomic read-modify-write (A extension)
    } state_t;

    state_t current_state, next_state;

    // ============================================================
    // Internal signals
    // ============================================================
    logic [31:0] pc;
    logic [31:0] instruction;
    
    // Internal instruction complete signal (immediate)
    logic instr_complete_internal;
    
    // ============================================================
    // RV32C Fetch Buffer and Decompressor Signals
    // ============================================================
    // Decompressor signals
    logic [31:0] decomp_input_32;    // Full assembled instruction (input to decompressor)
    logic [31:0] decomp_output;      // Decompressed 32-bit instruction
    logic        decomp_is_compressed; // Decompressor detected compressed instruction
    logic        decomp_is_valid;    // Decompressor output is valid
    
    // Instruction width tracking (from fetch buffer)
    logic        current_insn_compressed; // Current instruction being executed is compressed
    logic [31:0] pc_increment;       // How much to increment PC (2 or 4 bytes)
    
    // ============================================================
    // LR/SC Reservation Station (A Extension)
    // ============================================================
    logic        reservation_valid;
    logic [31:0] reservation_addr;
    
    // ============================================================
    // Staging Registers (Flip-Flops for Multi-Cycle Operation)
    // ============================================================
    
    // Instruction Register
    logic [31:0] ir_reg;
    logic ir_write;
    
    // Operand Registers (Integer)
    logic [31:0] a_reg;  // rs1 data (integer)
    logic [31:0] b_reg;  // rs2 data (integer)
    logic a_reg_write, b_reg_write;
    
    // FP Operand Registers
    logic [31:0] fa_reg;  // fs1 data (FP)
    logic [31:0] fb_reg;  // fs2 data (FP)
    logic [31:0] fc_reg;  // fs3 data (FP, for fused multiply-add)
    logic fa_reg_write, fb_reg_write, fc_reg_write;
    
    // Result Registers
    logic [31:0] alu_out_reg;  // ALU output
    logic [31:0] fpu_out_reg;  // FPU output
    logic [31:0] mdr;          // Memory data register
    logic alu_out_write, fpu_out_write, mdr_write;
    
    // Decoder outputs (registered inside decoder.sv)
    logic [6:0]  opcode_reg;
    logic [4:0]  rd_reg, rs1_reg, rs2_reg;
    logic [2:0]  funct3_reg;
    logic [6:0]  funct7_reg;
    logic [31:0] imm_i_reg, imm_s_reg, imm_b_reg, imm_u_reg, imm_j_reg;
    logic [4:0]  alu_op_reg;
    logic        alu_src_reg, reg_write_reg, mem_write_reg, mem_read_reg;
    logic        mem_to_reg_reg, branch_reg, jump_reg, is_auipc_reg;
    
    // PC for the current instruction (captured when the instruction is fetched)
    logic [31:0] instr_pc_reg;
    
    // Pre-computed branch/jump target registers (for timing closure)
    // These are registered during DECODE/EXECUTE to avoid long combinational paths
    // from the adder to next_pc_value in the same cycle
    logic [31:0] branch_target_reg;  // pc + imm_b (for B-type branches)
    logic [31:0] jal_target_reg;     // pc + imm_j (for JAL)
    logic [31:0] jalr_target_reg;    // a_reg + imm_i (for JALR, computed during EXECUTE)
    logic        jalr_target_write;  // Control signal to write jalr_target_reg
    
    // Completed instruction registers (captured at instruction completion for tracing)
    logic [31:0] completed_pc_reg;
    logic [31:0] completed_instr_reg;
    logic        is_ecall_reg, is_ebreak_reg, is_fence_reg, is_csr_reg;
    logic        is_lr_reg, is_sc_reg, is_amo_reg;  // A extension registers
    logic [4:0]  funct5_reg;  // A extension - atomic operation type
    // F extension registers
    logic [4:0]  fpu_op_reg;
    logic        fp_reg_write_reg, fp_to_int_reg, int_to_fp_reg;
    logic        is_fp_load_reg, is_fp_store_reg;  // FP load/store flags
    logic        instruction_valid;  // Decoder validity for the current instruction
    // Merged instruction validity register:
    // - Reset to 1 (assume valid on startup)
    // - Updated with decompressor validity when instruction fetched (ir_write)
    // - ANDed with decoder validity during S_DECODE
    logic        is_instruction_valid_reg;
    
    // Debug trace data registers (capture operand values at instruction completion)
    logic [31:0] trace_rs1_data_reg;
    logic [31:0] trace_rs2_data_reg;
    logic [31:0] trace_rd_data_reg;
    
    // Control Signals
    logic        pc_write;
    logic        reg_write_en;
    logic        fp_reg_write_en;  // FP register write enable (gated by FSM)
    logic        csr_rdata_write;  // Control signal to latch CSR read data
    
    // Integer Register file signals
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] rd_data;
    
    // FP Register file signals
    logic [31:0] fs1_data;
    logic [31:0] fs2_data;
    logic [31:0] fs3_data;
    logic [31:0] fd_data;
    
    // ALU signals
    logic [31:0] alu_a;
    logic [31:0] alu_b;
    logic [31:0] alu_result;
    logic        alu_zero;
    logic        alu_start;       // NEW: Start ALU operation
    logic        alu_ready;       // NEW: ALU operation complete
    logic        alu_start_sent;  // NEW: Track if start pulse has been sent (S_EXECUTE)
    logic        alu_start_sent_rmw;  // NEW: Track if start pulse has been sent (S_ATOMIC_RMW)
    
    // FPU signals
    logic [31:0] fpu_fp_result;   // FP result from FPU
    logic [31:0] fpu_int_result;  // Integer result from FPU (for comparisons, conversions)
    logic [4:0]  fpu_fflags;      // FPU exception flags
    logic [2:0]  fpu_rm;          // Rounding mode (from instruction or FCSR)
    logic        fpu_start;       // NEW: Start FPU operation
    logic        fpu_ready;       // NEW: FPU operation complete
    logic        fpu_start_sent;  // NEW: Track if start pulse has been sent
    
    // FCSR (Floating Point Control and Status Register)
    logic [31:0] fcsr;            // Full FCSR register
    // FCSR bitfields: {24'h0, frm[2:0], fflags[4:0]}
    // frm = rounding mode, fflags = exception flags (NV, DZ, OF, UF, NX)
    
    // Branch/Jump logic
    logic        take_branch;
    
    // CSR signals
    logic [11:0] csr_addr;
    logic [31:0] csr_rdata;
    logic [31:0] csr_rdata_reg;  // Registered CSR read data (captured before write)
    
    // Memory interface signals
    logic [31:0] formatted_load_data;
    
    // Internal memory interface signals (before unification)
    // These are generated by different pipeline stages and then muxed to the
    // unified memory interface outputs based on FSM state
    logic [31:0] imem_addr_internal;   // Instruction fetch address (PC)
    logic        imem_req_internal;    // Instruction fetch request
    logic [31:0] dmem_addr_internal;   // Data memory address
    logic [31:0] dmem_wdata_internal;  // Data memory write data
    logic        dmem_we_internal;     // Data memory write enable
    logic [1:0]  dmem_size_internal;   // Data memory operation size
    logic        dmem_req_internal;    // Data memory request
    logic        mem_req_inflight;     // Address request accepted, waiting for data response
    logic        mem_a_handshake;
    logic        mem_d_handshake;
    
    // Memory ready signal routing
    // In S_FETCH: imem_ready_internal indicates instruction response handshake
    // In S_MEM_READ/S_MEM_WRITE/S_ATOMIC_RMW: dmem_ready_internal indicates
    // data response handshake.
    logic        imem_ready_internal;  // Instruction memory response handshake
    logic        dmem_ready_internal;  // Data memory response handshake
    
    // Response completes on D-channel valid/ready handshake
    assign imem_ready_internal = mem_d_valid && mem_d_ready;
    assign dmem_ready_internal = mem_d_valid && mem_d_ready;
    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    
    // Track whether an address-channel request has been accepted and is awaiting
    // a data-channel response.
    always_ff @(posedge clk) begin
        if (!rst_n)
            mem_req_inflight <= 1'b0;
        else
            mem_req_inflight <= (mem_req_inflight || mem_a_handshake) && !mem_d_handshake;
    end
    
    // A extension: SC success/failure logic
    logic        sc_success;
    assign sc_success = reservation_valid && (reservation_addr == alu_out_reg);
    
    // A extension: AMO write data selection
    // AMOSWAP uses rs2 directly, others use ALU result
    logic [31:0] amo_write_data;
    assign amo_write_data = (funct5_reg == 5'b00001) ? b_reg : alu_result;  // funct5==00001 is AMOSWAP
    
    // CSR address comes directly from the decoder's registered immediate output.
    assign csr_addr = imm_i_reg[11:0];
    
    // ============================================================
    // State Register (Flip-Flop Based FSM)
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n)
            current_state <= S_BOOT;
        else
            current_state <= next_state;
    end
    
    // ============================================================
    // Staging Register Implementations (All Flip-Flops)
    // ============================================================
    
    // Instruction Register and Validity Tracking
    // Now stores the decompressed 32-bit instruction (expanded from 16-bit if compressed)
    // is_instruction_valid_reg tracks instruction validity through the pipeline:
    // - Reset to 1 (assume valid on startup)
    // - Populated with decompressor validity when instruction fetched (ir_write)
    // - ANDed with decoder validity during S_DECODE
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            ir_reg <= 32'h0;
            instr_pc_reg <= 32'h0;
            is_instruction_valid_reg <= 1'b1;  // Assume valid on startup
        end else if (ir_write) begin
            ir_reg <= decomp_output;  // Use decompressed output
            instr_pc_reg <= pc;  // Capture PC of this instruction alongside the decode outputs
            is_instruction_valid_reg <= decomp_is_valid;  // Capture decompressor validity
        end else if (current_state == S_DECODE) begin
            // u_decoder captures instruction_valid on the same ir_write edge as ir_reg,
            // then holds it stable through S_DECODE for this merge step.
            // AND with decoder validity - instruction must be valid from both
            // decompressor and decoder to be considered valid
            is_instruction_valid_reg <= is_instruction_valid_reg & instruction_valid;
        end
    end
    
    // Operand Registers (Integer)
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            a_reg <= 32'h0;
            b_reg <= 32'h0;
        end else begin
            if (a_reg_write) a_reg <= rs1_data;
            if (b_reg_write) b_reg <= rs2_data;
        end
    end
    
    // ============================================================
    // FP Operand Registers (Conditional Generation)
    // ============================================================
    generate
        if (ENABLE_F_EXT) begin : gen_fp_operand_regs
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    fa_reg <= 32'h0;
                    fb_reg <= 32'h0;
                    fc_reg <= 32'h0;
                end else begin
                    if (fa_reg_write) fa_reg <= fs1_data;
                    if (fb_reg_write) fb_reg <= fs2_data;
                    if (fc_reg_write) fc_reg <= fs3_data;
                end
            end
        end else begin : gen_no_fp_operand_regs
            // F extension disabled: Tie registers to zero
            assign fa_reg = 32'd0;
            assign fb_reg = 32'd0;
            assign fc_reg = 32'd0;
        end
    endgenerate
    
    // Result Registers
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            alu_out_reg <= 32'h0;
            fpu_out_reg <= 32'h0;
            mdr <= 32'h0;
            csr_rdata_reg <= 32'h0;
        end else begin
            if (alu_out_write) alu_out_reg <= alu_result;
            if (ENABLE_F_EXT && fpu_out_write) fpu_out_reg <= fp_to_int_reg ? fpu_int_result : fpu_fp_result;
            else if (fpu_out_write) fpu_out_reg <= 32'd0;  // F extension disabled
            if (mdr_write) mdr <= formatted_load_data;
            if (csr_rdata_write) csr_rdata_reg <= csr_rdata;
        end
    end
    
    // Branch/Jump Target Registers
    // Pre-compute branch and jump targets during DECODE to break timing path
    // For B-type and JAL: instr_pc_reg + immediate is computed during S_DECODE
    // For JALR: a_reg + imm_i is computed during EXECUTE (after a_reg is stable)
    // Note: Halfword alignment (~32'h1) is used because RV32C compressed instructions
    // can be 2-byte aligned. For non-compressed RV32I-only, this would be ~32'h3.
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            branch_target_reg <= 32'h0;
            jal_target_reg <= 32'h0;
            jalr_target_reg <= 32'h0;
        end else begin
            // Capture B-type and JAL targets once the decoder's registered outputs are available.
            // instr_pc_reg and imm_*_reg are loaded on the same ir_write edge, so they
            // remain aligned for the current instruction throughout S_DECODE.
            if (current_state == S_DECODE) begin
                branch_target_reg <= (instr_pc_reg + imm_b_reg) & ~32'h1;  // Halfword aligned for RV32C
                jal_target_reg <= (instr_pc_reg + imm_j_reg) & ~32'h1;     // Halfword aligned for RV32C
            end
            // Capture JALR target during EXECUTE (a_reg + imm_i_reg)
            // a_reg is stable after DECODE, imm_i_reg comes directly from the decoder register
            if (jalr_target_write) begin
                jalr_target_reg <= (a_reg + imm_i_reg) & ~32'h1;  // Halfword aligned for RV32C
            end
        end
    end

    
    // Completed Instruction Registers (captured at instruction completion)
    // Capture when current_state is in a completion state AND we're about to leave it
    // Delayed instr_complete signal for proper trace timing
    // Capture happens on cycle N when instr_complete_internal goes high
    // Output port sees delayed version on cycle N+1 after values have settled
    always_ff @(posedge clk) begin
        if (!rst_n)
            instr_complete <= 1'b0;
        else
            instr_complete <= instr_complete_internal;
    end
    
    // Capture completed instruction info when instruction finishes
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            completed_pc_reg <= 32'h0;
            completed_instr_reg <= 32'h0;
            trace_rs1_data_reg <= 32'h0;
            trace_rs2_data_reg <= 32'h0;
            trace_rd_data_reg <= 32'h0;
        end else if (instr_complete_internal) begin
            completed_pc_reg <= instr_pc_reg;
            completed_instr_reg <= ir_reg;
            trace_rs1_data_reg <= a_reg;
            trace_rs2_data_reg <= b_reg;
            trace_rd_data_reg <= rd_data;
        end
    end
    
    // Track if ALU start pulse has been sent (for multi-cycle operations)
    always_ff @(posedge clk) begin
        if (!rst_n)
            alu_start_sent <= 1'b0;
        else if (current_state != S_EXECUTE)
            alu_start_sent <= 1'b0;  // Reset when leaving S_EXECUTE
        else if (alu_start)
            alu_start_sent <= 1'b1;  // Mark as sent after pulsing
    end
    
    always_ff @(posedge clk) begin
        if (!rst_n)
            alu_start_sent_rmw <= 1'b0;
        else if (current_state != S_ATOMIC_RMW)
            alu_start_sent_rmw <= 1'b0;  // Reset when leaving S_ATOMIC_RMW
        else if (alu_start)
            alu_start_sent_rmw <= 1'b1;  // Mark as sent after pulsing
    end
    
    // Track if FPU start pulse has been sent (for multi-cycle FP operations)
    always_ff @(posedge clk) begin
        if (!rst_n)
            fpu_start_sent <= 1'b0;
        else if (current_state != S_EXECUTE)
            fpu_start_sent <= 1'b0;  // Reset when leaving S_EXECUTE
        else if (fpu_start)
            fpu_start_sent <= 1'b1;  // Mark as sent after pulsing
    end
    
    // ============================================================
    // LR/SC Reservation Tracking (A Extension)
    // ============================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            reservation_valid <= 1'b0;
            reservation_addr <= 32'h0;
        end else begin
            // Set reservation on LR.W completion (in S_MEM_READ with dmem_ready_internal)
            if (is_lr_reg && current_state == S_MEM_READ && dmem_ready_internal) begin
                reservation_valid <= 1'b1;
                reservation_addr <= alu_out_reg;  // Address from ALU (rs1 + 0)
            end
            // Clear reservation on SC.W (any SC, regardless of success)
            else if (is_sc_reg && current_state == S_MEM_WRITE && dmem_ready_internal) begin
                reservation_valid <= 1'b0;
            end
            // Clear reservation on any write to the reserved address (except SC.W writes)
            else if (dmem_we_internal && reservation_valid && dmem_addr_internal == reservation_addr && !is_sc_reg) begin
                reservation_valid <= 1'b0;
            end
        end
    end
    
    // ============================================================
    // RV32C Fetch Buffer and Decompressor Module Instantiations
    // ============================================================
    
    // Instantiate fetch buffer module
    // Note: Uses mem_d_rdata and imem_ready_internal for instruction fetch
    fetch_buffer u_fetch_buffer (
        .clk(clk),
        .rst_n(rst_n),
        .imem_data(mem_d_rdata),         // Memory read data from D channel
        .imem_ready(imem_ready_internal), // Routed from D-channel handshake
        .pc(pc),
        .ir_write(ir_write),
        .pc_write(pc_write),
        .is_branch(current_state == S_BRANCH),
        .is_writeback(current_state == S_WRITEBACK),
        .decomp_input(decomp_input_32),
        .decomp_is_compressed(decomp_is_compressed),
        .current_insn_compressed(current_insn_compressed),
        .pc_increment(pc_increment)
    );
    
    // Instantiate decompressor module
    // The decompressor looks at bits [15:0] to determine if compressed
    // and either decompresses or passes through the full 32 bits
    decompress decomp_inst (
        .insn_16(decomp_input_32[15:0]),
        .insn_32_in(decomp_input_32),
        .insn_32(decomp_output),
        .is_compressed(decomp_is_compressed),
        .is_valid(decomp_is_valid)
    );
    
    // ============================================================
    // Program Counter with Multi-Cycle Control
    // ============================================================
    // TIMING OPTIMIZATION: Branch/jump targets are pre-computed and registered
    // during DECODE (for B-type/JAL) or EXECUTE (for JALR) to break the critical
    // timing path from adder to PC in the same cycle.
    logic [31:0] next_pc_value;
    
    always_comb begin
        next_pc_value = pc + pc_increment;  // Sequential: increment by 2 or 4 bytes
        
        if (current_state == S_BRANCH) begin
            if (take_branch)
                next_pc_value = branch_target_reg;  // Use pre-computed branch target
        end else if (current_state == S_WRITEBACK && jump_reg) begin
            if (alu_src_reg)
                next_pc_value = jalr_target_reg;    // Use pre-computed JALR target
            else
                next_pc_value = jal_target_reg;     // Use pre-computed JAL target
        end
    end
    
    always_ff @(posedge clk) begin
        if (!rst_n)
            pc <= 32'h0;
        else if (current_state == S_BOOT && boot && !req_halt)
            pc <= boot_addr;
        else if (pc_write)
            pc <= next_pc_value;
    end
    
    // Halted signal
    assign halted = (current_state == S_HALT);
    
    // ============================================================
    // Internal Memory Address Assignments
    // ============================================================
    assign imem_addr_internal = pc;
    assign instruction = ir_reg;  // Use registered instruction
    
    // ============================================================
    // FSM Next-State Logic
    // ============================================================
    always_comb begin
        next_state = current_state;
        
        case (current_state)
            S_BOOT: begin
                if (req_halt)
                    next_state = S_HALT;
                else if (boot)
                    next_state = S_FETCH;
                else
                    next_state = S_BOOT;
            end
            
            S_FETCH: begin
                if (req_halt)
                    next_state = S_HALT;
                // Wait for instruction memory ready (unified interface)
                else if (imem_ready_internal)
                    next_state = S_DECODE;
                else
                    next_state = S_FETCH;
            end
            
            S_DECODE: begin
                // Decode instruction and start register file read.
                // Always transition through S_REG_READ and S_REG_READ_WAIT to cover the
                // two-cycle BRAM read latency.
                next_state = S_REG_READ;
            end
            
            // S_REG_READ: Wait for the internal BRAM pipeline stage
            S_REG_READ: begin
                next_state = S_REG_READ_WAIT;
            end

            // S_REG_READ_WAIT: BRAM data is now visible on the module outputs
            // Uses opcode_reg (captured in S_DECODE) to determine next state
            S_REG_READ_WAIT: begin
                // Check for invalid instruction using the merged validity register.
                // is_instruction_valid_reg combines:
                // 1. Decompressor validity (captured during ir_write in S_FETCH)
                // 2. Decoder validity (ANDed during S_DECODE)
                if (!is_instruction_valid_reg) begin
                    next_state = S_HALT;  // Invalid instruction - halt for debug
                end else begin
                    // Now register file data is available, proceed based on instruction type
                    case (opcode_reg)
                        7'b0110011,  // R-type
                        7'b0010011,  // I-type arithmetic
                        7'b0110111,  // LUI
                        7'b0010111,  // AUIPC
                        7'b1101111,  // JAL
                        7'b1100111:  // JALR
                            next_state = S_EXECUTE;
                        
                        7'b1010011,  // OP_FP: FP computational instructions
                        7'b1000011,  // OP_FMADD: Fused multiply-add
                        7'b1000111,  // OP_FMSUB: Fused multiply-sub
                        7'b1001011,  // OP_FNMSUB: Fused negate-multiply-sub
                        7'b1001111:  // OP_FNMADD: Fused negate-multiply-add
                            next_state = S_EXECUTE;  // FP operations execute in S_EXECUTE
                        
                        7'b0000011,  // Load (integer: LW, LH, LB, LHU, LBU)
                        7'b0000111,  // Load FP (FLW)
                        7'b0100011,  // Store (integer: SW, SH, SB)
                        7'b0100111:  // Store FP (FSW)
                            next_state = S_MEM_ADDR;
                        
                        7'b0101111:  // AMO (Atomic operations - A extension)
                            next_state = S_MEM_ADDR;
                        
                        7'b1100011:  // Branch
                            next_state = S_BRANCH;
                        
                        7'b1110011: begin  // SYSTEM
                            if (is_ecall_reg || is_ebreak_reg)
                                next_state = S_HALT;
                            else if (is_csr_reg)
                                next_state = S_CSR;
                        end

                        7'b0001111:  // FENCE
                            next_state = S_FETCH;
                        
                        default: next_state = S_HALT;  // Unknown instruction - halt for debug
                    endcase
                end
            end
            
            S_EXECUTE: begin
                // FP computational operations may be multi-cycle (e.g., FP division)
                // But FP loads/stores go through memory states (not handled here)
                if ((fp_reg_write_reg || fp_to_int_reg) && !is_fp_load_reg) begin
                    // FP operations - wait for FPU ready
                    if (fpu_ready) begin
                        next_state = S_WRITEBACK;
                    end else begin
                        next_state = S_EXECUTE;  // Wait for multi-cycle FPU operation
                    end
                // Integer ALU operations may be multi-cycle (e.g., division)
                end else if (alu_ready) begin
                    next_state = S_WRITEBACK;
                end else begin
                    next_state = S_EXECUTE;  // Wait for multi-cycle ALU operation
                end
            end
            
            S_MEM_ADDR: begin
                if (mem_read_reg)
                    next_state = S_MEM_READ;
                else
                    next_state = S_MEM_WRITE;
            end
            
            S_MEM_READ: begin
                // Wait for data memory ready (unified interface)
                if (dmem_ready_internal) begin
                    if (is_amo_reg) begin
                        next_state = S_ATOMIC_RMW;  // AMO: proceed to RMW phase
                    end else begin
                        next_state = S_WRITEBACK;   // Normal load or LR.W
                    end
                end else begin
                    next_state = S_MEM_READ;
                end
            end
            
            S_MEM_WRITE: begin
                // Wait for data memory ready (unified interface)
                if (dmem_ready_internal) begin
                    if (is_sc_reg) begin
                        next_state = S_WRITEBACK;  // SC.W: write success/failure to rd
                    end else begin
                        next_state = S_FETCH;      // Normal store
                    end
                end else begin
                    next_state = S_MEM_WRITE;
                end
            end
            
            S_WRITEBACK: begin
                next_state = S_FETCH;
            end
            
            S_BRANCH: begin
                next_state = S_FETCH;
            end
            
            S_CSR: begin
                next_state = S_WRITEBACK;
            end
            
            S_ATOMIC_RMW: begin
                // Wait for ALU ready and memory ready before writeback (unified interface)
                if (alu_ready && dmem_ready_internal)
                    next_state = S_WRITEBACK;
                else
                    next_state = S_ATOMIC_RMW;
            end
            
            S_HALT: begin
                next_state = S_HALT;
            end
            
            default: begin
                next_state = S_HALT;
            end
        endcase
    end
    
    // ============================================================
    // FSM Control Signal Output Logic
    // ============================================================
    always_comb begin
        // Default all control signals to inactive
        ir_write = 1'b0;
        a_reg_write = 1'b0;
        b_reg_write = 1'b0;
        fa_reg_write = 1'b0;
        fb_reg_write = 1'b0;
        fc_reg_write = 1'b0;
        alu_out_write = 1'b0;
        fpu_out_write = 1'b0;
        mdr_write = 1'b0;
        csr_rdata_write = 1'b0;
        pc_write = 1'b0;
        reg_write_en = 1'b0;
        fp_reg_write_en = 1'b0;
        imem_req_internal = 1'b0;
        dmem_req_internal = 1'b0;
        instr_complete_internal = 1'b0;
        alu_start = 1'b0;  // Default ALU start to inactive
        fpu_start = 1'b0;  // Default FPU start to inactive
        jalr_target_write = 1'b0;  // Default JALR target write to inactive
        
        case (current_state)
            S_FETCH: begin
                imem_req_internal = !req_halt;
                if (imem_ready_internal)
                    ir_write = 1'b1;
            end
            
            S_DECODE: begin
                // Decode instruction and present addresses to register file.
                // Register data will be captured in S_REG_READ_WAIT after BRAM latency.
            end
            
            S_REG_READ: begin
            end
            
            // S_REG_READ_WAIT: Capture BRAM register file read data
            // This state captures register data after BRAM synchronous read
            S_REG_READ_WAIT: begin
                // BRAM data is now available, capture it
                a_reg_write = 1'b1;
                b_reg_write = 1'b1;
                // Capture CSR read data after CSR BRAM read latency
                if (is_csr_reg)
                    csr_rdata_write = 1'b1;
                // FP register reads (for FP operations) - using registered signals
                if (fp_reg_write_reg || fp_to_int_reg || int_to_fp_reg || is_fp_store_reg) begin
                    fa_reg_write = 1'b1;
                    fb_reg_write = 1'b1;
                    fc_reg_write = 1'b1;  // Always read rs3 for fused multiply-add
                end
                // FENCE completes here (after register read state)
                if (is_fence_reg) begin
                    pc_write = 1'b1;
                    instr_complete_internal = 1'b1;
                end
            end
            
            S_EXECUTE: begin
                // Integer ALU operations
                if (!fp_reg_write_reg && !fp_to_int_reg) begin
                    // Pulse alu_start only on first cycle in S_EXECUTE
                    alu_start = !alu_start_sent;
                    
                    if (alu_ready) begin
                        alu_out_write = 1'b1;
                    end
                    
                    // Capture JALR target (a_reg + imm_i_reg) during EXECUTE
                    // This breaks the timing path by registering the target before WRITEBACK
                    if (jump_reg && alu_src_reg) begin
                        jalr_target_write = 1'b1;
                    end
                end
                // FP operations (may be multi-cycle, e.g., division)
                else begin
                    // Pulse fpu_start only on first cycle in S_EXECUTE
                    fpu_start = !fpu_start_sent;
                    
                    if (fpu_ready) begin
                        fpu_out_write = 1'b1;
                    end
                end
            end
            
            S_MEM_ADDR: begin
                alu_out_write = 1'b1;
            end
            
            S_MEM_READ: begin
                dmem_req_internal = 1'b1;
                if (dmem_ready_internal)
                    mdr_write = 1'b1;
            end
            
            S_MEM_WRITE: begin
                dmem_req_internal = 1'b1;
                if (dmem_ready_internal) begin
                    pc_write = 1'b1;
                    instr_complete_internal = 1'b1;
                end
            end
            
            S_WRITEBACK: begin
                // Enable write to integer regfile (for integer ops and FP-to-int conversions)
                reg_write_en = 1'b1;
                // Enable write to FP regfile (for FP ops)
                fp_reg_write_en = 1'b1;
                pc_write = 1'b1;
                instr_complete_internal = 1'b1;
            end
            
            S_BRANCH: begin
                pc_write = 1'b1;
                instr_complete_internal = 1'b1;
            end
            
            S_CSR: begin
                // CSR operations: CSR read/modify happens here
                // CSR state transitions to WRITEBACK, which will complete the instruction
                // Don't set instr_complete here - let WRITEBACK do it
            end
            
            S_ATOMIC_RMW: begin
                // Atomic read-modify-write phase for AMO instructions
                // Pulse alu_start only on first cycle in S_ATOMIC_RMW
                alu_start = !alu_start_sent_rmw;
                
                dmem_req_internal = 1'b1;  // Request memory write
                
                if (alu_ready && dmem_ready_internal) begin
                    alu_out_write = 1'b1;  // Capture computed result for memory write
                end
            end
            
            S_HALT: begin
                // HALT state: all control signals remain inactive
                // Note: instr_complete_internal should NOT be asserted here
                // It's already asserted when transitioning TO halt (see below)
            end
            
            default: begin
                // All inactive
            end
        endcase
        
    end
    
    // ============================================================
    // ============================================================
    // Module Instantiations
    // ============================================================
    
    // Branch Decision Unit (uses registered signals for multi-cycle operation)
    branch_unit u_branch_unit (
        .branch(branch_reg),
        .funct3(funct3_reg),
        .rs1_data(a_reg),
        .rs2_data(b_reg),
        .take_branch(take_branch)
    );
    
    // Decoder instantiation (registers decode outputs when a new instruction is fetched)
    decoder #(
        .ENABLE_M_EXT(ENABLE_M_EXT),
        .ENABLE_F_EXT(ENABLE_F_EXT)
    ) u_decoder (
        .clk(clk),
        .rst_n(rst_n),
        .decode_en(ir_write),
        .instruction(decomp_output),
        .opcode(opcode_reg),
        .rd(rd_reg),
        .rs1(rs1_reg),
        .rs2(rs2_reg),
        .funct3(funct3_reg),
        .funct7(funct7_reg),
        .imm_i(imm_i_reg),
        .imm_s(imm_s_reg),
        .imm_b(imm_b_reg),
        .imm_u(imm_u_reg),
        .imm_j(imm_j_reg),
        .alu_op(alu_op_reg),
        .alu_src(alu_src_reg),
        .reg_write(reg_write_reg),
        .mem_write(mem_write_reg),
        .mem_read(mem_read_reg),
        .mem_to_reg(mem_to_reg_reg),
        .branch(branch_reg),
        .jump(jump_reg),
        .is_ecall(is_ecall_reg),
        .is_ebreak(is_ebreak_reg),
        .is_fence(is_fence_reg),
        .is_csr(is_csr_reg),
        .is_auipc(is_auipc_reg),
        .is_lr(is_lr_reg),
        .is_sc(is_sc_reg),
        .is_amo(is_amo_reg),
        .funct5(funct5_reg),
        // F extension outputs
        .fpu_op(fpu_op_reg),
        .fp_reg_write(fp_reg_write_reg),
        .fp_to_int(fp_to_int_reg),
        .int_to_fp(int_to_fp_reg),
        .is_fp_load(is_fp_load_reg),
        .is_fp_store(is_fp_store_reg),
        .instruction_valid(instruction_valid)
    );
    
    // Register file instantiation (write enable gated by FSM)
    // Uses dual-banked BRAM with 2-cycle read latency, handled by S_REG_READ and
    // S_REG_READ_WAIT states
    // x0 write gating: prevent writes to x0 (derived from registered rd_reg)
    logic reg_write_x0_gate;
    assign reg_write_x0_gate = (rd_reg != 5'd0);
    
    regfile u_regfile (
        .clk(clk),
        .we(reg_write_en & reg_write_reg & reg_write_x0_gate),  // Gated by FSM and x0 check
        .rs1_addr(rs1_reg),  // From registered decoder outputs, BRAM samples on clock edge
        .rs2_addr(rs2_reg),  // From registered decoder outputs, BRAM samples on clock edge
        .rd_addr(rd_reg),
        .rd_data(rd_data),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );
    
    // ALU operation selection (multi-cycle: may need different ops for different states)
    logic [4:0] alu_op_mux;
    always_comb begin
        // Default: use the operation from decoder
        alu_op_mux = alu_op_reg;
        
        // Special case for S_MEM_ADDR: always use ADD for address calculation
        // even if the instruction is an AMO with a different operation
        if (current_state == S_MEM_ADDR) begin
            alu_op_mux = 5'b00000;  // ALU_ADD
        end
    end
    
    // ALU source selection (multi-cycle: uses registered operands and control)
    always_comb begin
        // Default sources
        alu_a = a_reg;
        // For S-type stores (SW, FSW), use imm_s; for I-type (loads, etc.), use imm_i
        alu_b = alu_src_reg ? ((mem_write_reg || is_fp_store_reg) ? imm_s_reg : imm_i_reg) : b_reg;
        
        // Special case for S_MEM_ADDR with AMO/LR/SC: address is just rs1 (no offset)
        if (current_state == S_MEM_ADDR && (is_amo_reg || is_lr_reg || is_sc_reg)) begin
            alu_a = a_reg;  // rs1
            alu_b = 32'h0;  // No offset for atomic operations
        end
        // Special cases in EXECUTE state
        else if (current_state == S_EXECUTE) begin
            if (jump_reg) begin
                // Compute PC+4 for return address using the instruction PC captured at decode
                alu_a = instr_pc_reg;
                alu_b = 32'd4;
            end else if (is_auipc_reg) begin
                // Use the PC captured for this instruction at decode time
                alu_a = instr_pc_reg;
                alu_b = imm_u_reg;
            end
        end
        // Special case for S_ATOMIC_RMW: compute new value for AMO
        else if (current_state == S_ATOMIC_RMW) begin
            alu_a = mdr;    // Original value from memory
            alu_b = b_reg;  // rs2 data
        end
    end
    
    // ALU instantiation (uses registered control signals)
    alu #(
        .ENABLE_M_EXT(ENABLE_M_EXT)
    ) u_alu (
        .clk(clk),              // NEW: Clock for division unit
        .rst_n(rst_n),          // NEW: Reset for division unit
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op_mux),    // Use muxed operation (not alu_op_reg directly)
        .alu_start(alu_start),  // NEW: Start operation pulse
        .result(alu_result),
        .zero(alu_zero),
        .alu_ready(alu_ready)   // NEW: Operation complete
    );
    
    // Memory Interface Module (uses registered control signals)
    // Generates internal data memory signals that will be muxed to unified interface
    mem_interface u_mem_interface (
        .funct3(funct3_reg),
        .mem_write(mem_write_reg),
        .mem_read(mem_read_reg),
        .is_atomic_rmw(current_state == S_ATOMIC_RMW),  // A extension
        .is_mem_write_state(current_state == S_MEM_WRITE), // In S_MEM_WRITE state
        .is_sc(is_sc_reg),                               // A extension
        .sc_success(sc_success),                         // A extension
        .is_fp_store(is_fp_store_reg),                   // F extension
        .alu_result(alu_out_reg),  // Use registered ALU output for address
        .rs2_data(b_reg),           // Use registered rs2 data
        .fs2_data(fs2_data),        // F extension: FP store data
        .dmem_rdata(mem_d_rdata),   // Memory read data from D channel
        .amo_wdata(amo_write_data),     // A extension: muxed AMO write data
        .dmem_addr(dmem_addr_internal),
        .dmem_wdata(dmem_wdata_internal),
        .dmem_we(dmem_we_internal),
        .dmem_size(dmem_size_internal),
        .formatted_load_data(formatted_load_data)
    );
    
    // CSR File Module (uses registered signals)
    csr_file u_csr_file (
        .clk(clk),
        .rst_n(rst_n),
        .is_csr(is_csr_reg & (current_state == S_CSR)),  // Gated by FSM
        .instr_complete(instr_complete_internal),         // Instruction completion signal
        .funct3(funct3_reg),
        .rs1(rs1_reg),
        .csr_addr(csr_addr),
        .rs1_data(a_reg),  // Use registered rs1 data
        .fcsr(fcsr),       // F extension: FCSR register
        .csr_rdata(csr_rdata)
    );
    
    // ============================================================
    // F Extension: FP Register File and FPU (Conditional Generation)
    // ============================================================
    
    generate
        if (ENABLE_F_EXT) begin : gen_f_ext
            // FP Register File Module
            fp_regfile u_fp_regfile (
                .clk(clk),
                .rst_n(rst_n),
                .we(fp_reg_write_en & fp_reg_write_reg),  // Gated by FSM
                .rs1_addr(rs1_reg),  // Use registered decoder outputs for reads
                .rs2_addr(rs2_reg),  // Use registered decoder outputs for reads
                .rs3_addr(instruction[31:27]),  // rs3 field for fused multiply-add
                .rd_addr(rd_reg),
                .rd_data(fd_data),
                .rs1_data(fs1_data),
                .rs2_data(fs2_data),
                .rs3_data(fs3_data)
            );
            
            // FPU rounding mode selection
            // Instruction rm field (funct3) encodes:
            // 000=RNE, 001=RTZ, 010=RDN, 011=RUP, 100=RMM, 111=dynamic (use FCSR.frm)
            assign fpu_rm = (funct3_reg == 3'b111) ? fcsr[7:5] : funct3_reg;
            
            // FPU Module
            fpu u_fpu (
                .clk(clk),                              // NEW: Clock for multi-cycle division
                .rst_n(rst_n),                          // NEW: Reset for multi-cycle division
                .fpu_start(fpu_start),                  // NEW: Start FPU operation
                .fs1(int_to_fp_reg ? a_reg : fa_reg),   // Source 1: integer or FP
                .fs2(fb_reg),                           // Source 2: always FP
                .fs3(fc_reg),                           // Source 3: FP (for fused multiply-add)
                .int_src(a_reg),                        // Integer source (for int-to-FP conversions)
                .fpu_op(fpu_op_reg),
                .rm(fpu_rm),
                .fp_result(fpu_fp_result),
                .int_result(fpu_int_result),
                .fflags(fpu_fflags),
                .fpu_ready(fpu_ready)                   // NEW: FPU operation complete
            );
            
            // FCSR (Floating Point Control and Status Register)
            // Address: 0x003 (full FCSR), 0x001 (FFLAGS), 0x002 (FRM)
            // Bitfields: {24'h0, frm[2:0], fflags[4:0]}
            always_ff @(posedge clk) begin
                if (!rst_n) begin
                    fcsr <= 32'h0;  // Reset to default rounding mode (RNE) and no exceptions
                end else begin
                    // Accumulate exception flags when FP instruction completes
                    if (current_state == S_WRITEBACK && fp_reg_write_reg) begin
                        fcsr[4:0] <= fcsr[4:0] | fpu_fflags;  // OR in new exception flags
                    end
                    // Handle CSR writes to FCSR, FRM, FFLAGS
                    else if (is_csr_reg && current_state == S_CSR) begin
                        case (csr_addr)
                            12'h001: fcsr[4:0] <= a_reg[4:0];   // FFLAGS write
                            12'h002: fcsr[7:5] <= a_reg[2:0];   // FRM write
                            12'h003: fcsr <= a_reg;              // FCSR write (full register)
                            default: ; // No change
                        endcase
                    end
                end
            end
        end else begin : gen_no_f_ext
            // F extension disabled: Tie FP signals to safe defaults
            assign fs1_data = 32'd0;
            assign fs2_data = 32'd0;
            assign fs3_data = 32'd0;
            assign fpu_fp_result = 32'd0;
            assign fpu_int_result = 32'd0;
            assign fpu_fflags = 5'd0;
            assign fpu_ready = 1'b1;
            assign fpu_rm = 3'd0;
            assign fcsr = 32'd0;
        end
    endgenerate
    
    // Writeback Multiplexer Module (uses registered signals)
    writeback_mux u_writeback_mux (
        .opcode(opcode_reg),
        .jump(jump_reg),
        .is_csr(is_csr_reg),
        .mem_to_reg(mem_to_reg_reg),
        .is_lr(is_lr_reg),          // A extension
        .is_sc(is_sc_reg),          // A extension
        .is_amo(is_amo_reg),        // A extension
        .sc_success(sc_success),    // A extension
        .fp_to_int(fp_to_int_reg),  // F extension
        .imm_u(imm_u_reg),
        .alu_result(alu_out_reg),  // Use registered ALU output
        .csr_rdata(csr_rdata_reg),  // Use registered CSR read data (old value)
        .formatted_load_data(mdr),  // Use MDR (memory data register)
        .fpu_result(fpu_out_reg),   // F extension: FP-to-int result
        .rd_data(rd_data)
    );
    
    // FP Writeback Data Selection
    // For FP instructions, select between FP result, integer-to-FP result, and FP load
    always_comb begin
        if (fp_to_int_reg) begin
            // FP-to-integer operation: result goes to integer regfile (handled by writeback_mux via fpu_out_reg)
            fd_data = 32'h0;  // Not used
        end else if (is_fp_load_reg) begin
            // Load FP from memory (FLW) - use MDR
            fd_data = mdr;
        end else if (fp_reg_write_reg) begin
            // FP operation result goes to FP regfile
            fd_data = fpu_out_reg;
        end else begin
            // Default
            fd_data = 32'h0;
        end
    end
    
    // ============================================================
    // Memory Address Channel Multiplexing
    // ============================================================
    // In multi-cycle operation, instruction fetch (S_FETCH) and data access
    // (S_MEM_READ/S_MEM_WRITE/S_ATOMIC_RMW) never occur simultaneously.
    // This multiplexer routes either instruction or data signals to the
    // address channel based on FSM state.
    
    always_comb begin
        // Default: instruction fetch (S_FETCH state)
        if (imem_req_internal) begin
            // Instruction fetch: read from PC, no write
            mem_a_addr  = imem_addr_internal;
            mem_a_wdata = 32'h0;
            mem_a_we    = 1'b0;
            mem_a_size  = 2'b10; // Always word-sized for instructions
            mem_a_valid = !mem_req_inflight;
        end else if (dmem_req_internal) begin
            // Data access: use data memory signals
            mem_a_addr  = dmem_addr_internal;
            mem_a_wdata = dmem_wdata_internal;
            mem_a_we    = dmem_we_internal;
            mem_a_size  = dmem_size_internal;
            mem_a_valid = !mem_req_inflight;
        end else begin
            // No memory request: drive defaults
            mem_a_addr  = 32'h0;
            mem_a_wdata = 32'h0;
            mem_a_we    = 1'b0;
            mem_a_size  = 2'b00;
            mem_a_valid = 1'b0;
        end
    end
    
    // CPU accepts D-channel responses only after an A-channel request handshake
    assign mem_d_ready = mem_req_inflight;
    
    // Debug outputs for trace callback (use captured values at instruction completion)
    assign debug_rs1_data = trace_rs1_data_reg;
    assign debug_rs2_data = trace_rs2_data_reg;
    assign debug_rd_data = trace_rd_data_reg;
    
    // Debug outputs for instruction tracing (completed instruction)
    assign debug_pc = completed_pc_reg;
    assign debug_instruction = completed_instr_reg;
    
    // Debug outputs for current execution state (for hung detection)
    // Use instr_pc_reg in states after DECODE, otherwise use pc
    // This ensures we always have a valid PC that corresponds to the current instruction
    // instr_pc_reg is written during DECODE, so it's valid from EXECUTE onward
    assign debug_current_pc = (current_state > S_DECODE) ? instr_pc_reg : pc;
    assign debug_current_instruction = ir_reg;
    
    // Debug output for FSM state
    assign debug_fsm_state = current_state;
    
    // Boot state indicator
    assign is_booting = (current_state == S_BOOT);

endmodule

// Top-Level CPU Module
// Multi-cycle RISC-V RV32IMC processor with variable-latency memory support

module top (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction memory interface (exposed to testbench)
    output logic [31:0] imem_addr,
    input  logic [31:0] imem_data,
    output logic        imem_req,     // NEW: Request instruction fetch
    input  logic        imem_ready,   // NEW: Memory has valid data
    
    // Data memory interface (exposed to testbench)
    output logic [31:0] dmem_addr,
    output logic [31:0] dmem_wdata,
    input  logic [31:0] dmem_rdata,
    output logic        dmem_we,
    output logic        dmem_re,      // Memory read enable
    output logic [1:0]  dmem_size,    // Memory operation size: 00=byte, 01=halfword, 10=word
    output logic        dmem_req,     // NEW: Request data memory operation
    input  logic        dmem_ready,   // NEW: Memory operation complete
    
    // System control signals
    output logic        halted,       // CPU halted (ECALL/EBREAK)
    output logic        instr_complete, // NEW: High for 1 cycle when instruction done
    
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
    output logic [3:0]  debug_fsm_state   // Current FSM state (for debugging)
);

    // ============================================================
    // FSM State Definitions
    // ============================================================
    typedef enum logic [3:0] {
        S_IDLE       = 4'b0000,  // After reset
        S_FETCH      = 4'b0001,  // Fetch instruction (wait for imem_ready)
        S_DECODE     = 4'b0010,  // Decode and read registers
        S_EXECUTE    = 4'b0011,  // ALU operation
        S_MEM_ADDR   = 4'b0100,  // Calculate memory address
        S_MEM_READ   = 4'b0101,  // Load from memory (wait for dmem_ready)
        S_MEM_WRITE  = 4'b0110,  // Store to memory (wait for dmem_ready)
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
    
    // Decoder outputs (combinational - will be captured in registers)
    logic [6:0]  opcode;
    logic [4:0]  rd;
    logic [4:0]  rs1;
    logic [4:0]  rs2;
    logic [2:0]  funct3;
    logic [6:0]  funct7;
    logic [31:0] imm_i;
    logic [31:0] imm_s;
    logic [31:0] imm_b;
    logic [31:0] imm_u;
    logic [31:0] imm_j;
    logic [4:0]  alu_op;
    logic        alu_src;
    logic        reg_write;
    logic        mem_write;
    logic        mem_read;
    logic        mem_to_reg;
    logic        branch;
    logic        jump;
    logic        is_ecall;
    logic        is_ebreak;
    logic        is_fence;
    logic        is_csr;
    logic        is_lr;        // LR.W instruction (A extension)
    logic        is_sc;        // SC.W instruction (A extension)
    logic        is_amo;       // AMO instruction (A extension)
    logic [4:0]  funct5;       // For atomic operation type
    
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
    
    // Operand Registers
    logic [31:0] a_reg;  // rs1 data
    logic [31:0] b_reg;  // rs2 data
    logic a_reg_write, b_reg_write;
    
    // Result Registers
    logic [31:0] alu_out_reg;  // ALU output
    logic [31:0] mdr;          // Memory data register
    logic alu_out_write, mdr_write;
    
    // Decoder Output Registers (all control signals stored)
    logic [6:0]  opcode_reg;
    logic [4:0]  rd_reg, rs1_reg, rs2_reg;
    logic [2:0]  funct3_reg;
    logic [6:0]  funct7_reg;
    logic [31:0] imm_i_reg, imm_s_reg, imm_b_reg, imm_u_reg, imm_j_reg;
    logic [4:0]  alu_op_reg;
    logic        alu_src_reg, reg_write_reg, mem_write_reg, mem_read_reg;
    logic        mem_to_reg_reg, branch_reg, jump_reg;
    
    // PC for the current instruction (captured in DECODE)
    logic [31:0] instr_pc_reg;
    
    // Completed instruction registers (captured at instruction completion for tracing)
    logic [31:0] completed_pc_reg;
    logic [31:0] completed_instr_reg;
    logic        is_ecall_reg, is_ebreak_reg, is_fence_reg, is_csr_reg;
    logic        is_lr_reg, is_sc_reg, is_amo_reg;  // A extension registers
    logic [4:0]  funct5_reg;  // A extension - atomic operation type
    logic        decode_reg_write;
    
    // Debug trace data registers (capture operand values at instruction completion)
    logic [31:0] trace_rs1_data_reg;
    logic [31:0] trace_rs2_data_reg;
    logic [31:0] trace_rd_data_reg;
    
    // Control Signals
    logic        pc_write;
    logic        reg_write_en;
    logic        csr_rdata_write;  // Control signal to latch CSR read data
    
    // Register file signals
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] rd_data;
    
    // ALU signals
    logic [31:0] alu_a;
    logic [31:0] alu_b;
    logic [31:0] alu_result;
    logic        alu_zero;
    logic        alu_start;       // NEW: Start ALU operation
    logic        alu_ready;       // NEW: ALU operation complete
    logic        alu_start_sent;  // NEW: Track if start pulse has been sent (S_EXECUTE)
    logic        alu_start_sent_rmw;  // NEW: Track if start pulse has been sent (S_ATOMIC_RMW)
    
    // Branch/Jump logic
    logic        take_branch;
    
    // CSR signals
    logic [11:0] csr_addr;
    logic [31:0] csr_rdata;
    logic [31:0] csr_rdata_reg;  // Registered CSR read data (captured before write)
    
    // Memory interface signals
    logic [31:0] formatted_load_data;
    
    // A extension: SC success/failure logic
    logic        sc_success;
    assign sc_success = reservation_valid && (reservation_addr == alu_out_reg);
    
    // A extension: AMO write data selection
    // AMOSWAP uses rs2 directly, others use ALU result
    logic [31:0] amo_write_data;
    assign amo_write_data = (funct5_reg == 5'b00001) ? b_reg : alu_result;  // funct5==00001 is AMOSWAP
    
    // CSR address: use combinational imm_i in S_DECODE (for read), registered imm_i_reg in other states
    assign csr_addr = (current_state == S_DECODE) ? imm_i[11:0] : imm_i_reg[11:0];
    
    // ============================================================
    // State Register (Flip-Flop Based FSM)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            current_state <= S_IDLE;
        else
            current_state <= next_state;
    end
    
    // ============================================================
    // Staging Register Implementations (All Flip-Flops)
    // ============================================================
    
    // Instruction Register
    // Now stores the decompressed 32-bit instruction (expanded from 16-bit if compressed)
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            ir_reg <= 32'h0;
        else if (ir_write)
            ir_reg <= decomp_output;  // Use decompressed output
    end
    
    // Operand Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            a_reg <= 32'h0;
            b_reg <= 32'h0;
        end else begin
            if (a_reg_write) a_reg <= rs1_data;
            if (b_reg_write) b_reg <= rs2_data;
        end
    end
    
    // Result Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            alu_out_reg <= 32'h0;
            mdr <= 32'h0;
            csr_rdata_reg <= 32'h0;
        end else begin
            if (alu_out_write) alu_out_reg <= alu_result;
            if (mdr_write) mdr <= formatted_load_data;
            if (csr_rdata_write) csr_rdata_reg <= csr_rdata;
        end
    end
    
    // Decoder Output Registers
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            opcode_reg <= 7'h0;
            rd_reg <= 5'h0;
            rs1_reg <= 5'h0;
            rs2_reg <= 5'h0;
            funct3_reg <= 3'h0;
            funct7_reg <= 7'h0;
            imm_i_reg <= 32'h0;
            imm_s_reg <= 32'h0;
            imm_b_reg <= 32'h0;
            imm_u_reg <= 32'h0;
            imm_j_reg <= 32'h0;
            alu_op_reg <= 5'h0;
            alu_src_reg <= 1'b0;
            reg_write_reg <= 1'b0;
            mem_write_reg <= 1'b0;
            mem_read_reg <= 1'b0;
            mem_to_reg_reg <= 1'b0;
            branch_reg <= 1'b0;
            jump_reg <= 1'b0;
            is_ecall_reg <= 1'b0;
            is_ebreak_reg <= 1'b0;
            is_fence_reg <= 1'b0;
            is_csr_reg <= 1'b0;
            is_lr_reg <= 1'b0;
            is_sc_reg <= 1'b0;
            is_amo_reg <= 1'b0;
            funct5_reg <= 5'h0;
            instr_pc_reg <= 32'h0;
        end else if (decode_reg_write) begin
            opcode_reg <= opcode;
            rd_reg <= rd;
            rs1_reg <= rs1;
            rs2_reg <= rs2;
            funct3_reg <= funct3;
            funct7_reg <= funct7;
            imm_i_reg <= imm_i;
            imm_s_reg <= imm_s;
            imm_b_reg <= imm_b;
            imm_u_reg <= imm_u;
            imm_j_reg <= imm_j;
            alu_op_reg <= alu_op;
            alu_src_reg <= alu_src;
            reg_write_reg <= reg_write;
            mem_write_reg <= mem_write;
            mem_read_reg <= mem_read;
            mem_to_reg_reg <= mem_to_reg;
            branch_reg <= branch;
            jump_reg <= jump;
            is_ecall_reg <= is_ecall;
            is_ebreak_reg <= is_ebreak;
            is_fence_reg <= is_fence;
            is_csr_reg <= is_csr;
            is_lr_reg <= is_lr;
            is_sc_reg <= is_sc;
            is_amo_reg <= is_amo;
            funct5_reg <= funct5;
            instr_pc_reg <= pc;  // Capture PC of this instruction
        end
    end

    
    // Completed Instruction Registers (captured at instruction completion)
    // Capture when current_state is in a completion state AND we're about to leave it
    // Delayed instr_complete signal for proper trace timing
    // Capture happens on cycle N when instr_complete_internal goes high
    // Output port sees delayed version on cycle N+1 after values have settled
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            instr_complete <= 1'b0;
        else
            instr_complete <= instr_complete_internal;
    end
    
    // Capture completed instruction info when instruction finishes
    always_ff @(posedge clk or negedge rst_n) begin
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
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            alu_start_sent <= 1'b0;
        else if (current_state != S_EXECUTE)
            alu_start_sent <= 1'b0;  // Reset when leaving S_EXECUTE
        else if (alu_start)
            alu_start_sent <= 1'b1;  // Mark as sent after pulsing
    end
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            alu_start_sent_rmw <= 1'b0;
        else if (current_state != S_ATOMIC_RMW)
            alu_start_sent_rmw <= 1'b0;  // Reset when leaving S_ATOMIC_RMW
        else if (alu_start)
            alu_start_sent_rmw <= 1'b1;  // Mark as sent after pulsing
    end
    
    // ============================================================
    // LR/SC Reservation Tracking (A Extension)
    // ============================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            reservation_valid <= 1'b0;
            reservation_addr <= 32'h0;
        end else begin
            // Set reservation on LR.W completion (in S_MEM_READ with dmem_ready)
            if (is_lr_reg && current_state == S_MEM_READ && dmem_ready) begin
                reservation_valid <= 1'b1;
                reservation_addr <= alu_out_reg;  // Address from ALU (rs1 + 0)
            end
            // Clear reservation on SC.W (any SC, regardless of success)
            else if (is_sc_reg && current_state == S_MEM_WRITE && dmem_ready) begin
                reservation_valid <= 1'b0;
            end
            // Clear reservation on any write to the reserved address
            else if (dmem_we && reservation_valid && dmem_addr == reservation_addr) begin
                reservation_valid <= 1'b0;
            end
        end
    end
    
    // ============================================================
    // RV32C Fetch Buffer and Decompressor Module Instantiations
    // ============================================================
    
    // Instantiate fetch buffer module
    fetch_buffer u_fetch_buffer (
        .clk(clk),
        .rst_n(rst_n),
        .imem_data(imem_data),
        .imem_ready(imem_ready),
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
    logic [31:0] next_pc_value;
    
    always_comb begin
        next_pc_value = pc + pc_increment;  // Sequential: increment by 2 or 4 bytes
        
        if (current_state == S_BRANCH) begin
            if (take_branch)
                next_pc_value = (instr_pc_reg + imm_b_reg) & ~32'h1;  // Branch target (ensure halfword-aligned)
        end else if (current_state == S_WRITEBACK) begin
            if (opcode_reg == 7'b1101111)  // JAL
                next_pc_value = (instr_pc_reg + imm_j_reg) & ~32'h1;  // Jump target (ensure halfword-aligned)
            else if (opcode_reg == 7'b1100111)  // JALR
                next_pc_value = (a_reg + imm_i_reg) & ~32'h1;  // Already has alignment mask
        end
    end
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n)
            pc <= boot_addr;
        else if (pc_write)
            pc <= next_pc_value;
    end
    
    // Halted signal
    assign halted = (current_state == S_HALT);
    
    // ============================================================
    // Instruction Memory Address Assignment
    // ============================================================
    assign imem_addr = pc;
    assign instruction = ir_reg;  // Use registered instruction
    
    // ============================================================
    // FSM Next-State Logic
    // ============================================================
    always_comb begin
        next_state = current_state;
        
        case (current_state)
            S_IDLE: begin
                next_state = S_FETCH;
            end
            
            S_FETCH: begin
                // Wait for instruction memory ready
                if (imem_ready)
                    next_state = S_DECODE;
                else
                    next_state = S_FETCH;
            end
            
            S_DECODE: begin
                // Use combinational decoder outputs (opcode, not opcode_reg)
                // because decode_reg_write captures them THIS cycle
                case (opcode)
                    7'b0110011,  // R-type
                    7'b0010011,  // I-type arithmetic
                    7'b0110111,  // LUI
                    7'b0010111,  // AUIPC
                    7'b1101111,  // JAL
                    7'b1100111:  // JALR
                        next_state = S_EXECUTE;
                    
                    7'b0000011,  // Load
                    7'b0100011:  // Store
                        next_state = S_MEM_ADDR;
                    
                    7'b0101111:  // AMO (Atomic operations - A extension)
                        next_state = S_MEM_ADDR;
                    
                    7'b1100011:  // Branch
                        next_state = S_BRANCH;
                    
                    7'b1110011: begin  // SYSTEM
                        if (is_ecall || is_ebreak)
                            next_state = S_HALT;
                        else if (is_csr)
                            next_state = S_CSR;
                    end

                    7'b0001111:  // FENCE
                        next_state = S_FETCH;
                    
                    default: next_state = S_HALT;  // Unknown instruction - halt for debug
                endcase
            end
            
            S_EXECUTE: begin
                // Wait for ALU ready signal before proceeding
                if (alu_ready)
                    next_state = S_WRITEBACK;
                else
                    next_state = S_EXECUTE;  // Wait for multi-cycle division
            end
            
            S_MEM_ADDR: begin
                if (mem_read_reg)
                    next_state = S_MEM_READ;
                else
                    next_state = S_MEM_WRITE;
            end
            
            S_MEM_READ: begin
                // Wait for data memory ready
                if (dmem_ready) begin
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
                // Wait for data memory ready
                if (dmem_ready) begin
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
                // Wait for ALU ready and memory ready before writeback
                if (alu_ready && dmem_ready)
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
        alu_out_write = 1'b0;
        mdr_write = 1'b0;
        csr_rdata_write = 1'b0;
        pc_write = 1'b0;
        reg_write_en = 1'b0;
        decode_reg_write = 1'b0;
        imem_req = 1'b0;
        dmem_req = 1'b0;
        instr_complete_internal = 1'b0;
        alu_start = 1'b0;  // NEW: Default ALU start to inactive
        
        case (current_state)
            S_FETCH: begin
                imem_req = 1'b1;
                if (imem_ready)
                    ir_write = 1'b1;
            end
            
            S_DECODE: begin
                a_reg_write = 1'b1;
                b_reg_write = 1'b1;
                decode_reg_write = 1'b1;
                // Capture CSR read data before write (for read-modify-write operations)
                if (is_csr)
                    csr_rdata_write = 1'b1;
                // FENCE completes here
                if (is_fence) begin
                    pc_write = 1'b1;
                    instr_complete_internal = 1'b1;
                end
            end
            
            S_EXECUTE: begin
                // Pulse alu_start only on first cycle in S_EXECUTE
                alu_start = !alu_start_sent;
                
                if (alu_ready) begin
                    alu_out_write = 1'b1;
                end
                // No else needed - stays in S_EXECUTE until alu_ready (handled by next_state logic)
            end
            
            S_MEM_ADDR: begin
                alu_out_write = 1'b1;
            end
            
            S_MEM_READ: begin
                dmem_req = 1'b1;
                if (dmem_ready)
                    mdr_write = 1'b1;
            end
            
            S_MEM_WRITE: begin
                dmem_req = 1'b1;
                if (dmem_ready) begin
                    pc_write = 1'b1;
                    instr_complete_internal = 1'b1;
                end
            end
            
            S_WRITEBACK: begin
                reg_write_en = 1'b1;
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
                
                dmem_req = 1'b1;  // Request memory write
                
                if (alu_ready && dmem_ready) begin
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
        
        // Special case: assert instr_complete_internal when entering HALT from another state
        // This must be done AFTER the case statement to avoid being overridden
        // Special case: HALT state stays complete once entered
        // This ensures delayed instr_complete signal stays high for the Rust code to see
        if (current_state == S_HALT) begin
            instr_complete_internal = 1'b1;
        end
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
        .alu_zero(alu_zero),
        .take_branch(take_branch)
    );
    
    // Decoder instantiation (decodes ir_reg)
    decoder u_decoder (
        .instruction(ir_reg),
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
        .is_lr(is_lr),
        .is_sc(is_sc),
        .is_amo(is_amo),
        .funct5(funct5)
    );
    
    // Register file instantiation (write enable gated by FSM)
    regfile u_regfile (
        .clk(clk),
        .we(reg_write_en & reg_write_reg),  // Gated by FSM
        .rs1_addr(rs1),  // Use combinational decoder output for reads
        .rs2_addr(rs2),  // Use combinational decoder output for reads
        .rd_addr(rd_reg),
        .rd_data(rd_data),
        .rs1_data(rs1_data),
        .rs2_data(rs2_data)
    );
    
    // ALU source selection (multi-cycle: uses registered operands and control)
    always_comb begin
        // Default sources
        alu_a = a_reg;
        alu_b = alu_src_reg ? ((opcode_reg == 7'b0100011) ? imm_s_reg : imm_i_reg) : b_reg;
        
        // Special case for S_MEM_ADDR with AMO: address is just rs1 (no offset)
        if (current_state == S_MEM_ADDR && is_amo_reg) begin
            alu_a = a_reg;  // rs1
            alu_b = 32'h0;  // No offset for AMO
        end
        // Special cases in EXECUTE state
        else if (current_state == S_EXECUTE) begin
            case (opcode_reg)
                7'b0010111: begin // AUIPC
                    // Use the PC captured for this instruction at decode time
                    alu_a = instr_pc_reg;
                    alu_b = imm_u_reg;
                end
                7'b1101111, 7'b1100111: begin // JAL, JALR
                    // Compute PC+4 for return address using the instruction PC captured at decode
                    alu_a = instr_pc_reg;
                    alu_b = 32'd4;
                end
                default: begin
                    // Use default
                end
            endcase
        end
        // Special case for S_ATOMIC_RMW: compute new value for AMO
        else if (current_state == S_ATOMIC_RMW) begin
            alu_a = mdr;    // Original value from memory
            alu_b = b_reg;  // rs2 data
        end
    end
    
    // ALU instantiation (uses registered control signals)
    alu u_alu (
        .clk(clk),              // NEW: Clock for division unit
        .rst_n(rst_n),          // NEW: Reset for division unit
        .a(alu_a),
        .b(alu_b),
        .alu_op(alu_op_reg),
        .alu_start(alu_start),  // NEW: Start operation pulse
        .result(alu_result),
        .zero(alu_zero),
        .alu_ready(alu_ready)   // NEW: Operation complete
    );
    
    // Memory Interface Module (uses registered control signals)
    mem_interface u_mem_interface (
        .funct3(funct3_reg),
        .mem_write(mem_write_reg),
        .mem_read(mem_read_reg),
        .is_atomic_rmw(current_state == S_ATOMIC_RMW),  // A extension
        .is_sc(is_sc_reg),                               // A extension
        .sc_success(sc_success),                         // A extension
        .alu_result(alu_out_reg),  // Use registered ALU output for address
        .rs2_data(b_reg),           // Use registered rs2 data
        .dmem_rdata(dmem_rdata),
        .amo_wdata(amo_write_data),     // A extension: muxed AMO write data
        .dmem_addr(dmem_addr),
        .dmem_wdata(dmem_wdata),
        .dmem_we(dmem_we),
        .dmem_re(dmem_re),
        .dmem_size(dmem_size),
        .formatted_load_data(formatted_load_data)
    );
    
    // CSR File Module (uses registered signals)
    csr_file u_csr_file (
        .clk(clk),
        .rst_n(rst_n),
        .is_csr(is_csr_reg & (current_state == S_CSR)),  // Gated by FSM
        .funct3(funct3_reg),
        .rs1(rs1_reg),
        .csr_addr(csr_addr),
        .rs1_data(a_reg),  // Use registered rs1 data
        .csr_rdata(csr_rdata)
    );
    
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
        .pc(instr_pc_reg),
        .imm_u(imm_u_reg),
        .alu_result(alu_out_reg),  // Use registered ALU output
        .csr_rdata(csr_rdata_reg),  // Use registered CSR read data (old value)
        .formatted_load_data(mdr),  // Use MDR (memory data register)
        .rd_data(rd_data)
    );
    
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
    
    // DEBUG: Print atomic operation state transitions
    always_ff @(posedge clk) begin
        if (current_state == S_MEM_ADDR && is_amo_reg) begin
            $display("[MEM_ADDR] Time=%0t PC=0x%08h (AMO)", $time, instr_pc_reg);
            $display("  a_reg=0x%08h b_reg=0x%08h alu_src_reg=%b", a_reg, b_reg, alu_src_reg);
            $display("  imm_i_reg=0x%08h alu_result=0x%08h (address)", imm_i_reg, alu_result);
        end
        if (current_state == S_MEM_READ && is_lr_reg && dmem_ready) begin
            $display("[LR.W] Time=%0t PC=0x%08h", $time, instr_pc_reg);
            $display("  Setting reservation: addr=0x%08h", alu_out_reg);
        end
        if (current_state == S_MEM_WRITE && is_sc_reg) begin
            $display("[SC.W] Time=%0t PC=0x%08h", $time, instr_pc_reg);
            $display("  reservation_valid=%b reservation_addr=0x%08h", reservation_valid, reservation_addr);
            $display("  alu_out_reg=0x%08h sc_success=%b", alu_out_reg, sc_success);
        end
        if (current_state == S_MEM_READ && is_amo_reg && dmem_ready) begin
            $display("[MEM_READ] Time=%0t PC=0x%08h (AMO)", $time, instr_pc_reg);
            $display("  dmem_addr=0x%08h dmem_rdata=0x%08h", dmem_addr, dmem_rdata);
        end
        if (current_state == S_ATOMIC_RMW && !alu_start_sent_rmw) begin
            $display("[ATOMIC_RMW_START] Time=%0t PC=0x%08h funct5=0x%02h", $time, instr_pc_reg, funct5_reg);
            $display("  mdr=0x%08h (mem value)", mdr);
            $display("  b_reg=0x%08h (rs2 value)", b_reg);
            $display("  alu_out_reg=0x%08h (address)", alu_out_reg);
            $display("  mem_write_reg=%b dmem_we=%b", mem_write_reg, dmem_we);
        end
        if (current_state == S_ATOMIC_RMW && alu_ready) begin
            $display("[ATOMIC_RMW_READY] ALU ready: alu_result=0x%08h amo_write_data=0x%08h", alu_result, amo_write_data);
            $display("  dmem_addr=0x%08h dmem_wdata=0x%08h dmem_we=%b", dmem_addr, dmem_wdata, dmem_we);
        end
        if (current_state == S_ATOMIC_RMW && alu_ready && dmem_ready) begin
            $display("[ATOMIC_RMW_COMPLETE] Write completing");
        end
    end

endmodule

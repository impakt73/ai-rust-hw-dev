// System Controller Peripheral
// Controls CPU boot process, system reset, and CPU reset
// Provides CPU status monitoring via memory-mapped registers

module system_controller (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,

    // Address channel
    input  logic [31:0] mem_a_addr,
    input  logic [31:0] mem_a_wdata,
    input  logic        mem_a_we,
    input  logic [1:0]  mem_a_size,
    input  logic        mem_a_valid,
    output logic        mem_a_ready,

    // Data channel
    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input  logic        mem_d_ready,

    // System control outputs
    output logic        sys_rst,       // System reset output
    output logic        cpu_rst_n,     // CPU reset output (active low)
    output logic [31:0] cpu_boot_addr, // Boot address output to CPU
    output logic        cpu_boot,      // Boot signal output to CPU
    output logic        req_cpu_halt,  // Pulse to request CPU halt
    output logic [31:0] halted_value,  // Latched halt value register
    
    // CPU status inputs
    input  logic        cpu_halted,    // From CPU halted signal
    input  logic        cpu_booting    // From CPU is_booting signal
);

    // ========================================================================
    // Register Map (offsets from base address)
    // ========================================================================
    localparam logic [3:0] REG_STATUS = 4'h0;  // Read-only: CPU status
    localparam logic [3:0] REG_RESET  = 4'h4;  // Write-only: Reset control
    localparam logic [3:0] REG_BOOT   = 4'h8;  // Write-only: Boot address
    localparam logic [3:0] REG_HALT   = 4'hC;  // Read/write: Halt code
    
    // ========================================================================
    // Internal Registers
    // ========================================================================
    logic [31:0] boot_addr_reg;          // Stored boot address
    logic [31:0] halt_reg;               // Stored halt code
    logic        halt_pending;           // Hold halt request until CPU acknowledges
    logic        sys_reset_pending;      // Delayed system reset pulse
    logic [31:0] response_data;
    logic        response_pending;
    logic        mem_a_handshake;
    logic        mem_d_handshake;
    typedef enum logic [2:0] {
        OP_IDLE,
        OP_BOOT_WAIT,
        OP_CPU_RESET_WAIT_HALT,
        OP_CPU_RESET_ASSERT,
        OP_CPU_RESET_WAIT_BOOT
    } pending_op_t;
    pending_op_t pending_op;

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    assign mem_a_ready = (pending_op == OP_IDLE) && !response_pending;
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;

    // ========================================================================
    // Main Control Registers
    // ========================================================================
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            sys_rst       <= 1'b0;
            cpu_rst_n     <= 1'b1;
            cpu_boot      <= 1'b0;
            boot_addr_reg <= 32'h00000000;
            halt_reg      <= 32'h00000000;
            halt_pending  <= 1'b0;
            req_cpu_halt  <= 1'b0;
            sys_reset_pending <= 1'b0;
            response_data <= 32'h00000000;
            response_pending <= 1'b0;
            pending_op <= OP_IDLE;
        end else begin
            // Default inactive values every cycle; writes can pulse outputs high/low.
            sys_rst      <= sys_reset_pending;
            cpu_rst_n    <= 1'b1;
            cpu_boot     <= 1'b0;
            req_cpu_halt <= 1'b0;
            sys_reset_pending <= 1'b0;

            if (mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (halt_pending) begin
                if (cpu_halted) begin
                    halt_pending <= 1'b0;
                end else begin
                    req_cpu_halt <= 1'b1;
                end
            end

            case (pending_op)
                OP_BOOT_WAIT: begin
                    if (!cpu_booting) begin
                        response_data <= 32'h00000000;
                        response_pending <= 1'b1;
                        pending_op <= OP_IDLE;
                    end
                end

                OP_CPU_RESET_WAIT_HALT: begin
                    req_cpu_halt <= 1'b1;
                    if (cpu_halted) begin
                        pending_op <= OP_CPU_RESET_ASSERT;
                    end
                end

                OP_CPU_RESET_ASSERT: begin
                    cpu_rst_n <= 1'b0;
                    pending_op <= OP_CPU_RESET_WAIT_BOOT;
                end

                OP_CPU_RESET_WAIT_BOOT: begin
                    if (cpu_booting) begin
                        response_data <= 32'h00000000;
                        response_pending <= 1'b1;
                        pending_op <= OP_IDLE;
                    end
                end

                default: begin
                    pending_op <= OP_IDLE;
                end
            endcase

            if (mem_a_handshake) begin
                if (mem_a_we) begin
                    case (mem_a_addr[3:0])  // Use only register offset bits
                        REG_BOOT: begin
                            // BOOT writes are accepted independently of cpu_booting state.
                            boot_addr_reg <= mem_a_wdata;
                            cpu_boot      <= 1'b1;
                            pending_op    <= OP_BOOT_WAIT;
                        end

                        REG_RESET: begin
                            if (mem_a_wdata[0]) begin
                                if (cpu_booting) begin
                                    pending_op <= OP_CPU_RESET_ASSERT;
                                end else begin
                                    req_cpu_halt <= 1'b1;
                                    pending_op <= OP_CPU_RESET_WAIT_HALT;
                                end
                            end else begin
                                sys_reset_pending <= 1'b1;
                            end
                        end

                        REG_HALT: begin
                            halt_reg          <= mem_a_wdata;
                            response_data     <= 32'h00000000;
                            response_pending  <= 1'b1;
                            req_cpu_halt      <= 1'b1;
                            halt_pending      <= 1'b1;
                        end

                        default: begin
                            response_data <= 32'h00000000;
                            response_pending <= 1'b1;
                        end
                    endcase
                end else begin
                    response_pending <= 1'b1;
                    case (mem_a_addr[3:0])
                        REG_STATUS: begin
                            // Bit 0 = cpu_booting, Bit 1 = cpu_halted
                            response_data <= {30'h0, cpu_halted, cpu_booting};
                        end

                        REG_HALT: begin
                            response_data <= halt_reg;
                        end

                        // RESET and BOOT registers are write-only, read as 0
                        default: begin
                            response_data <= 32'h00000000;
                        end
                    endcase
                end
            end
        end
    end
    
    assign cpu_boot_addr = boot_addr_reg;
    assign halted_value  = halt_reg;
    
    // ========================================================================
    logic [1:0] unused_size;
    assign unused_size = mem_a_size;

endmodule

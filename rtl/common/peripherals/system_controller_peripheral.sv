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
    output logic        req_cpu_halt,  // Halt request output to CPU
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

    typedef enum logic [1:0] {
        CPU_RESET_IDLE,
        CPU_RESET_WAIT_HALT,
        CPU_RESET_PULSE,
        CPU_RESET_WAIT_BOOT
    } cpu_reset_state_t;
    
    // ========================================================================
    // Internal Registers
    // ========================================================================
    logic [31:0] boot_addr_reg;          // Stored boot address
    logic [31:0] halt_reg;               // Stored halt code
    logic        sys_reset_pending;      // Delayed system reset pulse
    logic        halt_response_pending;   // Hold HALT req_cpu_halt until response completes
    logic [31:0] response_data;
    logic        response_pending;
    logic        mem_a_handshake;
    logic        mem_d_handshake;
    logic        cpu_reset_in_progress;
    logic        start_cpu_reset;
    cpu_reset_state_t cpu_reset_state;
    cpu_reset_state_t cpu_reset_state_next;

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    assign start_cpu_reset =
        mem_a_handshake && mem_a_we && (mem_a_addr[3:0] == REG_RESET) && mem_a_wdata[0];
    assign cpu_reset_in_progress = (cpu_reset_state != CPU_RESET_IDLE);
    assign mem_a_ready = !response_pending && !cpu_reset_in_progress;
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;

    always_comb begin
        cpu_reset_state_next = cpu_reset_state;

        case (cpu_reset_state)
            CPU_RESET_IDLE: begin
                if (start_cpu_reset)
                    cpu_reset_state_next = CPU_RESET_WAIT_HALT;
            end

            CPU_RESET_WAIT_HALT: begin
                if (cpu_halted)
                    cpu_reset_state_next = CPU_RESET_PULSE;
            end

            CPU_RESET_PULSE: begin
                cpu_reset_state_next = CPU_RESET_WAIT_BOOT;
            end

            CPU_RESET_WAIT_BOOT: begin
                if (cpu_booting)
                    cpu_reset_state_next = CPU_RESET_IDLE;
            end

            default: begin
                cpu_reset_state_next = CPU_RESET_IDLE;
            end
        endcase
    end

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
            req_cpu_halt  <= 1'b0;
            sys_reset_pending <= 1'b0;
            halt_response_pending <= 1'b0;
            response_data <= 32'h00000000;
            response_pending <= 1'b0;
            cpu_reset_state <= CPU_RESET_IDLE;
        end else begin
            // Default inactive values every cycle; writes/state machine can pulse outputs.
            sys_rst      <= sys_reset_pending;
            cpu_rst_n    <= 1'b1;
            cpu_boot     <= 1'b0;
            req_cpu_halt <= 1'b0;
            sys_reset_pending <= 1'b0;
            cpu_reset_state <= cpu_reset_state_next;

            case (cpu_reset_state)
                CPU_RESET_WAIT_HALT: begin
                    req_cpu_halt <= 1'b1;
                end

                CPU_RESET_PULSE: begin
                    cpu_rst_n <= 1'b0;
                end

                default: begin
                end
            endcase

            // HALT register writes are a separate short-lived request path: they
            // must keep req_cpu_halt asserted only until their D-channel response
            // is consumed, unlike RESET_CPU which is sequenced by cpu_reset_state.
            if (halt_response_pending && !mem_d_handshake) begin
                req_cpu_halt <= 1'b1;
            end

            if (mem_d_handshake) begin
                response_pending <= 1'b0;
                halt_response_pending <= 1'b0;
            end

            if (mem_a_handshake) begin
                response_pending <= 1'b1;
                response_data <= 32'h00000000;

                if (mem_a_we) begin
                    case (mem_a_addr[3:0])  // Use only register offset bits
                        REG_BOOT: begin
                            // BOOT writes are accepted independently of cpu_booting state.
                            boot_addr_reg <= mem_a_wdata;
                            cpu_boot      <= 1'b1;
                        end

                        REG_RESET: begin
                            if (mem_a_wdata[0]) begin
                                req_cpu_halt <= 1'b1;
                            end else begin
                                sys_reset_pending <= 1'b1;
                            end
                        end

                        REG_HALT: begin
                            halt_reg      <= mem_a_wdata;
                            req_cpu_halt  <= 1'b1;
                            halt_response_pending <= 1'b1;
                        end

                        default: ;
                    endcase
                end else begin
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

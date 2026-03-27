`default_nettype none
// System Controller Peripheral
// Controls CPU boot process, system reset, and CPU reset
// Provides CPU status monitoring via memory-mapped registers

module system_controller #(
    parameter int CLK_FREQ_HZ = 1_000_000
) (
    // Clock and reset
    input wire logic        clk,
    input wire logic        rst,

    // Address channel
    input wire logic [31:0] mem_a_addr,
    input wire logic [31:0] mem_a_wdata,
    input wire logic        mem_a_we,
    input wire logic [1:0]  mem_a_size,
    input wire logic        mem_a_valid,
    output logic        mem_a_ready,

    // Data channel
    output logic [31:0] mem_d_rdata,
    output logic        mem_d_valid,
    input wire logic        mem_d_ready,

    // System control outputs
    output logic        sys_rst,       // System reset output
    output logic        cpu_rst,       // CPU reset output (active high)
    output logic [31:0] cpu_boot_addr, // Boot address output to CPU
    output logic        cpu_boot,      // Boot signal output to CPU
    output logic        req_cpu_halt,  // Pulse to request CPU halt
    output logic [31:0] halted_value,  // Latched halt value register
    output logic [7:0]  led_out,       // User LED output register
    
    // CPU status inputs
    input wire logic        cpu_halted,    // From CPU halted signal
    input wire logic        cpu_booting    // From CPU is_booting signal
);

    // ========================================================================
    // Register Map (offsets from base address)
    // ========================================================================
    localparam logic [4:0] REG_STATUS     = 5'h00;  // Read-only: CPU status
    localparam logic [4:0] REG_RESET      = 5'h04;  // Write-only: Reset control
    localparam logic [4:0] REG_BOOT       = 5'h08;  // Write-only: Boot address
    localparam logic [4:0] REG_HALT       = 5'h0C;  // Read/write: Halt code
    localparam logic [4:0] REG_LED_OUT    = 5'h10;  // Read/write: LED output
    localparam logic [4:0] REG_ELAPSED_US = 5'h14;  // Read-only: Elapsed microseconds
    localparam logic [4:0] REG_ELAPSED_MS = 5'h18;  // Read-only: Elapsed milliseconds
    localparam logic [4:0] REG_ELAPSED_S  = 5'h1C;  // Read-only: Elapsed seconds
    
    // ========================================================================
    // Internal Registers
    // ========================================================================
    logic [31:0] boot_addr_reg;          // Stored boot address
    logic [31:0] halt_reg;               // Stored halt code
    logic [7:0]  led_out_reg;            // Stored LED output
    logic        sys_reset_pending;      // Delayed system reset pulse
    logic [31:0] response_data;
    logic        response_pending;
    logic        mem_a_handshake;
    logic        mem_d_handshake;
    localparam int CYCLES_PER_US = (CLK_FREQ_HZ >= 1_000_000) ? (CLK_FREQ_HZ / 1_000_000) : 1;
    localparam int CYCLE_COUNTER_WIDTH = (CYCLES_PER_US > 1) ? $clog2(CYCLES_PER_US) : 1;
    logic [CYCLE_COUNTER_WIDTH-1:0] cycle_counter;
    logic [9:0] us_sub_counter;
    logic [9:0] ms_sub_counter;
    logic [31:0] elapsed_us;
    logic [31:0] elapsed_ms;
    logic [31:0] elapsed_s;
    logic microsecond_elapsed;
    logic millisecond_elapsed;
    logic second_elapsed;

    initial begin
        if (CLK_FREQ_HZ < 1_000_000) begin
            $fatal(
                1,
                "system_controller: CLK_FREQ_HZ (%0d Hz) must be >= 1_000_000 Hz for valid elapsed time counters.",
                CLK_FREQ_HZ
            );
        end
    end

    typedef enum logic [1:0] {
        CPU_RESET_IDLE      = 2'b00,
        CPU_RESET_WAIT_HALT = 2'b01,
        CPU_RESET_PULSE     = 2'b10,
        CPU_RESET_WAIT_BOOT = 2'b11
    } cpu_reset_state_t;

    cpu_reset_state_t cpu_reset_state;

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    assign mem_a_ready = !response_pending && (cpu_reset_state == CPU_RESET_IDLE);
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;
    assign led_out = led_out_reg;

    generate
        if (CYCLES_PER_US == 1) begin : gen_1mhz
            assign microsecond_elapsed = 1'b1;

            always_ff @(posedge clk) begin
                if (rst) begin
                    cycle_counter <= '0;
                end else begin
                    cycle_counter <= '0;
                end
            end
        end else begin : gen_high_freq
            always_ff @(posedge clk) begin
                if (rst) begin
                    cycle_counter <= '0;
                end else if (cycle_counter >= CYCLE_COUNTER_WIDTH'(CYCLES_PER_US - 1)) begin
                    cycle_counter <= '0;
                end else begin
                    cycle_counter <= cycle_counter + 1'b1;
                end
            end

            assign microsecond_elapsed = (cycle_counter >= CYCLE_COUNTER_WIDTH'(CYCLES_PER_US - 1));
        end
    endgenerate

    always_ff @(posedge clk) begin
        if (rst) begin
            elapsed_us <= 32'h0;
        end else if (microsecond_elapsed) begin
            elapsed_us <= elapsed_us + 32'h1;
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            us_sub_counter <= 10'h0;
        end else if (microsecond_elapsed) begin
            if (us_sub_counter >= 10'd999) begin
                us_sub_counter <= 10'h0;
            end else begin
                us_sub_counter <= us_sub_counter + 10'h1;
            end
        end
    end

    assign millisecond_elapsed = microsecond_elapsed && (us_sub_counter >= 10'd999);

    always_ff @(posedge clk) begin
        if (rst) begin
            elapsed_ms <= 32'h0;
        end else if (millisecond_elapsed) begin
            elapsed_ms <= elapsed_ms + 32'h1;
        end
    end

    always_ff @(posedge clk) begin
        if (rst) begin
            ms_sub_counter <= 10'h0;
        end else if (millisecond_elapsed) begin
            if (ms_sub_counter >= 10'd999) begin
                ms_sub_counter <= 10'h0;
            end else begin
                ms_sub_counter <= ms_sub_counter + 10'h1;
            end
        end
    end

    assign second_elapsed = millisecond_elapsed && (ms_sub_counter >= 10'd999);

    always_ff @(posedge clk) begin
        if (rst) begin
            elapsed_s <= 32'h0;
        end else if (second_elapsed) begin
            elapsed_s <= elapsed_s + 32'h1;
        end
    end

    // ========================================================================
    // Main Control Registers
    // ========================================================================
    always_ff @(posedge clk) begin
        if (rst) begin
            sys_rst       <= 1'b0;
            cpu_rst       <= 1'b0;
            cpu_boot      <= 1'b0;
            boot_addr_reg <= 32'h00000000;
            halt_reg      <= 32'h00000000;
            led_out_reg   <= 8'h00;
            req_cpu_halt  <= 1'b0;
            sys_reset_pending <= 1'b0;
            response_pending <= 1'b0;
            cpu_reset_state <= CPU_RESET_IDLE;
        end else begin
            // Default inactive values every cycle; writes can pulse outputs high/low.
            sys_rst      <= sys_reset_pending;
            cpu_rst      <= 1'b0;
            cpu_boot     <= 1'b0;
            req_cpu_halt <= 1'b0;
            sys_reset_pending <= 1'b0;

            if (mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            case (cpu_reset_state)
                CPU_RESET_IDLE: begin
                    if (mem_a_handshake) begin
                        response_data <= 32'h00000000;

                        if (mem_a_we) begin
                            case (mem_a_addr[4:0])  // Use only register offset bits
                                REG_BOOT: begin
                                    // BOOT writes are accepted independently of cpu_booting state.
                                    boot_addr_reg <= mem_a_wdata;
                                    cpu_boot      <= 1'b1;
                                    response_pending <= 1'b1;
                                end

                                REG_RESET: begin
                                    if (mem_a_wdata[0]) begin
                                        req_cpu_halt <= 1'b1;
                                        cpu_reset_state <= CPU_RESET_WAIT_HALT;
                                    end else begin
                                        sys_reset_pending <= 1'b1;
                                        response_pending <= 1'b1;
                                    end
                                end

                                REG_HALT: begin
                                    halt_reg      <= mem_a_wdata;
                                    req_cpu_halt  <= 1'b1;
                                    response_pending <= 1'b1;
                                end

                                REG_LED_OUT: begin
                                    case (mem_a_size)
                                        2'b00: begin
                                            if (mem_a_addr[1:0] == 2'b00) begin
                                                led_out_reg <= mem_a_wdata[7:0];
                                            end
                                        end
                                        2'b01: begin
                                            if (mem_a_addr[1:0] == 2'b00) begin
                                                led_out_reg <= mem_a_wdata[7:0];
                                            end
                                        end
                                        2'b10: begin
                                            led_out_reg <= mem_a_wdata[7:0];
                                        end
                                        default: begin
                                        end
                                    endcase
                                    response_pending <= 1'b1;
                                end

                                default: begin
                                    response_pending <= 1'b1;
                                end
                            endcase
                        end else begin
                            response_pending <= 1'b1;

                            case (mem_a_addr[4:0])
                                REG_STATUS: begin
                                    // Bit 0 = cpu_booting, Bit 1 = cpu_halted
                                    response_data <= {30'h0, cpu_halted, cpu_booting};
                                end

                                REG_HALT: begin
                                    response_data <= halt_reg;
                                end

                                REG_LED_OUT: begin
                                    response_data <= {24'h0, led_out_reg};
                                end

                                REG_ELAPSED_US: begin
                                    response_data <= elapsed_us;
                                end

                                REG_ELAPSED_MS: begin
                                    response_data <= elapsed_ms;
                                end

                                REG_ELAPSED_S: begin
                                    response_data <= elapsed_s;
                                end

                                // RESET and BOOT registers are write-only, read as 0
                                default: begin
                                    response_data <= 32'h00000000;
                                end
                            endcase
                        end
                    end
                end

                CPU_RESET_WAIT_HALT: begin
                    req_cpu_halt <= 1'b1;
                    if (cpu_halted) begin
                        cpu_reset_state <= CPU_RESET_PULSE;
                    end
                end

                CPU_RESET_PULSE: begin
                    cpu_rst <= 1'b1;
                    cpu_reset_state <= CPU_RESET_WAIT_BOOT;
                end

                CPU_RESET_WAIT_BOOT: begin
                    if (cpu_booting) begin
                        response_data <= 32'h00000000;
                        response_pending <= 1'b1;
                        cpu_reset_state <= CPU_RESET_IDLE;
                    end
                end

                default: begin
                    cpu_reset_state <= CPU_RESET_IDLE;
                end
            endcase
        end
    end
    
    assign cpu_boot_addr = boot_addr_reg;
    assign halted_value  = halt_reg;
    
endmodule
`default_nettype wire

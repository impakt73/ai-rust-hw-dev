// System Controller Peripheral
// Controls CPU boot process, system reset, CPU reset, and system LEDs
// Provides CPU status monitoring via memory-mapped registers

module system_controller (
    // Clock and reset
    input  logic        clk,
    input  logic        rst_n,
    
    // CPU interface (memory-mapped)
    input  logic [31:0] addr,      // Address input (full 32-bit)
    input  logic [31:0] wdata,     // Write data
    output logic [31:0] rdata,     // Read data
    input  logic        we,        // Write enable
    input  logic        req,       // Memory request
    input  logic [1:0]  size,      // Access size (00=byte, 01=half, 10=word)
    output logic        ready,     // Operation complete (always ready)
    
    // System control outputs
    output logic        sys_rst,       // System reset output
    output logic        cpu_rst_n,     // CPU reset output (active low)
    output logic [31:0] cpu_boot_addr, // Boot address output to CPU
    output logic        cpu_boot,      // Boot signal output to CPU
    output logic [7:0]  sys_led,       // LED output for external LEDs
    
    // CPU status inputs
    input  logic        cpu_halted,    // From CPU halted signal
    input  logic        cpu_booting    // From CPU is_booting signal
);

    // ========================================================================
    // Register Map (offsets from base address)
    // ========================================================================
    localparam [31:0] REG_STATUS = 32'h00;  // Read-only: CPU status
    localparam [31:0] REG_RESET  = 32'h04;  // Write-only: Reset control
    localparam [31:0] REG_BOOT   = 32'h08;  // Write-only: Boot address
    
    // Reset control values
    localparam [31:0] RESET_SYSTEM = 32'h00000001;  // System reset
    localparam [31:0] RESET_CPU    = 32'h00000002;  // CPU reset
    
    // ========================================================================
    // FSM States (One-Hot Encoding)
    // ========================================================================
    localparam logic [4:0] S_CPU_BOOT_WAIT = 5'b00001;  // Wait for boot
    localparam logic [4:0] S_CPU_BOOT      = 5'b00010;  // Assert cpu_boot
    localparam logic [4:0] S_IDLE          = 5'b00100;  // Normal operation
    localparam logic [4:0] S_SYS_RESET     = 5'b01000;  // System reset
    localparam logic [4:0] S_CPU_RESET     = 5'b10000;  // CPU reset
    
    // ========================================================================
    // Internal Registers
    // ========================================================================
    logic [4:0]  state_reg;              // Current state
    logic [31:0] boot_addr_reg;          // Stored boot address
    logic        sys_reset_trigger;      // System reset request flag
    logic        cpu_reset_trigger;      // CPU reset request flag
    
    // ========================================================================
    // System controller is single-cycle - always ready
    // ========================================================================
    assign ready = 1'b1;
    
    // ========================================================================
    // Write Decode Logic - Detect writes to control registers
    // ========================================================================
    logic write_boot;
    logic write_reset;
    
    always_comb begin
        write_boot  = 1'b0;
        write_reset = 1'b0;
        
        if (req && we) begin
            case (addr[7:0])  // Use lower 8 bits for register decode
                REG_BOOT[7:0]:  write_boot  = 1'b1;
                REG_RESET[7:0]: write_reset = 1'b1;
                default: ;
            endcase
        end
    end
    
    // ========================================================================
    // Reset Trigger Flags - Set on write, cleared by FSM
    // ========================================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            sys_reset_trigger <= 1'b0;
            cpu_reset_trigger <= 1'b0;
        end else begin
            // Set flags on write to RESET register
            if (write_reset) begin
                if (wdata == RESET_SYSTEM) begin
                    sys_reset_trigger <= 1'b1;
                end else if (wdata == RESET_CPU) begin
                    cpu_reset_trigger <= 1'b1;
                end
            end
            
            // Clear flags when entering respective reset states
            if (state_reg == S_SYS_RESET) begin
                sys_reset_trigger <= 1'b0;
            end
            if (state_reg == S_CPU_RESET) begin
                cpu_reset_trigger <= 1'b0;
            end
        end
    end
    
    // ========================================================================
    // Boot Address Register - Capture on write to BOOT in S_CPU_BOOT_WAIT
    // ========================================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            boot_addr_reg <= 32'h00000000;
        end else begin
            // Capture boot address when written in CPU_BOOT_WAIT state
            if (state_reg == S_CPU_BOOT_WAIT && write_boot && cpu_booting) begin
                boot_addr_reg <= wdata;
            end
        end
    end
    
    // ========================================================================
    // FSM State Register
    // ========================================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state_reg <= S_CPU_BOOT_WAIT;
        end else begin
            case (state_reg)
                S_CPU_BOOT_WAIT: begin
                    // Wait for cpu_booting AND write to BOOT register
                    if (cpu_booting && write_boot) begin
                        state_reg <= S_CPU_BOOT;
                    end
                end
                
                S_CPU_BOOT: begin
                    // Assert boot signal for one cycle, then go to idle
                    state_reg <= S_IDLE;
                end
                
                S_IDLE: begin
                    // Check for reset requests
                    if (sys_reset_trigger) begin
                        state_reg <= S_SYS_RESET;
                    end else if (cpu_reset_trigger) begin
                        state_reg <= S_CPU_RESET;
                    end
                end
                
                S_SYS_RESET: begin
                    // Stay here forever (external reset will reset module)
                    state_reg <= S_SYS_RESET;
                end
                
                S_CPU_RESET: begin
                    // Assert CPU reset for one cycle, then back to boot wait
                    state_reg <= S_CPU_BOOT_WAIT;
                end
                
                default: begin
                    // Safety: return to boot wait
                    state_reg <= S_CPU_BOOT_WAIT;
                end
            endcase
        end
    end
    
    // ========================================================================
    // Output Control Logic - Combinational from FSM state
    // ========================================================================
    always_comb begin
        // Default values
        sys_rst       = 1'b0;
        cpu_rst_n     = 1'b1;
        cpu_boot      = 1'b0;
        cpu_boot_addr = boot_addr_reg;
        
        case (state_reg)
            S_CPU_BOOT_WAIT: begin
                cpu_rst_n = 1'b0;  // Hold CPU in reset
            end
            
            S_CPU_BOOT: begin
                cpu_boot  = 1'b1;  // Assert boot signal
                cpu_rst_n = 1'b1;  // Release CPU from reset
            end
            
            S_IDLE: begin
                // Normal operation - all resets deasserted
            end
            
            S_SYS_RESET: begin
                sys_rst = 1'b1;    // Assert system reset
            end
            
            S_CPU_RESET: begin
                cpu_rst_n = 1'b0;  // Assert CPU reset
            end
            
            default: begin
                cpu_rst_n = 1'b0;  // Safe default: keep CPU in reset
            end
        endcase
    end
    
    // ========================================================================
    // System LED Control Logic - Registered for clean external timing
    // ========================================================================
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            sys_led <= 8'h00;
        end else if (cpu_halted) begin
            sys_led <= 8'hFF;       // All LEDs on when halted
        end else if (cpu_booting) begin
            sys_led <= 8'h01;       // Bit 0 on when booting
        end else begin
            sys_led <= 8'h00;       // All LEDs off otherwise
        end
    end
    
    // ========================================================================
    // Read Logic - Combinational
    // ========================================================================
    always_comb begin
        rdata = 32'h00000000;
        
        if (req && !we) begin
            case (addr[7:0])
                REG_STATUS[7:0]: begin
                    // Bit 0 = cpu_booting, Bit 1 = cpu_halted
                    rdata = {30'h0, cpu_halted, cpu_booting};
                end
                
                // RESET and BOOT registers are write-only, read as 0
                default: begin
                    rdata = 32'h00000000;
                end
            endcase
        end
    end

endmodule

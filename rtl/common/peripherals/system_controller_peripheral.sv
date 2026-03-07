// System Controller Peripheral
// Controls CPU boot process, system reset, and CPU reset
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
    
    // Reset control values
    localparam [31:0] RESET_SYSTEM = 32'h00000001;  // System reset
    localparam [31:0] RESET_CPU    = 32'h00000002;  // CPU reset
    
    // ========================================================================
    // Internal Registers
    // ========================================================================
    logic [31:0] boot_addr_reg;          // Stored boot address
    logic [31:0] halt_reg;               // Stored halt code
    logic        sys_reset_pending;      // Delayed system reset pulse
    
    // ========================================================================
    // System controller is single-cycle - always ready
    // ========================================================================
    assign ready = 1'b1;
    
    // ========================================================================
    // Write Decode Logic - Detect writes to control registers
    // ========================================================================
    logic write_boot;
    logic write_reset;
    logic write_halt;
    
    always_comb begin
        write_boot  = 1'b0;
        write_reset = 1'b0;
        write_halt  = 1'b0;
        
        if (req && we) begin
            case (addr[3:0])  // Use only register offset bits
                REG_BOOT:  write_boot  = 1'b1;
                REG_RESET: write_reset = 1'b1;
                REG_HALT:  write_halt  = 1'b1;
                default: ;
            endcase
        end
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
        end else begin
            // Default inactive values every cycle; writes can pulse outputs high/low.
            sys_rst      <= sys_reset_pending;
            cpu_rst_n    <= 1'b1;
            cpu_boot     <= 1'b0;
            req_cpu_halt <= 1'b0;
            sys_reset_pending <= 1'b0;

            if (write_reset) begin
                if (wdata == RESET_SYSTEM) begin
                    sys_reset_pending <= 1'b1;
                end else if (wdata == RESET_CPU) begin
                    cpu_rst_n <= 1'b0;
                end
            end

            if (write_boot) begin
                // BOOT writes are accepted independently of cpu_booting state.
                boot_addr_reg <= wdata;
                cpu_boot      <= 1'b1;
            end

            if (write_halt) begin
                halt_reg      <= wdata;
                req_cpu_halt  <= 1'b1;
            end
        end
    end
    
    assign cpu_boot_addr = boot_addr_reg;
    assign halted_value  = halt_reg;
    
    // ========================================================================
    // Read Logic - Combinational
    // ========================================================================
    always_comb begin
        rdata = 32'h00000000;
        
        if (req && !we) begin
            case (addr[3:0])
                REG_STATUS: begin
                    // Bit 0 = cpu_booting, Bit 1 = cpu_halted
                    rdata = {30'h0, cpu_halted, cpu_booting};
                end

                REG_HALT: begin
                    rdata = halt_reg;
                end
                
                // RESET and BOOT registers are write-only, read as 0
                default: begin
                    rdata = 32'h00000000;
                end
            endcase
        end
    end

    // ========================================================================
    // Suppress warnings for unused signals
    // ========================================================================
    logic [1:0] unused_size;
    assign unused_size = size;

endmodule

// LED Controller Peripheral
// Simple 8-bit LED output controller
// Memory-mapped at 0x50000000 in RTL peripheral address space

module led_controller_peripheral (
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
    
    // LED outputs
    output logic [7:0]  led_out    // LED outputs (to FPGA pins)
);

    // LED output register (only register in this peripheral)
    logic [7:0] led_out_reg;
    
    // LED controller is single-cycle - always ready
    assign ready = 1'b1;
    
    // LED output is driven by register
    assign led_out = led_out_reg;
    
    // Register file with reset
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            led_out_reg <= 8'h00;  // All LEDs off on reset
        end else if (we) begin
            // Handle different access sizes with byte lane masking
            case (size)
                2'b00: begin  // Byte access
                    case (addr[1:0])
                        2'b00: led_out_reg <= wdata[7:0];
                        // Bytes 1-3 are reserved, no effect
                        default: ;
                    endcase
                end
                2'b01: begin  // Halfword access
                    if (addr[1:0] == 2'b00) begin
                        led_out_reg <= wdata[7:0];
                        // Upper 8 bits of halfword are reserved
                    end
                end
                2'b10: begin  // Word access
                    led_out_reg <= wdata[7:0];
                    // Bits [31:8] are reserved, ignored
                end
                default: ;
            endcase
        end
    end
    
    // Read logic - combinational
    // Read occurs when req is asserted and we is not (read intent implied)
    always_comb begin
        rdata = 32'h0;
        
        if (req && !we) begin
            // Always return LED_OUT value in lower 8 bits
            // Upper 24 bits read as 0
            rdata = {24'h0, led_out_reg};
        end
    end

endmodule

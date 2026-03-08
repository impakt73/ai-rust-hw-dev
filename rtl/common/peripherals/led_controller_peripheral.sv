// LED Controller Peripheral
// Simple 8-bit LED output controller
// Memory-mapped at 0x50000000 in RTL peripheral address space

module led_controller_peripheral (
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

    // LED outputs
    output logic [7:0]  led_out    // LED outputs (to FPGA pins)
);

    // LED output register (only register in this peripheral)
    logic [7:0] led_out_reg;
    logic [31:0] response_data;
    logic        response_pending;
    logic        mem_a_handshake;
    logic        mem_d_handshake;

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;
    assign mem_a_ready = !response_pending;
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;

    // LED output is driven by register
    assign led_out = led_out_reg;

    // Register file with reset
    always_ff @(posedge clk) begin
        if (!rst_n) begin
            led_out_reg <= 8'h00;  // All LEDs off on reset
            response_data <= 32'h0;
            response_pending <= 1'b0;
        end else begin
            if (mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            if (mem_a_handshake) begin
                response_pending <= 1'b1;

                if (mem_a_we) begin
                    response_data <= 32'h0;

                    // Handle different access sizes with byte lane masking
                    case (mem_a_size)
                        2'b00: begin  // Byte access
                            case (mem_a_addr[1:0])
                                2'b00: led_out_reg <= mem_a_wdata[7:0];
                                // Bytes 1-3 are reserved, no effect
                                default: ;
                            endcase
                        end
                        2'b01: begin  // Halfword access
                            if (mem_a_addr[1:0] == 2'b00) begin
                                led_out_reg <= mem_a_wdata[7:0];
                                // Upper 8 bits of halfword are reserved
                            end
                        end
                        2'b10: begin  // Word access
                            led_out_reg <= mem_a_wdata[7:0];
                            // Bits [31:8] are reserved, ignored
                        end
                        default: ;
                    endcase
                end else begin
                    // Always return LED_OUT value in lower 8 bits.
                    // Upper 24 bits read as 0.
                    response_data <= {24'h0, led_out_reg};
                end
            end
        end
    end

endmodule

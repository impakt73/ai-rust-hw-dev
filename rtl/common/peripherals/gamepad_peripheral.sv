`default_nettype none
// Gamepad Peripheral
// Read-only MMIO peripheral exposing Analogue Pocket-style controller state.
// Runs entirely in the bus clock domain; no clock-domain crossing is required.
//
// Register map (base-relative byte offsets):
//   0x00  GAMEPAD_STATE (32-bit read-only)
//         [0]   dpad_up    – D-pad up
//         [1]   dpad_down  – D-pad down
//         [2]   dpad_left  – D-pad left
//         [3]   dpad_right – D-pad right
//         [4]   btn_a      – Face button A
//         [5]   btn_b      – Face button B
//         [6]   btn_x      – Face button X
//         [7]   btn_y      – Face button Y
//         [8]   trig_l     – Left shoulder / trigger
//         [9]   trig_r     – Right shoulder / trigger
//         [31:10] reserved – Always reads as 0
//
// gamepad_in bit assignments (active-high, 1 = button pressed):
//   [0] dpad_up  [1] dpad_down  [2] dpad_left  [3] dpad_right
//   [4] btn_a    [5] btn_b      [6] btn_x       [7] btn_y
//   [8] trig_l   [9] trig_r
//
// On Analogue Pocket hardware gamepad_in should be driven with
// ~cont1_key[9:0] (inverting the active-low Pocket key signals).
//
// Writes are silently accepted and acknowledged with 32'h0 data.

module gamepad_peripheral (
    input  wire logic        clk,
    input  wire logic        rst,

    // Gamepad button inputs (active-high, 1 = pressed)
    // Bit layout matches GAMEPAD_STATE register bits [9:0].
    input  wire logic [9:0]  gamepad_in,

    // Address channel (A)
    input  wire logic [31:0] mem_a_addr,
    input  wire logic [31:0] mem_a_wdata,
    input  wire logic        mem_a_we,
    input  wire logic [1:0]  mem_a_size,
    input  wire logic        mem_a_valid,
    output logic             mem_a_ready,

    // Data channel (D)
    output logic [31:0]      mem_d_rdata,
    output logic             mem_d_valid,
    input  wire logic        mem_d_ready
);

    // =========================================================================
    // Response registers
    // =========================================================================
    // response_pending is the "valid" flag for the response payload.
    // Per project convention, the payload register (response_data) is NOT reset;
    // it is always written before response_pending is asserted so downstream
    // logic can safely ignore it while response_pending is low.
    logic        response_pending;
    logic [31:0] response_data;

    localparam int unsigned GAMEPAD_BUTTON_BITS = 10;
    localparam int unsigned GAMEPAD_RESERVED_BITS = 32 - GAMEPAD_BUTTON_BITS;

    logic mem_a_handshake;
    logic mem_d_handshake;

    assign mem_a_handshake = mem_a_valid && mem_a_ready;
    assign mem_d_handshake = mem_d_valid && mem_d_ready;

    // Accept a new request only when no response is outstanding.
    assign mem_a_ready = !response_pending;
    assign mem_d_rdata = response_data;
    assign mem_d_valid = response_pending;

    always_ff @(posedge clk) begin
        if (rst) begin
            response_pending <= 1'b0;
        end else begin
            // Clear pending flag when the requester consumes the response.
            if (mem_d_handshake) begin
                response_pending <= 1'b0;
            end

            // Latch button state and assert response on every new request.
            // mem_a_handshake and mem_d_handshake cannot both be true in the same
            // cycle: mem_a_ready requires !response_pending, and mem_d_valid
            // requires response_pending.
            if (mem_a_handshake) begin
                if (mem_a_we) begin
                    // Writes are silently accepted; the write data is discarded.
                    response_data <= 32'h0000_0000;
                end else begin
                    // Sample current button state into the response register.
                    // Address and size decoding are intentionally omitted: there
                    // is a single register that responds to any byte offset.
                    response_data <= {
                        {GAMEPAD_RESERVED_BITS{1'b0}},
                        gamepad_in[GAMEPAD_BUTTON_BITS-1:0]
                    };
                end
                response_pending <= 1'b1;
            end
        end
    end

    // mem_a_addr, mem_a_wdata, and mem_a_size are part of the standard bus
    // interface but are not functionally needed by this peripheral (single
    // register, writes ignored, any access size allowed).  They are listed as
    // explicit inputs so the module remains a drop-in for the common bus
    // fabric; synthesis will optimise away the unused connections.
    // Suppress Verilator lint warnings for those intentionally unused inputs.
    /* verilator lint_off UNUSED */
    logic unused_bus_inputs;
    assign unused_bus_inputs = ^mem_a_addr ^ ^mem_a_wdata ^ ^mem_a_size;
    /* verilator lint_on UNUSED */

endmodule

`default_nettype wire

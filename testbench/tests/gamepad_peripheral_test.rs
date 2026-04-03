use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_gamepad_peripheral_runtime, GamepadPeripheralTestWrapper};
use riscv_shared::bus::{
    GAMEPAD_BTN_A, GAMEPAD_BTN_B, GAMEPAD_BTN_X, GAMEPAD_BTN_Y, GAMEPAD_DPAD_DOWN,
    GAMEPAD_DPAD_LEFT, GAMEPAD_DPAD_RIGHT, GAMEPAD_DPAD_UP, GAMEPAD_TRIG_L, GAMEPAD_TRIG_R,
};

const GAMEPAD_BASE_ADDR: u32 = 0x5000_0000;
const MEM_SIZE_WORD: u8 = 2;
const RESET_SETTLE_CYCLES: usize = 4;

// ---------------------------------------------------------------------------
// Clock / bus helpers
// ---------------------------------------------------------------------------

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut GamepadPeripheralTestWrapper) {
    dut.rst = 1;
    dut.gamepad_in = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = 0;
    dut.mem_a_valid = 0;
    dut.mem_d_ready = 0;

    for _ in 0..RESET_SETTLE_CYCLES {
        clock_cycle!(dut);
    }

    dut.rst = 0;
    for _ in 0..RESET_SETTLE_CYCLES {
        clock_cycle!(dut);
    }
}

/// Issue a single bus read and return the data.  Panics on timeout.
fn read_access(dut: &mut GamepadPeripheralTestWrapper, addr: u32) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "gamepad peripheral must be ready to accept a request"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    // Wait for response (should appear within 2 cycles)
    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.mem_d_valid, 1,
        "timed out waiting for gamepad read response"
    );

    let rdata = dut.mem_d_rdata;

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    rdata
}

/// Issue a single bus write and consume the ack response.
fn write_access(dut: &mut GamepadPeripheralTestWrapper, addr: u32, wdata: u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "gamepad peripheral must be ready to accept a write"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    // Consume ack
    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.mem_d_valid, 1,
        "timed out waiting for gamepad write ack"
    );
    assert_eq!(dut.mem_d_rdata, 0, "write ack must return zero data");

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// After reset with no buttons pressed the register reads as zero.
#[test]
fn test_gamepad_reads_zero_after_reset() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(
        state, 0,
        "GAMEPAD_STATE must read 0 when no buttons are pressed"
    );
}

/// Setting all ten button inputs is reflected in the register read.
#[test]
fn test_gamepad_all_buttons_pressed() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    // Press all 10 buttons (bits [9:0])
    dut.gamepad_in = 0b_11_1111_1111;
    dut.eval();

    let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    let expected = GAMEPAD_DPAD_UP
        | GAMEPAD_DPAD_DOWN
        | GAMEPAD_DPAD_LEFT
        | GAMEPAD_DPAD_RIGHT
        | GAMEPAD_BTN_A
        | GAMEPAD_BTN_B
        | GAMEPAD_BTN_X
        | GAMEPAD_BTN_Y
        | GAMEPAD_TRIG_L
        | GAMEPAD_TRIG_R;
    assert_eq!(
        state, expected,
        "GAMEPAD_STATE must reflect all pressed buttons"
    );

    // Upper bits must always be zero
    assert_eq!(state & !0x3FF, 0, "Reserved bits [31:10] must read as 0");
}

/// Individual button bits are mapped to the documented bit positions.
#[test]
fn test_gamepad_individual_button_bit_positions() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    let cases: &[(u16, u32, &str)] = &[
        (1 << 0, GAMEPAD_DPAD_UP, "dpad_up"),
        (1 << 1, GAMEPAD_DPAD_DOWN, "dpad_down"),
        (1 << 2, GAMEPAD_DPAD_LEFT, "dpad_left"),
        (1 << 3, GAMEPAD_DPAD_RIGHT, "dpad_right"),
        (1 << 4, GAMEPAD_BTN_A, "btn_a"),
        (1 << 5, GAMEPAD_BTN_B, "btn_b"),
        (1 << 6, GAMEPAD_BTN_X, "btn_x"),
        (1 << 7, GAMEPAD_BTN_Y, "btn_y"),
        (1 << 8, GAMEPAD_TRIG_L, "trig_l"),
        (1 << 9, GAMEPAD_TRIG_R, "trig_r"),
    ];

    for &(input_bit, expected_mask, name) in cases {
        dut.gamepad_in = input_bit;
        dut.eval();

        let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
        assert_eq!(
            state,
            expected_mask,
            "Button '{name}' (gamepad_in bit {}, expected mask 0x{expected_mask:08x}) \
             but got 0x{state:08x}",
            input_bit.trailing_zeros()
        );
    }
}

/// Input changes between two consecutive reads are visible immediately.
#[test]
fn test_gamepad_input_updates_reflected_between_reads() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    // First read: only dpad_up
    dut.gamepad_in = 0b_00_0000_0001; // dpad_up
    dut.eval();
    let first = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(first, GAMEPAD_DPAD_UP);

    // Second read: only trig_r
    dut.gamepad_in = 0b_10_0000_0000; // trig_r
    dut.eval();
    let second = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(second, GAMEPAD_TRIG_R);

    // Third read: no buttons
    dut.gamepad_in = 0;
    dut.eval();
    let third = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(third, 0);
}

/// Writes are silently acknowledged with zero data and do not affect reads.
#[test]
fn test_gamepad_writes_are_ignored() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    dut.gamepad_in = GAMEPAD_BTN_A as u16;
    dut.eval();

    // Attempt to write all-ones; the peripheral should acknowledge and ignore
    write_access(&mut dut, GAMEPAD_BASE_ADDR, 0xFFFF_FFFF);

    // Button state must still read correctly after the write
    let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(
        state, GAMEPAD_BTN_A,
        "write must not alter the gamepad button state"
    );
}

/// Reserved bits [31:10] always read as zero regardless of gamepad_in.
#[test]
fn test_gamepad_reserved_bits_always_zero() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    // Drive all 10 buttons; the upper 22 bits must still be zero
    dut.gamepad_in = 0b_11_1111_1111;
    dut.eval();

    let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
    assert_eq!(
        state & !0x3FF,
        0,
        "bits [31:10] must read as zero even with all buttons asserted"
    );
}

/// The peripheral accepts a second request immediately after the first
/// response is consumed (no pipeline stall).
#[test]
fn test_gamepad_back_to_back_reads() {
    let runtime =
        create_gamepad_peripheral_runtime().expect("Failed to create gamepad peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<GamepadPeripheralTestWrapper>()
        .expect("Failed to create gamepad peripheral model");

    reset(&mut dut);

    dut.gamepad_in = GAMEPAD_DPAD_LEFT as u16;
    dut.eval();

    for _ in 0..4 {
        let state = read_access(&mut dut, GAMEPAD_BASE_ADDR);
        assert_eq!(
            state, GAMEPAD_DPAD_LEFT,
            "consecutive reads must return consistent results"
        );
    }
}

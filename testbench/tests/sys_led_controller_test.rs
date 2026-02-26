use riscv_core::{create_sys_led_controller_runtime, SysLedControllerWrapper};

const ACTIVITY_BITS_MASK: u8 = 0x1E;
const WRAPPER_CLK_FREQ_HZ: u32 = 4;
const ACTIVITY_FREQ_MILLIHERTZ: u32 = 250;
// Half-period cycles with millihertz scaling, rounded up for integer division.
const ACTIVITY_HALF_PERIOD_CYCLES: u32 =
    ((WRAPPER_CLK_FREQ_HZ * 1000) + ACTIVITY_FREQ_MILLIHERTZ) / (2 * ACTIVITY_FREQ_MILLIHERTZ);
const ACTIVITY_OBSERVE_CYCLES: u32 = (ACTIVITY_HALF_PERIOD_CYCLES * 2) + 8;

fn clock_cycle(dut: &mut SysLedControllerWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

fn reset_dut(dut: &mut SysLedControllerWrapper) {
    dut.rst_n = 0;
    dut.cpu_booting = 0;
    dut.cpu_halted = 0;
    dut.instr_complete = 0;
    dut.sys_bus_handshake = 0;
    dut.host_bus_rx_handshake = 0;
    dut.host_bus_tx_handshake = 0;
    dut.com_err = 0;
    clock_cycle(dut);
    clock_cycle(dut);
}

#[test]
fn test_sys_led_all_ones_during_reset() {
    let runtime =
        create_sys_led_controller_runtime().expect("Failed to create sys_led_controller runtime");
    let mut dut = runtime
        .create_model_simple::<SysLedControllerWrapper>()
        .expect("Failed to create sys_led_controller model");

    reset_dut(&mut dut);
    assert_eq!(dut.sys_led, 0xFF, "All LEDs must be on during reset");
}

#[test]
fn test_sys_led_halted_sets_bit0() {
    let runtime =
        create_sys_led_controller_runtime().expect("Failed to create sys_led_controller runtime");
    let mut dut = runtime
        .create_model_simple::<SysLedControllerWrapper>()
        .expect("Failed to create sys_led_controller model");

    reset_dut(&mut dut);
    dut.rst_n = 1;
    dut.cpu_booting = 0;
    dut.cpu_halted = 1;
    clock_cycle(&mut dut);

    assert_eq!(
        dut.sys_led, 0x01,
        "Only bit 0 must be set when CPU is halted"
    );
}

#[test]
fn test_sys_led_booting_blinks_bit0() {
    let runtime =
        create_sys_led_controller_runtime().expect("Failed to create sys_led_controller runtime");
    let mut dut = runtime
        .create_model_simple::<SysLedControllerWrapper>()
        .expect("Failed to create sys_led_controller model");

    reset_dut(&mut dut);
    dut.rst_n = 1;
    dut.cpu_booting = 1;
    dut.cpu_halted = 0;

    let mut saw_low = false;
    let mut saw_high = false;
    for _ in 0..20 {
        clock_cycle(&mut dut);
        saw_low |= (dut.sys_led & 0x01) == 0;
        saw_high |= (dut.sys_led & 0x01) == 1;
        assert_eq!(
            dut.sys_led & 0x80,
            0,
            "Bit 7 must stay low when com_err is low"
        );
    }

    assert!(
        saw_low && saw_high,
        "Bit 0 must blink while CPU is booting (observe both low and high)"
    );
}

#[test]
fn test_sys_led_activity_indicators_and_com_err() {
    let runtime =
        create_sys_led_controller_runtime().expect("Failed to create sys_led_controller runtime");
    let mut dut = runtime
        .create_model_simple::<SysLedControllerWrapper>()
        .expect("Failed to create sys_led_controller model");

    reset_dut(&mut dut);
    dut.rst_n = 1;
    dut.com_err = 1;

    dut.instr_complete = 1;
    dut.sys_bus_handshake = 1;
    dut.host_bus_rx_handshake = 1;
    dut.host_bus_tx_handshake = 1;
    clock_cycle(&mut dut);
    dut.instr_complete = 0;
    dut.sys_bus_handshake = 0;
    dut.host_bus_rx_handshake = 0;
    dut.host_bus_tx_handshake = 0;

    let mut saw_all_activity_bits_high = false;
    for _ in 0..ACTIVITY_OBSERVE_CYCLES {
        clock_cycle(&mut dut);
        if (dut.sys_led & ACTIVITY_BITS_MASK) == ACTIVITY_BITS_MASK {
            saw_all_activity_bits_high = true;
            break;
        }
    }

    assert!(
        saw_all_activity_bits_high,
        "Bits 1-4 must pulse after activity handshakes"
    );
    assert_eq!(dut.sys_led & 0x80, 0x80, "Bit 7 must reflect com_err input");
}

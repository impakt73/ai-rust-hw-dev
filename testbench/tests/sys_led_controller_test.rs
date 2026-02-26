use riscv_core::{create_sys_led_controller_runtime, SysLedControllerWrapper};

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

    assert_eq!(dut.sys_led, 0x01, "Only bit 0 must be set when CPU is halted");
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
        assert_eq!(dut.sys_led & 0x80, 0, "Bit 7 must stay low when not halted");
    }

    assert!(
        saw_low && saw_high,
        "Bit 0 must blink while CPU is booting (observe both low and high)"
    );
}

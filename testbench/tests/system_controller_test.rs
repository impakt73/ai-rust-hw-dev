/// System Controller Peripheral RTL Tests
///
/// Tests the system_controller module which manages CPU boot, reset, and system control.
///
/// Register Map:
///   0x00 - STATUS (RO): bit 0 = cpu_booting, bit 1 = cpu_halted
///   0x04 - RESET  (WO): write 1 = system reset, write 2 = CPU reset
///   0x08 - BOOT   (WO): write boot address to complete CPU boot
///   0x0C - HALT   (RW): termination code + CPU halt request pulse
///
/// FSM States (one-hot):
///   S_CPU_BOOT_WAIT - Waits for cpu_booting + BOOT write
///   S_CPU_BOOT      - Asserts cpu_boot for one cycle, then -> S_IDLE
///   S_IDLE          - Normal operation
///   S_SYS_RESET     - System reset asserted (stays until external reset)
///   S_CPU_RESET     - CPU reset for one cycle, then -> S_CPU_BOOT_WAIT
use riscv_core::{create_system_controller_runtime, SystemController};

// Register offsets
const REG_STATUS: u32 = 0x00;
const REG_RESET: u32 = 0x04;
const REG_BOOT: u32 = 0x08;
const REG_HALT: u32 = 0x0C;

// Reset control values
const RESET_SYSTEM: u32 = 1;
const RESET_CPU: u32 = 2;

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

fn reset_dut(dut: &mut SystemController) {
    dut.rst_n = 0;
    dut.clk = 0;
    dut.req = 0;
    dut.we = 0;
    dut.addr = 0;
    dut.wdata = 0;
    dut.size = 0b10; // Word
    dut.cpu_halted = 0;
    dut.cpu_booting = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
}

fn read_register(dut: &mut SystemController, offset: u32) -> u32 {
    dut.addr = offset;
    dut.we = 0;
    dut.req = 1;
    dut.size = 0b10; // Word
    dut.eval();
    let result = dut.rdata;
    clock_cycle!(dut);
    dut.req = 0;
    dut.eval();
    result
}

fn write_register(dut: &mut SystemController, offset: u32, value: u32) {
    dut.addr = offset;
    dut.wdata = value;
    dut.we = 1;
    dut.req = 1;
    dut.size = 0b10; // Word
    dut.eval();
    clock_cycle!(dut);
    dut.req = 0;
    dut.we = 0;
    dut.eval();
}

// ============================================================
// Basic Register Tests
// ============================================================

#[test]
fn test_system_controller_ready_always_asserted() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Ready should always be 1 (single-cycle peripheral)
    for _ in 0..10 {
        assert_eq!(dut.ready, 1, "Ready should always be asserted");
        clock_cycle!(dut);
    }
}

#[test]
fn test_system_controller_status_register_read() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // When cpu_booting=0, cpu_halted=0, STATUS should be 0
    dut.cpu_booting = 0;
    dut.cpu_halted = 0;
    let status = read_register(&mut dut, REG_STATUS);
    assert_eq!(
        status & 0x03,
        0,
        "STATUS should be 0 when nothing is active"
    );

    // When cpu_booting=1, bit 0 should be set
    dut.cpu_booting = 1;
    dut.cpu_halted = 0;
    let status = read_register(&mut dut, REG_STATUS);
    assert_eq!(
        status & 0x01,
        1,
        "STATUS bit 0 should reflect cpu_booting=1"
    );
    assert_eq!(status & 0x02, 0, "STATUS bit 1 should reflect cpu_halted=0");

    // When cpu_halted=1, bit 1 should be set
    dut.cpu_booting = 0;
    dut.cpu_halted = 1;
    let status = read_register(&mut dut, REG_STATUS);
    assert_eq!(
        status & 0x01,
        0,
        "STATUS bit 0 should reflect cpu_booting=0"
    );
    assert_eq!(status & 0x02, 2, "STATUS bit 1 should reflect cpu_halted=1");

    // When both are set
    dut.cpu_booting = 1;
    dut.cpu_halted = 1;
    let status = read_register(&mut dut, REG_STATUS);
    assert_eq!(status & 0x03, 3, "STATUS should have both bits set");
}

#[test]
fn test_system_controller_halt_register_read_write() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    assert_eq!(
        read_register(&mut dut, REG_HALT),
        0,
        "HALT register should reset to zero"
    );

    let halt_code = 0x1234_ABCD;
    write_register(&mut dut, REG_HALT, halt_code);

    assert_eq!(
        read_register(&mut dut, REG_HALT),
        halt_code,
        "HALT register should return last written value"
    );
}

#[test]
fn test_system_controller_halt_write_pulses_req_cpu_halt() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    write_register(&mut dut, REG_HALT, 0xCAFE_BABE);
    assert_eq!(
        dut.req_cpu_halt, 1,
        "req_cpu_halt should pulse high for the cycle after HALT write"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.req_cpu_halt, 0,
        "req_cpu_halt should deassert after the one-cycle pulse"
    );
}

// ============================================================
// FSM State Machine Tests
// ============================================================

#[test]
fn test_system_controller_initial_state_after_reset() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // After reset, should be in S_CPU_BOOT_WAIT state
    // cpu_rst_n should be 0 (holding CPU in reset while waiting for boot)
    assert_eq!(
        dut.cpu_rst_n, 0,
        "cpu_rst_n should be low (CPU held in reset) in S_CPU_BOOT_WAIT"
    );

    // sys_rst should be 0 (no system reset)
    assert_eq!(dut.sys_rst, 0, "sys_rst should be low after reset");
}

#[test]
fn test_system_controller_boot_sequence() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Simulate CPU being in boot state
    dut.cpu_booting = 1;
    dut.eval();

    // Write boot address to BOOT register
    let boot_addr: u32 = 0x8000_0000;
    write_register(&mut dut, REG_BOOT, boot_addr);

    // After write, the system controller should have stored the boot address
    assert_eq!(
        dut.cpu_boot_addr, boot_addr,
        "cpu_boot_addr should match written boot address"
    );

    // After one more clock, should transition to S_IDLE (through S_CPU_BOOT)
    clock_cycle!(dut);

    // In S_IDLE, cpu_rst_n should be 1 (CPU released from reset)
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n should be high (CPU released) after boot complete"
    );
}

#[test]
fn test_system_controller_boot_requires_cpu_booting() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // cpu_booting is NOT set
    dut.cpu_booting = 0;
    dut.eval();

    // Write boot address - should be ignored since cpu_booting is not high
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut);

    // Should still be in S_CPU_BOOT_WAIT with cpu_rst_n = 0
    assert_eq!(
        dut.cpu_rst_n, 0,
        "cpu_rst_n should still be low when boot write occurs without cpu_booting"
    );
}

#[test]
fn test_system_controller_boot_addr_output() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    dut.cpu_booting = 1;
    dut.eval();

    // Write a specific boot address
    let test_addr: u32 = 0xDEAD_BEEF;
    write_register(&mut dut, REG_BOOT, test_addr);

    // cpu_boot_addr should reflect the written value
    assert_eq!(
        dut.cpu_boot_addr, test_addr,
        "cpu_boot_addr should output the boot address"
    );
}

// ============================================================
// Reset Control Tests
// ============================================================

#[test]
fn test_system_controller_system_reset() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // First boot the CPU to get to S_IDLE
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut); // S_CPU_BOOT -> S_IDLE

    // Now trigger a system reset
    write_register(&mut dut, REG_RESET, RESET_SYSTEM);
    clock_cycle!(dut);

    // sys_rst should be asserted
    assert_eq!(
        dut.sys_rst, 1,
        "sys_rst should be asserted after system reset trigger"
    );

    // sys_rst should stay asserted (stays in S_SYS_RESET forever)
    for _ in 0..10 {
        clock_cycle!(dut);
        assert_eq!(
            dut.sys_rst, 1,
            "sys_rst should remain asserted in S_SYS_RESET"
        );
    }

    // Only external reset can bring it back
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    clock_cycle!(dut);

    // After external reset, sys_rst should be deasserted
    assert_eq!(dut.sys_rst, 0, "sys_rst should be low after external reset");
}

#[test]
fn test_system_controller_cpu_reset() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Boot the CPU to get to S_IDLE
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut); // S_CPU_BOOT -> S_IDLE

    // Verify CPU is released
    assert_eq!(dut.cpu_rst_n, 1, "cpu_rst_n should be high in S_IDLE");

    // Trigger CPU reset
    write_register(&mut dut, REG_RESET, RESET_CPU);
    // After write_register: cpu_reset_trigger is set
    // Need one more clock cycle for S_IDLE -> S_CPU_RESET
    clock_cycle!(dut);
    // Now in S_CPU_RESET for one cycle, then -> S_CPU_BOOT_WAIT
    clock_cycle!(dut);

    // Now in S_CPU_BOOT_WAIT
    // cpu_rst_n should be 0 (CPU held in reset again)
    assert_eq!(
        dut.cpu_rst_n, 0,
        "cpu_rst_n should be low after CPU reset (back to S_CPU_BOOT_WAIT)"
    );

    // Should be able to boot again
    dut.cpu_booting = 1;
    dut.eval();
    let new_boot_addr: u32 = 0x9000_0000;
    write_register(&mut dut, REG_BOOT, new_boot_addr);
    clock_cycle!(dut);

    assert_eq!(
        dut.cpu_boot_addr, new_boot_addr,
        "cpu_boot_addr should reflect new boot address after CPU reset"
    );
    assert_eq!(dut.cpu_rst_n, 1, "cpu_rst_n should be high after re-boot");
}

// ============================================================
// LED Output Tests
// ============================================================

#[test]
fn test_system_controller_led_halted() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // When cpu_halted is high, all LED bits should be 1
    // sys_led is registered so needs a clock cycle to update
    dut.cpu_halted = 1;
    dut.cpu_booting = 0;
    dut.eval();
    clock_cycle!(dut);

    assert_eq!(
        dut.sys_led, 0xFF,
        "All LEDs should be on when CPU is halted"
    );
}

#[test]
fn test_system_controller_led_booting() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // When cpu_booting is high (and not halted), only first LED bit should be on
    // sys_led is registered so needs a clock cycle to update
    dut.cpu_halted = 0;
    dut.cpu_booting = 1;
    dut.eval();
    clock_cycle!(dut);

    assert_eq!(
        dut.sys_led, 0x01,
        "Only first LED should be on when CPU is booting"
    );
}

#[test]
fn test_system_controller_led_normal() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // When neither halted nor booting, all LEDs should be off
    // sys_led is registered so needs a clock cycle to update
    dut.cpu_halted = 0;
    dut.cpu_booting = 0;
    dut.eval();
    clock_cycle!(dut);

    assert_eq!(
        dut.sys_led, 0x00,
        "All LEDs should be off during normal operation"
    );
}

#[test]
fn test_system_controller_led_halted_takes_priority() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // When both halted and booting, halted takes priority (all LEDs on)
    // sys_led is registered so needs a clock cycle to update
    dut.cpu_halted = 1;
    dut.cpu_booting = 1;
    dut.eval();
    clock_cycle!(dut);

    assert_eq!(
        dut.sys_led, 0xFF,
        "Halted should take priority - all LEDs on even when booting"
    );
}

// ============================================================
// Edge Case Tests
// ============================================================

#[test]
fn test_system_controller_write_to_status_ignored() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // STATUS is read-only, writing should have no effect
    dut.cpu_booting = 1;
    dut.cpu_halted = 0;
    dut.eval();

    let status_before = read_register(&mut dut, REG_STATUS);
    write_register(&mut dut, REG_STATUS, 0xFFFFFFFF);
    let status_after = read_register(&mut dut, REG_STATUS);

    // STATUS should still reflect the actual signal state
    assert_eq!(
        status_before & 0x03,
        status_after & 0x03,
        "Writing to STATUS should have no effect"
    );
}

#[test]
fn test_system_controller_reset_clears_state() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Boot the CPU
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut);

    // Verify we're in S_IDLE
    assert_eq!(dut.cpu_rst_n, 1);

    // Apply external reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    clock_cycle!(dut);

    // Should be back to S_CPU_BOOT_WAIT
    assert_eq!(
        dut.cpu_rst_n, 0,
        "After external reset, should be back in S_CPU_BOOT_WAIT"
    );
    assert_eq!(dut.sys_rst, 0, "sys_rst should be low after external reset");
}

#[test]
fn test_system_controller_cpu_reset_then_reboot() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // First boot
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut);
    assert_eq!(dut.cpu_rst_n, 1, "CPU should be released after first boot");

    // CPU reset
    write_register(&mut dut, REG_RESET, RESET_CPU);
    // write_register does one clock cycle (trigger set)
    // One more for S_IDLE -> S_CPU_RESET
    clock_cycle!(dut);
    // One more for S_CPU_RESET -> S_CPU_BOOT_WAIT
    clock_cycle!(dut);
    assert_eq!(
        dut.cpu_rst_n, 0,
        "CPU should be held in reset after CPU reset"
    );

    // Second boot with different address
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0xA000_0000);
    clock_cycle!(dut);

    assert_eq!(
        dut.cpu_boot_addr, 0xA000_0000,
        "Boot address should be updated on reboot"
    );
    assert_eq!(dut.cpu_rst_n, 1, "CPU should be released after reboot");
}

#[test]
fn test_system_controller_invalid_reset_value_ignored() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Boot the CPU
    dut.cpu_booting = 1;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0x8000_0000);
    clock_cycle!(dut);
    assert_eq!(dut.cpu_rst_n, 1, "CPU should be in S_IDLE");

    // Write invalid value to RESET register (neither 1 nor 2)
    write_register(&mut dut, REG_RESET, 0x42);
    clock_cycle!(dut);

    // Should still be in S_IDLE
    assert_eq!(
        dut.cpu_rst_n, 1,
        "CPU should still be released - invalid reset value ignored"
    );
    assert_eq!(
        dut.sys_rst, 0,
        "sys_rst should not be asserted for invalid reset value"
    );
}

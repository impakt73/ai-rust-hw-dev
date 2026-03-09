/// System Controller Peripheral RTL Tests
///
/// Tests the system_controller module which manages CPU boot, reset, and system control.
///
/// Register Map:
///   0x00 - STATUS (RO): bit 0 = cpu_booting, bit 1 = cpu_halted
///   0x04 - RESET  (WO): write-data bit 0 selects reset type
///                      bit 0 = 0 => system reset, bit 0 = 1 => CPU reset
///   0x08 - BOOT   (WO): write boot address to complete CPU boot
///   0x0C - HALT   (RW): termination code + CPU halt request pulse
///
/// Reads respond immediately; BOOT, HALT, and CPU reset writes respond after the
/// corresponding CPU state transition completes.
use riscv_core::{create_system_controller_runtime, SystemController};

// Register offsets
const REG_STATUS: u32 = 0x00;
const REG_RESET: u32 = 0x04;
const REG_BOOT: u32 = 0x08;
const REG_HALT: u32 = 0x0C;

// Reset control values
const RESET_SYSTEM: u32 = 0;
const RESET_CPU: u32 = 1;

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
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = 0b10; // Word
    dut.mem_d_ready = 0;
    dut.cpu_halted = 0;
    dut.cpu_booting = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
}

fn read_register(dut: &mut SystemController, offset: u32) -> u32 {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_valid = 1;
    dut.mem_a_size = 0b10; // Word
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "system controller should accept read request"
    );
    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            let result = dut.mem_d_rdata;
            dut.mem_d_ready = 1;
            dut.eval();
            clock_cycle!(dut);
            dut.mem_d_ready = 0;
            dut.eval();
            return result;
        }

        clock_cycle!(dut);
    }

    panic!("system controller read did not complete on D channel");
}

fn issue_write_register(dut: &mut SystemController, offset: u32, value: u32) {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = value;
    dut.mem_a_we = 1;
    dut.mem_a_valid = 1;
    dut.mem_a_size = 0b10; // Word
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "system controller should accept write request"
    );
    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();
}

fn complete_pending_response(dut: &mut SystemController) {
    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            dut.mem_d_ready = 1;
            dut.eval();
            clock_cycle!(dut);
            dut.mem_d_ready = 0;
            dut.mem_a_we = 0;
            dut.eval();
            return;
        }

        clock_cycle!(dut);
    }

    panic!("system controller write did not complete on D channel");
}

fn write_register(dut: &mut SystemController, offset: u32, value: u32) {
    issue_write_register(dut, offset, value);
    complete_pending_response(dut);
}

fn finish_response_after_observation(dut: &mut SystemController) {
    assert_eq!(dut.mem_d_valid, 1, "expected pending D-channel response");
    dut.mem_d_ready = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.mem_a_we = 0;
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
        assert_eq!(dut.mem_a_ready, 1, "A channel should stay ready while idle");
        assert_eq!(
            dut.mem_d_valid, 0,
            "D channel should stay idle while no request is pending"
        );
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
    issue_write_register(&mut dut, REG_HALT, halt_code);
    dut.cpu_halted = 1;
    clock_cycle!(dut);
    complete_pending_response(&mut dut);
    dut.cpu_halted = 0;
    dut.eval();

    assert_eq!(
        read_register(&mut dut, REG_HALT),
        halt_code,
        "HALT register should return last written value"
    );
}

#[test]
fn test_system_controller_halt_write_waits_for_cpu_halt() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    issue_write_register(&mut dut, REG_HALT, 0xCAFE_BABE);
    assert_eq!(
        dut.req_cpu_halt, 1,
        "req_cpu_halt should assert while the controller waits for the CPU to halt"
    );
    assert_eq!(
        dut.mem_d_valid, 0,
        "HALT writes must wait for cpu_halted before responding"
    );
    assert_eq!(
        dut.mem_a_ready, 0,
        "controller must block new requests while a HALT is in flight"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 0,
        "HALT response should remain blocked until cpu_halted goes high"
    );
    assert_eq!(
        dut.req_cpu_halt, 1,
        "req_cpu_halt should remain asserted until the CPU reports halted"
    );

    dut.cpu_halted = 1;
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 1,
        "HALT response should be released once cpu_halted goes high"
    );

    finish_response_after_observation(&mut dut);
    assert_eq!(
        dut.req_cpu_halt, 0,
        "req_cpu_halt should deassert after the write response is consumed"
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

    // After reset, cpu_rst_n should be deasserted (inactive high)
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n should be high (inactive) after reset"
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
    issue_write_register(&mut dut, REG_BOOT, boot_addr);

    // After write, the system controller should have stored the boot address
    assert_eq!(
        dut.cpu_boot_addr, boot_addr,
        "cpu_boot_addr should match written boot address"
    );
    assert_eq!(dut.cpu_boot, 1, "cpu_boot should pulse on BOOT write");
    assert_eq!(
        dut.mem_d_valid, 0,
        "BOOT response must wait while the CPU remains in the boot state"
    );

    dut.cpu_booting = 0;
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 1,
        "BOOT response should appear once cpu_booting goes low"
    );
    finish_response_after_observation(&mut dut);

    // After one more clock, the controller should have finished the boot write handling.
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

    // cpu_booting is NOT set; BOOT write should still work.
    dut.cpu_booting = 0;
    dut.eval();
    issue_write_register(&mut dut, REG_BOOT, 0x8000_0000);
    assert_eq!(dut.cpu_boot_addr, 0x8000_0000);
    assert_eq!(dut.cpu_boot, 1, "cpu_boot should pulse on BOOT write");
    assert_eq!(
        dut.mem_d_valid, 0,
        "BOOT responses should not return in the same cycle as the write"
    );
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 1,
        "BOOT response should complete on the next cycle when cpu_booting is already low"
    );
    finish_response_after_observation(&mut dut);
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
    issue_write_register(&mut dut, REG_BOOT, test_addr);
    dut.cpu_booting = 0;
    clock_cycle!(dut);
    finish_response_after_observation(&mut dut);

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

    // Trigger a system reset. By design, no D-channel response is returned.
    issue_write_register(&mut dut, REG_RESET, RESET_SYSTEM);
    assert_eq!(
        dut.sys_rst, 0,
        "sys_rst should remain low in the request cycle"
    );
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 0,
        "system reset should not produce a D-channel response"
    );
    assert_eq!(dut.sys_rst, 1, "sys_rst should pulse one cycle after write");
    assert_eq!(
        dut.mem_a_ready, 1,
        "controller remains A-channel ready until reset takes effect externally"
    );
    clock_cycle!(dut);
    assert_eq!(
        dut.sys_rst, 0,
        "sys_rst pulse should deassert after one cycle"
    );
}

#[test]
fn test_system_controller_cpu_reset() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Trigger CPU reset. The controller must first halt the CPU.
    issue_write_register(&mut dut, REG_RESET, RESET_CPU);
    assert_eq!(
        dut.req_cpu_halt, 1,
        "CPU reset should begin by requesting a CPU halt"
    );
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n should stay high until the CPU reports halted"
    );
    assert_eq!(
        dut.mem_d_valid, 0,
        "CPU reset must not respond before the halt/reset sequence completes"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.req_cpu_halt, 1,
        "halt request should remain asserted while waiting for cpu_halted"
    );
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n must remain high before cpu_halted is observed"
    );

    dut.cpu_halted = 1;
    clock_cycle!(dut);
    assert_eq!(
        dut.req_cpu_halt, 1,
        "halt request remains asserted through the cycle that detects cpu_halted"
    );
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n should not pulse low until after cpu_halted is latched"
    );

    dut.cpu_halted = 0;
    clock_cycle!(dut);
    assert_eq!(
        dut.cpu_rst_n, 0,
        "cpu_rst_n should pulse low after the CPU has halted"
    );
    assert_eq!(
        dut.mem_d_valid, 0,
        "response should remain blocked until the CPU reports booting after reset"
    );

    dut.cpu_booting = 1;
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 1,
        "response should be released once cpu_booting goes high after reset"
    );
    finish_response_after_observation(&mut dut);

    dut.cpu_booting = 0;
    dut.eval();
    assert_eq!(
        dut.cpu_rst_n, 1,
        "cpu_rst_n should return high after one-cycle reset pulse"
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

    // Apply external reset
    dut.rst_n = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    clock_cycle!(dut);

    // Outputs should return to defaults after external reset
    assert_eq!(
        dut.cpu_rst_n, 1,
        "After external reset, cpu_rst_n should be high (inactive)"
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

    // CPU reset
    issue_write_register(&mut dut, REG_RESET, RESET_CPU);
    dut.cpu_halted = 1;
    clock_cycle!(dut);
    dut.cpu_halted = 0;
    clock_cycle!(dut);
    assert_eq!(
        dut.cpu_rst_n, 0,
        "CPU reset should pulse cpu_rst_n low after halt"
    );
    dut.cpu_booting = 1;
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 1,
        "CPU reset response should wait for cpu_booting to go high"
    );
    finish_response_after_observation(&mut dut);
    assert_eq!(dut.cpu_rst_n, 1, "cpu_rst_n should deassert after pulse");

    // Second boot with different address
    dut.cpu_booting = 0;
    dut.eval();
    write_register(&mut dut, REG_BOOT, 0xA000_0000);

    assert_eq!(
        dut.cpu_boot_addr, 0xA000_0000,
        "Boot address should be updated on reboot"
    );
    assert_eq!(dut.cpu_rst_n, 1, "CPU should be released after reboot");
}

#[test]
fn test_system_controller_reset_uses_only_bit_zero() {
    let runtime =
        create_system_controller_runtime().expect("Failed to create system controller runtime");
    let mut dut = runtime
        .create_model_simple::<SystemController>()
        .expect("Failed to create system controller model");

    reset_dut(&mut dut);

    // Boot the CPU
    dut.cpu_booting = 1;
    dut.eval();
    issue_write_register(&mut dut, REG_BOOT, 0x8000_0000);
    dut.cpu_booting = 0;
    clock_cycle!(dut);
    complete_pending_response(&mut dut);
    assert_eq!(dut.cpu_rst_n, 1, "CPU reset should remain deasserted after boot");

    // 0x42 keeps bit 0 cleared while setting upper bits to prove only bit 0 matters.
    issue_write_register(&mut dut, REG_RESET, 0x42);
    assert_eq!(
        dut.sys_rst, 0,
        "sys_rst should remain low in the request cycle"
    );
    clock_cycle!(dut);
    assert_eq!(
        dut.mem_d_valid, 0,
        "system reset should not return a response"
    );
    assert_eq!(
        dut.sys_rst, 1,
        "sys_rst should pulse when RESET bit 0 is cleared, regardless of upper bits"
    );
    clock_cycle!(dut);
    assert_eq!(
        dut.sys_rst, 0,
        "sys_rst pulse should deassert after one cycle"
    );
    assert_eq!(
        dut.cpu_rst_n, 1,
        "system reset write should not assert CPU reset"
    );
}

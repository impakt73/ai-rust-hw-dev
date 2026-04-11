use riscv_core::AsDynamicVerilatedModel;
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
///   0x10 - LED_OUT (RW): bits [7:0] drive the user LEDs
///   0x14 - ELAPSED_US (RO): elapsed microseconds since reset
///   0x18 - ELAPSED_MS (RO): elapsed milliseconds since reset
///   0x1C - ELAPSED_S (RO): elapsed seconds since reset
///   0x20 - CPU_PC (RO): current CPU PC debug signal
///   0x24 - CPU_INSTR (RO): current CPU instruction debug signal
///
/// BOOT, HALT, and system reset are request pulses on register writes.
/// CPU reset writes instead hold HALT high, wait for cpu_halted, pulse cpu_rst high,
/// and only return a D-channel response once cpu_booting is observed after reset.
use riscv_core::SystemController;
use testbench::with_system_controller_model;

// Register offsets
const REG_STATUS: u32 = 0x00;
const REG_RESET: u32 = 0x04;
const REG_BOOT: u32 = 0x08;
const REG_HALT: u32 = 0x0C;
const REG_LED_OUT: u32 = 0x10;
const REG_ELAPSED_US: u32 = 0x14;
const REG_ELAPSED_MS: u32 = 0x18;
const REG_ELAPSED_S: u32 = 0x1C;
const REG_CPU_PC: u32 = 0x20;
const REG_CPU_INSTR: u32 = 0x24;

// Reset control values
const RESET_SYSTEM: u32 = 0;
const RESET_CPU: u32 = 1;
const SIZE_BYTE: u8 = 0b00;
const SIZE_HALFWORD: u8 = 0b01;
const SIZE_WORD: u8 = 0b10;

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
    dut.rst = 1;
    dut.clk = 0;
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_d_ready = 0;
    dut.cpu_halted = 0;
    dut.cpu_booting = 0;
    dut.cpu_pc = 0;
    dut.cpu_instr = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);
    dut.rst = 0;
    dut.eval();
}

fn read_register(dut: &mut SystemController, offset: u32) -> u32 {
    read_register_with_size(dut, offset, SIZE_WORD)
}

fn read_register_with_size(dut: &mut SystemController, offset: u32, size: u8) -> u32 {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_valid = 1;
    dut.mem_a_size = size;
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
    issue_write_register_with_size(dut, offset, value, SIZE_WORD);
}

fn issue_write_register_with_size(dut: &mut SystemController, offset: u32, value: u32, size: u8) {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = value;
    dut.mem_a_we = 1;
    dut.mem_a_valid = 1;
    dut.mem_a_size = size;
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

fn write_register_with_size(dut: &mut SystemController, offset: u32, value: u32, size: u8) {
    issue_write_register_with_size(dut, offset, value, size);
    complete_pending_response(dut);
}

fn write_register_and_wait_for_response(dut: &mut SystemController, offset: u32, value: u32) {
    issue_write_register(dut, offset, value);

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            return;
        }

        clock_cycle!(dut);
    }

    panic!("system controller write did not reach D-channel response");
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
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // Ready should stay 1 while the controller is idle.
        for _ in 0..10 {
            assert_eq!(dut.mem_a_ready, 1, "A channel should stay ready while idle");
            assert_eq!(
                dut.mem_d_valid, 0,
                "D channel should stay idle while no request is pending"
            );
            clock_cycle!(dut);
        }
    });
}

#[test]
fn test_system_controller_status_register_read() {
    with_system_controller_model(|mut dut| {
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
    });
}

#[test]
fn test_system_controller_halt_register_read_write() {
    with_system_controller_model(|mut dut| {
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
    });
}

#[test]
fn test_system_controller_halt_write_pulses_req_cpu_halt() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        write_register_and_wait_for_response(&mut dut, REG_HALT, 0xCAFE_BABE);
        assert_eq!(
            dut.req_cpu_halt, 1,
            "req_cpu_halt should pulse high while the write response is pending"
        );

        finish_response_after_observation(&mut dut);
        assert_eq!(
            dut.req_cpu_halt, 0,
            "req_cpu_halt should deassert after the write response is consumed"
        );
    });
}

#[test]
fn test_system_controller_led_register_read_write() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        assert_eq!(
            read_register(&mut dut, REG_LED_OUT),
            0,
            "LED register should reset to zero"
        );
        assert_eq!(dut.led_out, 0, "LED output should reset low");

        write_register(&mut dut, REG_LED_OUT, 0xAB);
        assert_eq!(
            read_register(&mut dut, REG_LED_OUT),
            0xAB,
            "LED register should return the stored 8-bit value"
        );
        assert_eq!(dut.led_out, 0xAB, "LED output should track the register");

        write_register_with_size(&mut dut, REG_LED_OUT, 0x1234_0055, SIZE_BYTE);
        assert_eq!(
            dut.led_out, 0x55,
            "Byte writes should update the low LED byte"
        );

        write_register_with_size(&mut dut, REG_LED_OUT, 0x5678_00CC, SIZE_HALFWORD);
        assert_eq!(
            dut.led_out, 0xCC,
            "Halfword writes should update the low LED byte and ignore upper bits"
        );
    });
}

#[test]
fn test_system_controller_led_register_upper_bytes_ignored() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        write_register_with_size(&mut dut, REG_LED_OUT + 1, 0xFF, SIZE_BYTE);
        assert_eq!(
            dut.led_out, 0,
            "Byte writes above the low LED byte should be ignored"
        );

        write_register_with_size(&mut dut, REG_LED_OUT + 2, 0xAAAA, SIZE_HALFWORD);
        assert_eq!(
            dut.led_out, 0,
            "Halfword writes outside the low LED halfword should be ignored"
        );
    });
}

#[test]
fn test_system_controller_clock_registers_reset_to_zero() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        assert_eq!(
            read_register(&mut dut, REG_ELAPSED_US),
            0,
            "ELAPSED_US should be zero after reset"
        );
        assert_eq!(
            read_register(&mut dut, REG_ELAPSED_MS),
            0,
            "ELAPSED_MS should be zero after reset"
        );
        assert_eq!(
            read_register(&mut dut, REG_ELAPSED_S),
            0,
            "ELAPSED_S should be zero after reset"
        );
    });
}

#[test]
fn test_system_controller_elapsed_us_advances() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        let us_0 = read_register(&mut dut, REG_ELAPSED_US);
        clock_cycle!(dut);
        let us_1 = read_register(&mut dut, REG_ELAPSED_US);
        assert!(
            us_1.saturating_sub(us_0) >= 1,
            "ELAPSED_US should advance after one clock cycle"
        );

        for _ in 0..8 {
            clock_cycle!(dut);
        }
        let us_9 = read_register(&mut dut, REG_ELAPSED_US);
        assert!(
            us_9 >= us_1 + 8,
            "ELAPSED_US should keep advancing with each microsecond tick"
        );
    });
}

#[test]
fn test_system_controller_elapsed_ms_and_s_advance() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        for _ in 0..1_000 {
            clock_cycle!(dut);
        }
        assert_eq!(
            read_register(&mut dut, REG_ELAPSED_MS),
            1,
            "At 1 MHz default clock, 1000 cycles should equal 1 ms"
        );

        for _ in 0..999_000 {
            clock_cycle!(dut);
        }
        assert_eq!(
            read_register(&mut dut, REG_ELAPSED_S),
            1,
            "At 1 MHz default clock, 1,000,000 cycles should equal 1 s"
        );
    });
}

#[test]
fn test_system_controller_clock_registers_are_read_only() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        for _ in 0..10 {
            clock_cycle!(dut);
        }

        let us_before = read_register(&mut dut, REG_ELAPSED_US);
        let ms_before = read_register(&mut dut, REG_ELAPSED_MS);
        let s_before = read_register(&mut dut, REG_ELAPSED_S);

        write_register(&mut dut, REG_ELAPSED_US, 0xDEAD_BEEF);
        write_register(&mut dut, REG_ELAPSED_MS, 0xCAFE_BABE);
        write_register(&mut dut, REG_ELAPSED_S, 0x1234_5678);

        let us_after = read_register(&mut dut, REG_ELAPSED_US);
        let ms_after = read_register(&mut dut, REG_ELAPSED_MS);
        let s_after = read_register(&mut dut, REG_ELAPSED_S);

        assert!(
            us_after > us_before,
            "ELAPSED_US should keep incrementing after ignored writes"
        );
        assert!(
            ms_after >= ms_before,
            "ELAPSED_MS should never move backwards"
        );
        assert!(s_after >= s_before, "ELAPSED_S should never move backwards");
        assert_ne!(us_after, 0xDEAD_BEEF, "ELAPSED_US should not be writable");
        assert_ne!(ms_after, 0xCAFE_BABE, "ELAPSED_MS should not be writable");
        assert_ne!(s_after, 0x1234_5678, "ELAPSED_S should not be writable");
    });
}

#[test]
fn test_system_controller_cpu_debug_registers_reflect_inputs() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        dut.cpu_pc = 0x8000_0120;
        dut.cpu_instr = 0x00C5_8533;
        dut.eval();

        assert_eq!(
            read_register(&mut dut, REG_CPU_PC),
            0x8000_0120,
            "CPU_PC should mirror the live cpu_pc input"
        );
        assert_eq!(
            read_register(&mut dut, REG_CPU_INSTR),
            0x00C5_8533,
            "CPU_INSTR should mirror the live cpu_instr input"
        );
    });
}

#[test]
fn test_system_controller_cpu_debug_registers_are_read_only() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        dut.cpu_pc = 0x8000_0040;
        dut.cpu_instr = 0x0000_0013;
        dut.eval();

        write_register(&mut dut, REG_CPU_PC, 0xDEAD_BEEF);
        write_register(&mut dut, REG_CPU_INSTR, 0xCAFE_BABE);

        assert_eq!(
            read_register(&mut dut, REG_CPU_PC),
            0x8000_0040,
            "CPU_PC writes should be ignored"
        );
        assert_eq!(
            read_register(&mut dut, REG_CPU_INSTR),
            0x0000_0013,
            "CPU_INSTR writes should be ignored"
        );
    });
}

// ============================================================
// FSM State Machine Tests
// ============================================================

#[test]
fn test_system_controller_initial_state_after_reset() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // After reset, cpu_rst should be deasserted (inactive low, active-high signal)
        assert_eq!(
            dut.cpu_rst, 0,
            "cpu_rst should be low (inactive) after reset"
        );

        // sys_rst should be 0 (no system reset)
        assert_eq!(dut.sys_rst, 0, "sys_rst should be low after reset");
    });
}

#[test]
fn test_system_controller_boot_sequence() {
    with_system_controller_model(|mut dut| {
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

        // In S_IDLE, cpu_rst should be 0 (CPU released from reset, active-high)
        assert_eq!(
            dut.cpu_rst, 0,
            "cpu_rst should be low (CPU released) after boot complete"
        );
    });
}

#[test]
fn test_system_controller_boot_requires_cpu_booting() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // cpu_booting is NOT set; BOOT write should still work.
        dut.cpu_booting = 0;
        dut.eval();
        write_register_and_wait_for_response(&mut dut, REG_BOOT, 0x8000_0000);
        assert_eq!(dut.cpu_boot_addr, 0x8000_0000);
        assert_eq!(dut.cpu_boot, 1, "cpu_boot should pulse on BOOT write");
        finish_response_after_observation(&mut dut);
    });
}

#[test]
fn test_system_controller_boot_addr_output() {
    with_system_controller_model(|mut dut| {
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
    });
}

// ============================================================
// Reset Control Tests
// ============================================================

#[test]
fn test_system_controller_system_reset() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // Trigger a system reset
        write_register_and_wait_for_response(&mut dut, REG_RESET, RESET_SYSTEM);
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst should remain low while the reset write response is pending"
        );
        finish_response_after_observation(&mut dut);
        assert_eq!(dut.sys_rst, 1, "sys_rst should pulse one cycle after write");
        clock_cycle!(dut);
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst pulse should deassert after one cycle"
        );
    });
}

#[test]
fn test_system_controller_cpu_reset() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // Trigger CPU reset
        issue_write_register(&mut dut, REG_RESET, RESET_CPU);
        assert_eq!(
            dut.req_cpu_halt, 1,
            "RESET_CPU should hold req_cpu_halt high while waiting for the CPU to halt"
        );
        assert_eq!(
            dut.cpu_rst, 0,
            "cpu_rst should stay low (inactive) until the CPU has halted"
        );
        assert_eq!(
            dut.mem_a_ready, 0,
            "A channel should block new requests while CPU reset sequencing is active"
        );
        assert_eq!(
            dut.mem_d_valid, 0,
            "D response should not complete until the CPU reset sequence finishes"
        );

        dut.mem_a_addr = REG_STATUS;
        dut.mem_a_we = 0;
        dut.mem_a_valid = 1;
        dut.eval();
        assert_eq!(
            dut.mem_a_ready, 0,
            "system controller must reject new A-channel requests during CPU reset sequencing"
        );
        dut.mem_a_valid = 0;
        dut.eval();

        dut.cpu_halted = 1;
        dut.eval();

        let mut saw_reset_pulse = false;
        for _ in 0..4 {
            clock_cycle!(dut);
            if dut.cpu_rst == 1 {
                saw_reset_pulse = true;
                break;
            }
        }

        assert!(
            saw_reset_pulse,
            "cpu_rst should pulse high after cpu_halted goes high"
        );
        assert_eq!(
            dut.req_cpu_halt, 0,
            "req_cpu_halt should deassert once the reset pulse is issued"
        );
        assert_eq!(
            dut.mem_d_valid, 0,
            "response should remain pending until cpu_booting is observed after reset"
        );

        dut.cpu_halted = 0;
        dut.cpu_booting = 1;
        dut.eval();

        for _ in 0..4 {
            if dut.mem_d_valid != 0 {
                break;
            }
            clock_cycle!(dut);
        }

        assert_eq!(
            dut.mem_d_valid, 1,
            "RESET_CPU should complete only after cpu_booting reasserts following reset"
        );
        assert_eq!(
            dut.mem_a_ready, 0,
            "A channel should remain blocked until the reset completion response is consumed"
        );

        finish_response_after_observation(&mut dut);
        assert_eq!(
            dut.mem_a_ready, 1,
            "A channel should accept new requests once the CPU reset response is consumed"
        );
    });
}

// ============================================================
// Edge Case Tests
// ============================================================

#[test]
fn test_system_controller_write_to_status_ignored() {
    with_system_controller_model(|mut dut| {
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
    });
}

#[test]
fn test_system_controller_reset_clears_state() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // Apply external reset
        dut.rst = 1;
        clock_cycle!(dut);
        dut.rst = 0;
        dut.eval();
        clock_cycle!(dut);

        // Outputs should return to defaults after external reset
        assert_eq!(
            dut.cpu_rst, 0,
            "After external reset, cpu_rst should be low (inactive)"
        );
        assert_eq!(dut.sys_rst, 0, "sys_rst should be low after external reset");
    });
}

#[test]
fn test_system_controller_cpu_reset_then_reboot() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // CPU reset
        issue_write_register(&mut dut, REG_RESET, RESET_CPU);
        dut.cpu_halted = 1;
        dut.eval();
        for _ in 0..4 {
            if dut.cpu_rst == 1 {
                break;
            }
            clock_cycle!(dut);
        }
        assert_eq!(
            dut.cpu_rst, 1,
            "CPU reset should pulse cpu_rst high after halt"
        );
        dut.cpu_halted = 0;
        dut.cpu_booting = 1;
        dut.eval();
        for _ in 0..4 {
            if dut.mem_d_valid != 0 {
                break;
            }
            clock_cycle!(dut);
        }
        assert_eq!(
            dut.mem_d_valid, 1,
            "CPU reset should respond after cpu_booting is observed"
        );
        finish_response_after_observation(&mut dut);
        assert_eq!(dut.cpu_rst, 0, "cpu_rst should deassert after pulse");

        // Second boot with different address
        write_register(&mut dut, REG_BOOT, 0xA000_0000);

        assert_eq!(
            dut.cpu_boot_addr, 0xA000_0000,
            "Boot address should be updated on reboot"
        );
        assert_eq!(dut.cpu_rst, 0, "CPU should be released after reboot");
    });
}

#[test]
fn test_system_controller_reset_uses_only_bit_zero() {
    with_system_controller_model(|mut dut| {
        reset_dut(&mut dut);

        // Boot the CPU
        dut.cpu_booting = 1;
        dut.eval();
        write_register(&mut dut, REG_BOOT, 0x8000_0000);
        clock_cycle!(dut);
        assert_eq!(dut.cpu_rst, 0, "CPU should be in S_IDLE (not in reset)");

        // 0x42 keeps bit 0 cleared while setting upper bits to prove only bit 0 matters.
        write_register_and_wait_for_response(&mut dut, REG_RESET, 0x42);
        assert_eq!(
            dut.sys_rst, 0,
            "sys_rst should remain low while the reset write response is pending"
        );
        finish_response_after_observation(&mut dut);
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
            dut.cpu_rst, 0,
            "system reset write should not assert CPU reset"
        );
    });
}

/// Integration tests for BusDevice reset() and clock_cycle() lifecycle hooks
///
/// These tests verify that the Simulator correctly calls reset() and clock_cycle()
/// on registered devices at the appropriate times.
use cpu_sim::*;
use riscv_core::instruction::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// A test device that tracks reset and clock_cycle calls using atomic counters
/// This allows us to verify the device lifecycle from an integration test
struct LifecycleTestDevice {
    size: u32,
    reset_count: Arc<AtomicU32>,
    clock_cycle_count: Arc<AtomicU32>,
}

impl LifecycleTestDevice {
    fn new(size: u32, reset_count: Arc<AtomicU32>, clock_cycle_count: Arc<AtomicU32>) -> Self {
        LifecycleTestDevice {
            size,
            reset_count,
            clock_cycle_count,
        }
    }
}

impl BusDevice for LifecycleTestDevice {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x0 => Ok(self.reset_count.load(Ordering::SeqCst)),
            0x4 => Ok(self.clock_cycle_count.load(Ordering::SeqCst)),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        _offset: u32,
        _value: u32,
    ) -> Result<(), BusDeviceError> {
        Ok(())
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.reset_count.fetch_add(1, Ordering::SeqCst);
    }

    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        self.clock_cycle_count.fetch_add(1, Ordering::SeqCst);
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn name(&self) -> &str {
        "LifecycleTestDevice"
    }
}

#[test]
fn test_device_reset_called_during_simulation() -> Result<(), String> {
    // Initialize logger for test
    let _ = env_logger::builder().is_test(true).try_init();

    // Create shared counters
    let reset_count = Arc::new(AtomicU32::new(0));
    let clock_cycle_count = Arc::new(AtomicU32::new(0));

    let reset_count_clone = reset_count.clone();
    let clock_cycle_count_clone = clock_cycle_count.clone();

    // Run a minimal program that just halts immediately
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Register our lifecycle test device
            let device = Box::new(LifecycleTestDevice::new(
                256,
                reset_count_clone,
                clock_cycle_count_clone,
            ));
            sim.register_device(0x7000_0000, device)?;

            // Write a simple program that halts via SimControl
            // lui x15, 0x80000000  - Load upper immediate for DRAM base
            // addi x10, x0, 1      - Set exit code to 1 (success)
            // lui x11, 0x40000000  - Load upper immediate for SimControl base
            // sw x10, 0(x11)       - Write to tohost (triggers halt)
            // jal x0, 0            - Infinite loop (stay here)
            let instructions: Vec<u32> = vec![
                lui(15, 0x80000000), // lui x15, 0x80000000
                addi(10, 0, 1),      // addi x10, x0, 1
                lui(11, 0x40000000), // lui x11, 0x40000000
                sw(11, 10, 0),       // sw x10, 0(x11)
                jal(0, 0),           // jal x0, 0
            ];
            let program_bytes: Vec<u8> = instructions
                .iter()
                .flat_map(|inst| inst.to_le_bytes())
                .collect();

            sim.write_memory_region(0x8000_0000, &program_bytes, true);
            Ok(0x8000_0000)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )?;

    // Verify simulation completed
    assert!(result.tohost_value.is_some());
    assert_eq!(result.tohost_value.unwrap(), 1);

    // Verify reset was called at least once (during Simulator::reset())
    let final_reset_count = reset_count.load(Ordering::SeqCst);
    assert!(
        final_reset_count >= 1,
        "reset() should be called at least once, got {}",
        final_reset_count
    );

    // Verify clock_cycle was called multiple times (once per clock cycle)
    // The simulation should take at least a few cycles to execute the program
    let final_clock_cycle_count = clock_cycle_count.load(Ordering::SeqCst);
    assert!(
        final_clock_cycle_count > 0,
        "clock_cycle() should be called multiple times, got {}",
        final_clock_cycle_count
    );

    println!(
        "Device lifecycle test passed: reset_count={}, clock_cycle_count={}",
        final_reset_count, final_clock_cycle_count
    );

    Ok(())
}

#[test]
fn test_device_clock_cycle_called_every_cycle() -> Result<(), String> {
    // Initialize logger for test
    let _ = env_logger::builder().is_test(true).try_init();

    // Create shared counters
    let reset_count = Arc::new(AtomicU32::new(0));
    let clock_cycle_count = Arc::new(AtomicU32::new(0));

    let reset_count_clone = reset_count.clone();
    let clock_cycle_count_clone = clock_cycle_count.clone();

    // Run a program that executes a known number of instructions
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Register our lifecycle test device
            let device = Box::new(LifecycleTestDevice::new(
                256,
                reset_count_clone,
                clock_cycle_count_clone,
            ));
            sim.register_device(0x7000_0000, device)?;

            // Write a simple program with a few NOPs followed by halt
            // lui x15, 0x80000000  - Load upper immediate for DRAM base
            // nop (addi x0, x0, 0) - No operation
            // nop (addi x0, x0, 0) - No operation
            // nop (addi x0, x0, 0) - No operation
            // addi x10, x0, 42     - Set exit code to 42
            // lui x11, 0x40000000  - Load upper immediate for SimControl base
            // sw x10, 0(x11)       - Write to tohost (triggers halt)
            // jal x0, 0            - Infinite loop (stay here)
            let instructions: Vec<u32> = vec![
                lui(15, 0x80000000), // lui x15, 0x80000000
                addi(0, 0, 0),       // nop (addi x0, x0, 0)
                addi(0, 0, 0),       // nop (addi x0, x0, 0)
                addi(0, 0, 0),       // nop (addi x0, x0, 0)
                addi(10, 0, 42),     // addi x10, x0, 42
                lui(11, 0x40000000), // lui x11, 0x40000000
                sw(11, 10, 0),       // sw x10, 0(x11)
                jal(0, 0),           // jal x0, 0
            ];
            let program_bytes: Vec<u8> = instructions
                .iter()
                .flat_map(|inst| inst.to_le_bytes())
                .collect();

            sim.write_memory_region(0x8000_0000, &program_bytes, true);
            Ok(0x8000_0000)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )?;

    // Verify simulation completed with correct exit code
    assert!(result.tohost_value.is_some());
    assert_eq!(result.tohost_value.unwrap(), 42);

    // Verify reset was called exactly once
    let final_reset_count = reset_count.load(Ordering::SeqCst);
    assert_eq!(
        final_reset_count, 1,
        "reset() should be called exactly once during initialization"
    );

    // Verify clock_cycle was called equal to the number of simulation cycles
    let final_clock_cycle_count = clock_cycle_count.load(Ordering::SeqCst);
    assert_eq!(
        final_clock_cycle_count, result.cycles as u32,
        "clock_cycle() should be called once per simulation cycle"
    );

    println!(
        "Clock cycle test passed: {} cycles, reset_count={}, clock_cycle_count={}",
        result.cycles, final_reset_count, final_clock_cycle_count
    );

    Ok(())
}

#[test]
fn test_multiple_devices_receive_lifecycle_calls() -> Result<(), String> {
    // Initialize logger for test
    let _ = env_logger::builder().is_test(true).try_init();

    // Create shared counters for two devices
    let reset_count_1 = Arc::new(AtomicU32::new(0));
    let clock_cycle_count_1 = Arc::new(AtomicU32::new(0));
    let reset_count_2 = Arc::new(AtomicU32::new(0));
    let clock_cycle_count_2 = Arc::new(AtomicU32::new(0));

    let reset_count_1_clone = reset_count_1.clone();
    let clock_cycle_count_1_clone = clock_cycle_count_1.clone();
    let reset_count_2_clone = reset_count_2.clone();
    let clock_cycle_count_2_clone = clock_cycle_count_2.clone();

    // Run a minimal program
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Register first device
            let device1 = Box::new(LifecycleTestDevice::new(
                256,
                reset_count_1_clone,
                clock_cycle_count_1_clone,
            ));
            sim.register_device(0x7000_0000, device1)?;

            // Register second device
            let device2 = Box::new(LifecycleTestDevice::new(
                256,
                reset_count_2_clone,
                clock_cycle_count_2_clone,
            ));
            sim.register_device(0x6000_0000, device2)?;

            // Write a simple halt program
            let instructions: Vec<u32> = vec![
                lui(15, 0x80000000), // lui x15, 0x80000000
                addi(10, 0, 1),      // addi x10, x0, 1
                lui(11, 0x40000000), // lui x11, 0x40000000
                sw(11, 10, 0),       // sw x10, 0(x11)
                jal(0, 0),           // jal x0, 0
            ];
            let program_bytes: Vec<u8> = instructions
                .iter()
                .flat_map(|inst| inst.to_le_bytes())
                .collect();

            sim.write_memory_region(0x8000_0000, &program_bytes, true);
            Ok(0x8000_0000)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )?;

    // Verify simulation completed
    assert!(result.tohost_value.is_some());

    // Both devices should receive the same number of lifecycle calls
    let reset_1 = reset_count_1.load(Ordering::SeqCst);
    let reset_2 = reset_count_2.load(Ordering::SeqCst);
    let clock_1 = clock_cycle_count_1.load(Ordering::SeqCst);
    let clock_2 = clock_cycle_count_2.load(Ordering::SeqCst);

    assert_eq!(
        reset_1, reset_2,
        "Both devices should receive same number of reset calls"
    );
    assert_eq!(
        clock_1, clock_2,
        "Both devices should receive same number of clock_cycle calls"
    );
    assert_eq!(
        clock_1, result.cycles as u32,
        "clock_cycle calls should match simulation cycles"
    );

    println!(
        "Multiple devices test passed: device1 (reset={}, clock={}), device2 (reset={}, clock={})",
        reset_1, clock_1, reset_2, clock_2
    );

    Ok(())
}

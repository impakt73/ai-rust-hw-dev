mod common;

use bus_shared::{BusDevice, Video, VideoConfig, FIFO_BASE};
use common::create_simple_exit_program;
use cpu_sim::{AccessSize, BusRequest, InteractiveSimulator};
use riscv_shared::SUCCESS_CODE;

fn step_instruction_via_cycle(
    sim: &mut InteractiveSimulator,
) -> Result<(Option<u32>, u64), String> {
    let mut cycles_executed = 0;
    let mut tohost_value = None;

    loop {
        let result = sim.step_cycle()?;
        let result_tohost = result.tohost_value;
        cycles_executed += 1;
        tohost_value = tohost_value.or(result_tohost);
        if result.instruction_completed || result_tohost.is_some() {
            return Ok((tohost_value, cycles_executed));
        }
    }
}

#[test]
fn test_interactive_simulator_step_without_elf() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Test that stepping a cycle without loading an ELF is allowed
    let result = sim.step_cycle();
    assert!(
        result.is_ok(),
        "Should allow stepping cycles without loaded ELF"
    );
}

#[test]
fn test_interactive_simulator_step_cycle() {
    let _ = env_logger::builder().is_test(true).try_init();

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

    let max_cycles = 1000;
    let mut instruction_completed = false;

    for _ in 0..max_cycles {
        match sim.step_cycle() {
            Ok(result) if result.instruction_completed => {
                instruction_completed = true;
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("Unexpected error during cycle stepping: {}", e),
        }
    }

    assert!(
        instruction_completed,
        "Instruction should complete within {} cycles",
        max_cycles
    );

    let max_cycles_for_tohost = 10_000;
    let mut tohost_value = None;

    for _ in 0..max_cycles_for_tohost {
        match sim.step_cycle() {
            Ok(result) => {
                if let Some(value) = result.tohost_value {
                    tohost_value = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Unexpected error during cycle stepping: {}", e),
        }
    }

    assert_eq!(
        tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with tohost value {SUCCESS_CODE}"
    );
}

#[test]
fn test_interactive_simulator_simple_program() {
    let _ = env_logger::builder().is_test(true).try_init();

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

    // Step through instructions until we hit tohost or max iterations
    let max_instructions = 100;
    let mut instruction_count = 0;
    let mut tohost_value = None;

    for _ in 0..max_instructions {
        match step_instruction_via_cycle(&mut sim) {
            Ok((tohost, _cycles_executed)) => {
                instruction_count += 1;
                if let Some(value) = tohost {
                    tohost_value = Some(value);
                    break;
                }
            }
            Err(e) => {
                panic!("Unexpected error during execution: {}", e);
            }
        }
    }

    println!("Executed {} instructions", instruction_count);
    assert!(
        tohost_value.is_some(),
        "Program should terminate via tohost"
    );
    assert_eq!(
        tohost_value,
        Some(42),
        "Program should exit with tohost value 42"
    );
}

#[test]
fn test_interactive_simulator_multiple_programs() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test that we can load multiple programs sequentially
    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Load and run first program
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load first program");
    let mut tohost_1 = None;
    for _ in 0..100 {
        match step_instruction_via_cycle(&mut sim) {
            Ok((tohost, _cycles_executed)) => {
                if let Some(value) = tohost {
                    tohost_1 = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Error in first program: {}", e),
        }
    }
    assert_eq!(tohost_1, Some(42), "First program should exit with 42");

    // Load and run second program
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load second program");
    let mut tohost_2 = None;
    for _ in 0..100 {
        match step_instruction_via_cycle(&mut sim) {
            Ok((tohost, _cycles_executed)) => {
                if let Some(value) = tohost {
                    tohost_2 = Some(value);
                    break;
                }
            }
            Err(e) => panic!("Error in second program: {}", e),
        }
    }
    assert_eq!(tohost_2, Some(42), "Second program should exit with 42");
}

#[test]
fn test_interactive_simulator_step_result() {
    let _ = env_logger::builder().is_test(true).try_init();

    let program = create_simple_exit_program();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    sim.load_program(0x8000_0000, &program)
        .expect("Failed to load program");

    // Step once and verify instruction-complete stepping behavior
    let (tohost_value, cycles_executed) =
        step_instruction_via_cycle(&mut sim).expect("First step should succeed");

    // Check that tohost is None initially (program hasn't terminated)
    assert_eq!(tohost_value, None, "First instruction should not terminate");

    // Check that cycles executed is reasonable (should be non-zero)
    assert!(
        cycles_executed > 0,
        "Cycles executed should be greater than 0"
    );
}

#[test]
fn test_interactive_simulator_rejects_overflowing_host_request_range() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");
    let result = sim.send_bus_request(BusRequest::read(0xFFFF_FFFE, AccessSize::Word));

    assert!(
        result.is_err(),
        "Overflowing request range should be rejected"
    );
}

#[test]
fn test_interactive_simulator_can_register_device_at_fifo_base() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut sim = InteractiveSimulator::new().expect("Failed to create simulator");

    // Registering at FIFO_BASE should succeed because FIFO is now an external device.
    let video: Box<dyn BusDevice> = Box::new(Video::new(None::<fn(&[u8], &VideoConfig)>));
    let register_result = sim.register_device(FIFO_BASE, video);

    assert!(register_result.is_ok());
}

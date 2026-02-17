mod common;

use common::{assert_tohost, create_fifo_collector, fifo_data_to_string, init_test_logger};
use cpu_sim::*;

#[test]
fn test_fifo_hello_world() {
    init_test_logger();

    let elf_path = sim_tests::test_program_path("hello_world").expect("Failed to find hello_world");
    let (fifo_data, callback) = create_fifo_collector();

    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(callback),
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        Some(|sim: &mut SimulatorView| {
            // Write test string to FIFO RX after ELF is loaded
            sim.fifo_write_rx_string(test_string);
        }),
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("FIFO hello world simulation should succeed");

    assert_tohost(&result, 0x2a, "hello_world program");

    let received_data = fifo_data.lock().unwrap();
    let received_string = fifo_data_to_string(&received_data);

    assert_eq!(
        received_string, test_string,
        "Expected to receive echoed test string via FIFO"
    );

    println!("✓ FIFO echo test passed in {} cycles", result.cycles);
    println!("✓ Echoed data via FIFO: '{}'", received_string);
}

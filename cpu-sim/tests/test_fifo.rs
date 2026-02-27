mod common;

use common::{
    assert_tohost, create_fifo_collector, create_fifo_echo_program, fifo_data_to_string,
    init_test_logger,
};
use cpu_sim::*;
use riscv_shared::bus::FIFO_BASE;

#[test]
fn test_fifo_hello_world() {
    init_test_logger();

    let program = create_fifo_echo_program();
    let (fifo_data, callback) = create_fifo_collector();

    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        move |sim| {
            sim.write_memory_region(0x8000_0000, &program);
            let fifo_source = std::sync::Arc::new(std::sync::Mutex::new(FifoDataSource::new()));
            fifo_source
                .lock()
                .expect("test fifo_source lock poisoned")
                .push_string_to_fifo_rx(test_string);
            let fifo = Fifo::new_with_callback(fifo_source, callback);
            sim.register_device(FIFO_BASE, Box::new(fifo))
                .map_err(|e| format!("Failed to register FIFO device: {}", e))?;
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("FIFO hello world simulation should succeed");

    assert_tohost(&result, 0x2a, "fifo echo program");

    let received_data = fifo_data.lock().unwrap();
    let received_string = fifo_data_to_string(&received_data);

    assert_eq!(
        received_string, test_string,
        "Expected to receive echoed test string via FIFO"
    );

    println!("✓ FIFO echo test passed in {} cycles", result.cycles);
    println!("✓ Echoed data via FIFO: '{}'", received_string);
}

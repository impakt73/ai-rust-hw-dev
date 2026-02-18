use bus_shared::Dma;
use cpu_sim::{
    run_elf_with_fifo_callback, InstructionTrace, SimulationResult, SimulatorView,
    GLOBAL_MAX_CYCLES,
};
use std::sync::{Arc, Mutex};

#[test]
fn test_dma_copy() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path =
        sim_tests::test_program_path("test_dma_copy").expect("Failed to find test_dma_copy");

    // DMA device base address (from riscv_shared)
    use riscv_shared::dma::DMA_BASE;

    // Track DMA activity via FIFO for debugging (optional)
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
    };

    // Setup callback to register DMA device
    let setup_callback = |view: &mut SimulatorView| {
        // Register DMA device at DMA_BASE
        let dma = Box::new(Dma::new());
        view.register_device(DMA_BASE, dma)
            .expect("Failed to register DMA device");
        log::info!("DMA device registered at 0x{:08x}", DMA_BASE);
    };

    let result = run_elf_with_fifo_callback(
        &elf_path,
        GLOBAL_MAX_CYCLES, // Max cycles
        false,             // print_inst_trace
        false,             // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        Some(Box::new(fifo_callback)),
        Some(setup_callback), // Register DMA device
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    // Check that the program completed successfully
    assert_eq!(
        result.tohost_value,
        Some(42),
        "DMA test should exit with success code 42"
    );

    println!("\n=== DMA Copy Test ===");
    println!("Cycles: {}", result.cycles);
    println!("Test passed: DMA successfully copied data and verification succeeded");

    // Optional: Print any FIFO output for debugging
    let words = fifo_data.lock().unwrap();
    if !words.is_empty() {
        println!("FIFO output: {:08x?}", &words);
    }
}

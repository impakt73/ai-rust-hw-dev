use bus_shared::Dma;
use cpu_sim::{run_elf, InstructionTrace, SimulationResult, SimulatorView, GLOBAL_MAX_CYCLES};

#[test]
fn test_dma_copy() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path =
        sim_tests::test_program_path("test_dma_copy").expect("Failed to find test_dma_copy");

    // DMA device base address (from riscv_shared)
    use riscv_shared::dma::DMA_BASE;

    // Setup callback to register DMA device
    let setup_callback = |view: &mut SimulatorView| {
        // Register DMA device at DMA_BASE
        let dma = Box::new(Dma::new());
        view.register_device(DMA_BASE, dma)
            .expect("Failed to register DMA device");
        log::info!("DMA device registered at 0x{:08x}", DMA_BASE);
    };

    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES, // Max cycles
        false,             // print_inst_trace
        false,             // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,                 // vcd_path
        0,                    // mem_latency_cycles
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
}

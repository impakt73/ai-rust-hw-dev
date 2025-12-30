pub mod bus;
pub mod dram;
pub mod fifo;
pub mod memory;
pub mod sim;

pub use riscv_core::trace::InstructionTrace;
pub use sim::SimulationResult;

use bus::SystemBus;
use dram::Dram;
use sim::Simulator;
use std::path::Path;

/// Run an ELF file on the simulated CPU with an optional FIFO callback and RX data
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
/// * `fifo_rx_data` - Optional string to write to the FIFO RX queue before running
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_fifo<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
{
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        fifo_rx_data,
        None::<fn(&InstructionTrace)>,
    )
}

/// Run an ELF file on the simulated CPU with optional FIFO and trace callbacks
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
/// * `fifo_rx_data` - Optional string to write to the FIFO RX queue before running
/// * `trace_callback` - Optional callback invoked for each instruction executed (receives InstructionTrace)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_all_callbacks<F, T>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
    fifo_rx_data: Option<&str>,
    trace_callback: Option<T>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
    T: FnMut(&InstructionTrace),
{
    // Initialize DRAM and load ELF
    let mut dram = Dram::new();
    let entry_point = dram
        .load_elf(elf_path)
        .map_err(|e| format!("Error loading ELF: {}", e))?;

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Create system bus with DRAM and FIFO
    let bus = SystemBus::new(dram);

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .map_err(|e| format!("Error creating CPU runtime: {}", e))?;
    let mut sim = Simulator::new(
        &runtime,
        bus,
        entry_point,
        print_inst_trace,
        fifo_callback,
        trace_callback,
    )?;

    // Write data to RX FIFO if provided
    if let Some(data) = fifo_rx_data {
        sim.fifo_write_rx_string(data);
    }

    // Run simulation
    sim.run(max_cycles)
}

/// Run an ELF file on the simulated CPU with an optional trace callback
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace to console
/// * `trace_callback` - Optional callback invoked for each instruction executed (receives InstructionTrace)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::{run_elf_with_trace_callback, InstructionTrace};
/// use std::path::Path;
///
/// let mut trace_count = 0;
/// let trace_callback = |trace: &InstructionTrace| {
///     trace_count += 1;
///     println!("Instruction {}: {:?}", trace_count, trace.inst_type);
/// };
///
/// let result = run_elf_with_trace_callback(
///     Path::new("test.elf"),
///     1000,
///     false,
///     Some(trace_callback)
/// )?;
/// # Ok::<(), String>(())
/// ```
pub fn run_elf_with_trace_callback<T>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    trace_callback: Option<T>,
) -> Result<SimulationResult, String>
where
    T: FnMut(&InstructionTrace),
{
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        None::<fn(u32)>,
        None,
        trace_callback,
    )
}

/// Run an ELF file on the simulated CPU with an optional FIFO callback
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
/// * `fifo_callback` - Optional callback invoked when data is written to the FIFO (receives u32 words)
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
pub fn run_elf_with_callback<F>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    fifo_callback: Option<F>,
) -> Result<SimulationResult, String>
where
    F: FnMut(u32),
{
    run_elf_with_all_callbacks(
        elf_path,
        max_cycles,
        print_inst_trace,
        fifo_callback,
        None,
        None::<fn(&InstructionTrace)>,
    )
}

/// Run an ELF file on the simulated CPU
///
/// # Arguments
/// * `elf_path` - Path to the RISC-V ELF executable
/// * `max_cycles` - Maximum number of cycles to run
/// * `print_inst_trace` - Whether to print instruction trace
///
/// # Returns
/// * `Ok(SimulationResult)` on success
/// * `Err(String)` on error
///
/// # Examples
/// ```no_run
/// use cpu_sim::run_elf;
/// use std::path::Path;
///
/// let result = run_elf(Path::new("test.elf"), 1000, false)?;
/// assert_eq!(result.tohost_value, Some(0x2a));
/// # Ok::<(), String>(())
/// ```
pub fn run_elf(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
) -> Result<SimulationResult, String> {
    run_elf_with_callback(elf_path, max_cycles, print_inst_trace, None::<fn(u32)>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_comprehensive_elf() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the test ELF file
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/test.elf");

        // Run the simulation
        let result = run_elf(&elf_path, 500, false).expect("Simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42)"
        );

        println!(
            "✓ Comprehensive test ELF executed successfully in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_instruction_trace() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the test ELF file
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/test.elf");

        // Run the simulation with instruction trace enabled
        // Note: We can't easily capture and verify the trace output programmatically,
        // but we can verify that the simulation still completes successfully
        let result = run_elf(&elf_path, 500, true).expect("Simulation with trace should succeed");

        // Verify the program halted with the correct exit code
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) with trace enabled"
        );

        println!(
            "✓ Instruction trace test passed in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_register_trace_audit() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the register trace audit test program
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/register_trace_audit.elf");

        println!("\n========================================");
        println!("REGISTER TRACE AUDIT TEST");
        println!("========================================");
        println!("This test verifies that instruction trace correctly displays");
        println!("source and destination register values.");
        println!();
        println!("VERIFICATION GUIDE:");
        println!("- Each ADD instruction: rd_value should equal rs1_value + rs2_value");
        println!("- Each SUB instruction: rd_value should equal rs1_value - rs2_value");
        println!("- Each ADDI instruction: rd_value should equal rs1_value + immediate");
        println!("- Load/Store instructions: verify address calculations and data values");
        println!();
        println!("Expected patterns to verify:");
        println!("  Phase 1: Fibonacci-like sequence (1, 2, 3, 5, 8, 13, 21)");
        println!("  Phase 2: Round numbers (10, 20, 30, 50, 80, 100)");
        println!("  Phase 3: Powers of 2 (1, 2, 4, 8, 16, 32, 64, 128, 256)");
        println!("  Phase 4: Subtraction (100-40=60, 60-40=20)");
        println!("  Phase 5: Load/Store with value 123 (0x7b)");
        println!("========================================\n");

        // Run the simulation with instruction trace enabled
        let result =
            run_elf(&elf_path, 500, true).expect("Register trace audit simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from register trace audit program"
        );

        println!(
            "\n✓ Register trace audit test passed in {} cycles",
            result.cycles
        );
        println!("========================================");
        println!("AUDIT COMPLETE");
        println!("========================================");
        println!("Review the trace output above to verify:");
        println!("1. All ADD results match rs1 + rs2");
        println!("2. All SUB results match rs1 - rs2");
        println!("3. All ADDI results match rs1 + immediate");
        println!("4. Load/store operations show correct register values");
        println!("5. Register values progress through expected sequences");
        println!("========================================\n");
    }

    #[test]
    fn test_rust_bare_metal_elf() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the Rust test program ELF from test_programs directory
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/rust_test.elf");

        // Run the simulation
        let result =
            run_elf(&elf_path, 500, false).expect("Rust bare metal simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from Rust bare metal program"
        );

        println!(
            "✓ Rust bare metal test ELF executed successfully in {} cycles",
            result.cycles
        );
    }

    #[test]
    fn test_fifo_hello_world() {
        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the hello_world test program ELF
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/hello_world.elf");

        // Collect FIFO data via callback
        use std::sync::{Arc, Mutex};
        let fifo_data = Arc::new(Mutex::new(Vec::new()));
        let fifo_data_clone = Arc::clone(&fifo_data);

        let callback = move |word: u32| {
            // Convert u32 word to bytes (little-endian)
            let bytes = [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
                ((word >> 24) & 0xFF) as u8,
            ];
            let mut fifo = fifo_data_clone.lock().unwrap();
            fifo.extend_from_slice(&bytes);
        };

        // Run the simulation with FIFO callback and RX data
        // The program should echo a longer random pattern from RX to TX
        // This tests multiple read/write loop iterations with varied character patterns
        let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
        let result = run_elf_with_fifo(&elf_path, 10000, false, Some(callback), Some(test_string))
            .expect("FIFO hello world simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from hello_world program"
        );

        // Verify the FIFO data
        let received_data = fifo_data.lock().unwrap();
        // Remove trailing null bytes while preserving any embedded nulls
        let end_index = received_data.iter().rposition(|&b| b != 0);
        let trimmed_data: Vec<u8> = match end_index {
            Some(idx) => received_data[..=idx].to_vec(),
            None => Vec::new(),
        };
        let received_string =
            String::from_utf8(trimmed_data).expect("FIFO data should be valid UTF-8");

        assert_eq!(
            received_string, test_string,
            "Expected to receive echoed test string via FIFO"
        );

        println!("✓ FIFO echo test passed in {} cycles", result.cycles);
        println!("✓ Echoed data via FIFO: '{}'", received_string);
    }

    #[test]
    fn test_trace_callback() {
        use super::InstructionTrace;
        use riscv_core::trace::InstructionType;
        use std::sync::{Arc, Mutex};

        // Initialize logger for tests (ignore if already initialized)
        let _ = env_logger::builder().is_test(true).try_init();

        // Load the trace test program ELF
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        let elf_path = workspace_root.join("test_programs/trace_test.elf");

        // Collect instruction traces via callback
        let traces = Arc::new(Mutex::new(Vec::new()));
        let traces_clone = Arc::clone(&traces);

        let trace_callback = move |trace: &InstructionTrace| {
            traces_clone.lock().unwrap().push(trace.clone());
        };

        // Run the simulation with trace callback
        let result = run_elf_with_trace_callback(&elf_path, 500, false, Some(trace_callback))
            .expect("Trace test simulation should succeed");

        // Verify the program halted with the correct exit code (42 = 0x2a)
        assert_eq!(
            result.tohost_value,
            Some(0x2a),
            "Expected tohost value 0x2a (42) from trace test program"
        );

        // Verify we captured traces
        let captured_traces = traces.lock().unwrap();
        assert!(
            !captured_traces.is_empty(),
            "Should have captured instruction traces"
        );

        println!("\n========================================");
        println!("TRACE CALLBACK TEST RESULTS");
        println!("========================================");
        println!("Total instructions traced: {}", captured_traces.len());

        // Validate specific instructions we know should be in the trace
        // The first few instructions should be our known sequence:
        // addi x1, x0, 10
        // addi x2, x0, 20
        // addi x3, x0, 5
        // add x4, x1, x2
        // sub x5, x2, x3

        let mut found_addi_x1 = false;
        let mut found_addi_x2 = false;
        let mut found_addi_x3 = false;
        let mut found_add_x4 = false;
        let mut found_sub_x5 = false;
        let mut found_lui = false;
        let mut found_sw = false;
        let mut found_lw = false;

        for trace in captured_traces.iter() {
            match trace.inst_type {
                InstructionType::Addi => {
                    if let super::InstructionTrace {
                        rd:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rd_reg,
                                value: rd_val,
                            }),
                        rs1:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rs1_reg,
                                value: _,
                            }),
                        immediate: Some(imm),
                        ..
                    } = trace
                    {
                        if *rd_reg == 1 && *rs1_reg == 0 && *imm == 10 && *rd_val == 10 {
                            found_addi_x1 = true;
                            println!("✓ Found: addi x1, x0, 10 → x1=10");
                        }
                        if *rd_reg == 2 && *rs1_reg == 0 && *imm == 20 && *rd_val == 20 {
                            found_addi_x2 = true;
                            println!("✓ Found: addi x2, x0, 20 → x2=20");
                        }
                        if *rd_reg == 3 && *rs1_reg == 0 && *imm == 5 && *rd_val == 5 {
                            found_addi_x3 = true;
                            println!("✓ Found: addi x3, x0, 5 → x3=5");
                        }
                    }
                }
                InstructionType::Add => {
                    if let super::InstructionTrace {
                        rd:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rd_reg,
                                value: rd_val,
                            }),
                        rs1:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rs1_reg,
                                value: rs1_val,
                            }),
                        rs2:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rs2_reg,
                                value: rs2_val,
                            }),
                        ..
                    } = trace
                    {
                        if *rd_reg == 4 && *rs1_reg == 1 && *rs2_reg == 2 && *rd_val == 30 {
                            found_add_x4 = true;
                            println!(
                                "✓ Found: add x4, x1, x2 → x4={} (x1={} + x2={})",
                                rd_val, rs1_val, rs2_val
                            );
                        }
                    }
                }
                InstructionType::Sub => {
                    if let super::InstructionTrace {
                        rd:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rd_reg,
                                value: rd_val,
                            }),
                        rs1:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rs1_reg,
                                value: rs1_val,
                            }),
                        rs2:
                            Some(riscv_core::trace::Operand::Register {
                                reg: rs2_reg,
                                value: rs2_val,
                            }),
                        ..
                    } = trace
                    {
                        if *rd_reg == 5 && *rs1_reg == 2 && *rs2_reg == 3 && *rd_val == 15 {
                            found_sub_x5 = true;
                            println!(
                                "✓ Found: sub x5, x2, x3 → x5={} (x2={} - x3={})",
                                rd_val, rs1_val, rs2_val
                            );
                        }
                    }
                }
                InstructionType::Lui => {
                    found_lui = true;
                    println!("✓ Found: LUI instruction");
                }
                InstructionType::Sw => {
                    found_sw = true;
                    println!("✓ Found: SW (store word) instruction");
                }
                InstructionType::Lw => {
                    found_lw = true;
                    println!("✓ Found: LW (load word) instruction");
                }
                _ => {}
            }
        }

        println!("========================================");

        // Assert that we found the expected instructions
        assert!(found_addi_x1, "Should find addi x1, x0, 10");
        assert!(found_addi_x2, "Should find addi x2, x0, 20");
        assert!(found_addi_x3, "Should find addi x3, x0, 5");
        assert!(found_add_x4, "Should find add x4, x1, x2");
        assert!(found_sub_x5, "Should find sub x5, x2, x3");
        assert!(found_lui, "Should find LUI instruction");
        assert!(found_sw, "Should find SW instruction");
        assert!(found_lw, "Should find LW instruction");

        println!(
            "✓ Trace callback test passed in {} cycles",
            result.cycles
        );
        println!("✓ All expected instructions found and validated");
        println!("========================================\n");
    }
}

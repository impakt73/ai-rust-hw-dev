/// Integration tests for the CPU simulator
use std::process::Command;

#[test]
fn test_run_comprehensive_elf() {
    // Run the CPU simulator with the comprehensive test ELF file
    let output = Command::new(env!("CARGO_BIN_EXE_cpu-sim"))
        .arg("test_programs/test.elf")
        .arg("--max-cycles")
        .arg("500")
        .current_dir(env!("CARGO_MANIFEST_DIR").to_owned() + "/..")
        .output()
        .expect("Failed to execute cpu-sim");

    // Check that the process succeeded
    assert!(
        output.status.success(),
        "CPU simulator failed with status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify output contains success message
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Simulation completed"),
        "Expected success message not found in output:\n{}",
        stdout
    );

    // Verify the program halted with the correct exit code (42 = 0x2a)
    // The log output goes to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("value=0x0000002a"),
        "Expected halt value 0x2a (42) not found in stderr:\n{}",
        stderr
    );

    println!("✓ Comprehensive test ELF executed successfully");
}

#[test]
fn test_run_with_instruction_trace() {
    // Run the CPU simulator with instruction trace enabled
    let output = Command::new(env!("CARGO_BIN_EXE_cpu-sim"))
        .arg("test_programs/test.elf")
        .arg("--print-inst-trace")
        .arg("--max-cycles")
        .arg("500")
        .current_dir(env!("CARGO_MANIFEST_DIR").to_owned() + "/..")
        .output()
        .expect("Failed to execute cpu-sim");

    assert!(
        output.status.success(),
        "CPU simulator with trace failed: {}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify trace output contains expected instructions
    assert!(
        stdout.contains("addi"),
        "Expected ADDI instruction in trace"
    );
    assert!(stdout.contains("beq"), "Expected BEQ instruction in trace");
    assert!(stdout.contains("bne"), "Expected BNE instruction in trace");
    assert!(stdout.contains("blt"), "Expected BLT instruction in trace");
    assert!(stdout.contains("bge"), "Expected BGE instruction in trace");
    assert!(stdout.contains("lw"), "Expected LW instruction in trace");
    assert!(stdout.contains("sw"), "Expected SW instruction in trace");

    println!("✓ Instruction trace test passed");
}

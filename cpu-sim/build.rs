use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../rust-test-program/src/main.rs");
    println!("cargo:rerun-if-changed=../rust-test-program/linker.ld");
    
    // Build the rust-test-program before running tests
    // This ensures the ELF is available for testing
    // We build in release mode to match what the test expects
    let _status = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--package",
            "rust-test-program",
            "--bin",
            "rust_test",
        ])
        .output();
    
    // Don't fail the build if this doesn't work - the test will fail if needed
    // This is just to ensure the binary is built when possible
}

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

#[derive(Deserialize)]
struct CargoToml {
    bin: Vec<BinTarget>,
}

#[derive(Deserialize)]
struct BinTarget {
    name: String,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let rust_test_program_dir = manifest_dir.parent().unwrap().join("rust-test-program");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Ensure rust-test-program directory exists
    if !rust_test_program_dir.exists() {
        panic!(
            "rust-test-program directory not found at: {}",
            rust_test_program_dir.display()
        );
    }

    // Build rust-test-program using cargo
    println!("cargo:warning=Building rust-test-program in release mode...");

    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(&rust_test_program_dir)
        .status()
        .expect("Failed to execute cargo build for rust-test-program");

    if !status.success() {
        panic!("Failed to build rust-test-program");
    }

    // Parse the Cargo.toml to get the list of binaries
    let cargo_toml_path = rust_test_program_dir.join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(&cargo_toml_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read Cargo.toml at {}: {}",
            cargo_toml_path.display(),
            e
        )
    });

    let cargo_toml: CargoToml = toml::from_str(&cargo_toml_content)
        .unwrap_or_else(|e| panic!("Failed to parse Cargo.toml: {}", e));

    // Find the built binaries and copy them to OUT_DIR
    // The binaries are built to: rust-test-program/target/riscv32imafc-unknown-none-elf/release/
    let target_dir = rust_test_program_dir
        .join("target")
        .join("riscv32imafc-unknown-none-elf")
        .join("release");

    // Copy all binaries to OUT_DIR without .elf extension
    for bin_target in &cargo_toml.bin {
        let binary_name = &bin_target.name;
        let src = target_dir.join(binary_name);
        let dst = out_dir.join(binary_name);

        if src.exists() {
            fs::copy(&src, &dst).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy {} to {}: {}",
                    src.display(),
                    dst.display(),
                    e
                )
            });
            println!("cargo:warning=Copied {} -> {}", binary_name, dst.display());
        } else {
            println!("cargo:warning=Binary not found: {}", src.display());
        }
    }

    // Tell cargo to rerun if rust-test-program source files change
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("build.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("memory.x").display()
    );
}

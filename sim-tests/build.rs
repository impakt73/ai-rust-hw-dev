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

    // Find the built binaries directory
    // The binaries are built to: rust-test-program/target/riscv32imafc-unknown-none-elf/release/
    let target_dir = rust_test_program_dir
        .join("target")
        .join("riscv32imafc-unknown-none-elf")
        .join("release");

    // Build each binary individually so a failure in one does not block the others
    println!("cargo:warning=Building rust-test-program binaries in release mode...");
    let mut failed_binaries = Vec::new();

    for bin_target in &cargo_toml.bin {
        let binary_name = &bin_target.name;
        let output = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--bin")
            .arg(binary_name)
            .current_dir(&rust_test_program_dir)
            .output()
            .expect("Failed to execute cargo build for rust-test-program");

        let src = target_dir.join(binary_name);
        if output.status.success() && src.exists() {
            let dst = out_dir.join(binary_name);
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
            println!(
                "cargo:warning=Binary '{}' failed to build (skipped)",
                binary_name
            );
            failed_binaries.push(binary_name.clone());
        }
    }

    if !failed_binaries.is_empty() {
        println!(
            "cargo:warning=Skipped {} binary(ies) that failed to build: {}",
            failed_binaries.len(),
            failed_binaries.join(", ")
        );
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
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir
            .join(".cargo")
            .join("config.toml")
            .display()
    );
}
